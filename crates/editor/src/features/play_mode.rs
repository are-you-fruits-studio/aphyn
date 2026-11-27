use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        game_window::GameViewFocusResource,
        inspector_window::{
            data::{InspectedEntityResource, OpenProjectResource, OpenedEntitiesAssetResource},
            systems::{open_entities_asset, update_hierarchy_window_system},
        },
        project_reload::ProjectReloadResource,
        test_window::{WINDOW_KIND_GAME, focus_window_by_kind, open_window_by_kind},
        ui_interaction::{ButtonComponent, check_ui_interaction_system},
        windows_layout::save_windows_layout_system,
        world_preload::{
            SimulatedWorldResource, SimulationMode, preload_assets_into_simulated_world_system, rebuild_simulated_world,
            tick_simulated_world_system,
        },
    },
    theme::{PAL, blend},
};

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(PlayModeResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(toggle_play_mode_system)
        .insert_child_system(open_game_window_on_play_system)
        .insert_child_system(update_play_button_system)
        .insert_child_system(update_project_title_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(toggle_play_mode_system);

    update
        .order_system(preload_assets_into_simulated_world_system)
        .before_system(toggle_play_mode_system)
        .before_system(tick_simulated_world_system);

    update
        .order_system(toggle_play_mode_system)
        .before_system(update_hierarchy_window_system);
    update
        .order_system(toggle_play_mode_system)
        .before_system(update_play_button_system);

    update
        .order_system(toggle_play_mode_system)
        .before_system(open_game_window_on_play_system)
        .before_system(save_windows_layout_system);
}

#[derive(Component, Default, Debug)]
pub struct PlayButtonComponent;

#[derive(Component, Default, Debug)]
pub struct ProjectTitleTextComponent;

#[derive(Resource, Default)]
pub struct PlayModeResource {
    should_open_game_window: bool,
}

pub fn toggle_play_mode_system(
    button_q: WorldQuery<&ButtonComponent, WithFilter<PlayButtonComponent>>,
    open_project: Res<OpenProjectResource>,
    render_api: Res<RenderApiResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut focus_res: ResMut<GameViewFocusResource>,
    mut play_mode: ResMut<PlayModeResource>,
    mut project_reload: ResMut<ProjectReloadResource>,
) {
    return_if!(!button_q.iter().any(|b| b.was_clicked_this_frame));
    return_if!(simulated_world.0.is_none());

    if simulated_world.is_playing() {
        rebuild_simulated_world(
            &mut simulated_world,
            &mut inspected_entity,
            &open_project.dir_path,
            &render_api,
            SimulationMode::Edit,
            "",
            project_reload.take_pending(),
        );

        let opened = std::mem::take(&mut *opened_asset);
        let world = simulated_world.0.as_mut().unwrap();
        let pending_reselect = inspected_entity.pending_reselect.take();

        open_entities_asset(
            world,
            &mut opened_asset,
            &mut inspected_entity,
            &opened.asset_key,
            &opened.path,
        );

        // reopening the same scene keeps what was selected while playing
        inspected_entity.pending_reselect = pending_reselect;

        focus_res.is_focused = false;
    } else {
        return_if!(!can_play(&opened_asset));

        rebuild_simulated_world(
            &mut simulated_world,
            &mut inspected_entity,
            &open_project.dir_path,
            &render_api,
            SimulationMode::Play,
            &opened_asset.asset_key,
            false,
        );

        let (world, thread_pool) = simulated_world.world_and_thread_pool().unwrap();
        world.start_if_needed(thread_pool);

        play_mode.should_open_game_window = true;
    }
}

fn update_project_title_system(
    open_project: Res<OpenProjectResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
    mut text_q: WorldQuery<&mut TextComponent, WithFilter<ProjectTitleTextComponent>>,
) {
    let project = std::path::Path::new(&open_project.dir_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = match opened_asset.asset_key.is_empty() {
        true => project,
        false => format!("{project} · {}", opened_asset.asset_key),
    };

    for text_c in text_q.iter_mut() {
        if text_c.text.as_str() != title {
            text_c.text.clear();
            text_c.text.push_str(&title);
        }
    }
}

fn can_play(opened_asset: &OpenedEntitiesAssetResource) -> bool {
    opened_asset.asset_type == Some(AssetType::Scene)
}

fn open_game_window_on_play_system(mut world: WorldDataMut) {
    let play_mode = world.resources_mut().into_get_mut::<PlayModeResource>().unwrap();
    return_if!(!std::mem::take(&mut play_mode.should_open_game_window));

    if !focus_window_by_kind(world.as_mut(), WINDOW_KIND_GAME) {
        open_window_by_kind(world, WINDOW_KIND_GAME);
    }
}

fn update_play_button_system(
    mut ent: EntitiesHolderMut,
    simulated_world: Res<SimulatedWorldResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
) {
    let is_playing = simulated_world.is_playing();
    let is_enabled = is_playing || can_play(&opened_asset);

    let buttons = ent
        .query_filtered::<EntityId, WithFilter<PlayButtonComponent>>()
        .iter()
        .collect::<Vec<_>>();

    for ent_button in buttons {
        if let Some(image_c) = ent.get_component_mut::<ImageComponent>(ent_button) {
            image_c.color = if is_enabled && !is_playing {
                PAL.accent
            } else {
                PAL.surface_raised
            };
        }

        if let Some(ent_bar) = ent.get_component::<ChildComponent>(ent_button).map(|c| c.parent())
            && let Some(image_c) = ent.get_component_mut::<ImageComponent>(ent_bar)
        {
            image_c.color = if is_playing {
                blend(PAL.accent, PAL.bg_app, 0.16)
            } else {
                PAL.bg_app
            };
        }

        let children = ent
            .get_component::<ParentComponent>(ent_button)
            .map(|p| p.children().to_vec())
            .unwrap_or_default();

        for ent_text in children {
            continue_if_not!(Some(text_c) = ent.get_component_mut::<TextComponent>(ent_text));

            let text = if is_playing { "Stop" } else { "Play" };
            if text_c.text.as_str() != text {
                text_c.text.clear();
                text_c.text.push_str(text);
            }

            text_c.color = match (is_playing, is_enabled) {
                (true, _) => PAL.text_1,
                (false, true) => PAL.text_on_accent,
                (false, false) => PAL.text_disabled,
            };
        }
    }
}
