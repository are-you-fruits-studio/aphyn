use fruits_engine::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        game_window::GameViewFocusResource,
        inspector_window::{
            data::{InspectedEntityResource, OpenProjectResource, OpenedEntitiesAssetResource},
            systems::{open_entities_asset, update_hierarchy_window_system},
        },
        test_window::{WINDOW_KIND_GAME, open_window_by_kind},
        ui_interaction::{ButtonComponent, check_ui_interaction_system},
        windows_layout::save_windows_layout_system,
        world_preload::{SimulationMode, SimulatedWorldResource, preload_assets_into_simulated_world_system, rebuild_simulated_world, tick_simulated_world_system},
    },
};

const BUTTON_COLOR: Vec4<f32> = Vec4::new(0.3, 0.3, 0.3, 1.0);
const BUTTON_COLOR_DISABLED: Vec4<f32> = Vec4::new(0.15, 0.15, 0.15, 1.0);
const TEXT_COLOR: Vec4<f32> = Vec4::splat(1.0);
const TEXT_COLOR_DISABLED: Vec4<f32> = Vec4::splat(0.5);

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(PlayModeResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(toggle_play_mode_system)
        .insert_child_system(open_game_window_on_play_system)
        .insert_child_system(update_play_button_system);

    update.order_system(check_ui_interaction_system).before_system(toggle_play_mode_system);

    update
        .order_system(preload_assets_into_simulated_world_system)
        .before_system(toggle_play_mode_system)
        .before_system(tick_simulated_world_system);

    update.order_system(toggle_play_mode_system).before_system(update_hierarchy_window_system);
    update.order_system(toggle_play_mode_system).before_system(update_play_button_system);

    update
        .order_system(toggle_play_mode_system)
        .before_system(open_game_window_on_play_system)
        .before_system(save_windows_layout_system);
}

#[derive(Component, Default, Debug)]
pub struct PlayButtonComponent;

#[derive(Resource, Default)]
pub struct PlayModeResource {
    should_open_game_window: bool,
}

fn toggle_play_mode_system(
    button_q: WorldQuery<&ButtonComponent, WithFilter<PlayButtonComponent>>,
    open_project: Res<OpenProjectResource>,
    render_api: Res<RenderApiResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut focus_res: ResMut<GameViewFocusResource>,
    mut play_mode: ResMut<PlayModeResource>,
) {
    return_if!(!button_q.iter().any(|b| b.was_clicked_this_frame));
    return_if!(simulated_world.0.is_none());

    if simulated_world.is_playing() {
        rebuild_simulated_world(&mut simulated_world, &open_project.dir_path, &render_api, SimulationMode::Edit, "");

        let opened = std::mem::take(&mut *opened_asset);
        let world = simulated_world.0.as_mut().unwrap();

        open_entities_asset(world, &mut opened_asset, &mut inspected_entity, &opened.asset_key, &opened.path);

        focus_res.is_focused = false;
    } else {
        return_if!(!can_play(&opened_asset));

        rebuild_simulated_world(&mut simulated_world, &open_project.dir_path, &render_api, SimulationMode::Play, &opened_asset.asset_key);

        let world = simulated_world.0.as_mut().unwrap();
        world.start_if_needed();

        *inspected_entity = InspectedEntityResource::default();

        play_mode.should_open_game_window = true;
    }
}

fn can_play(opened_asset: &OpenedEntitiesAssetResource) -> bool {
    opened_asset.asset_type == Some(AssetType::Scene)
}

fn open_game_window_on_play_system(mut world: WorldDataMut) {
    let play_mode = world.resources_mut().into_get_mut::<PlayModeResource>().unwrap();
    return_if!(!std::mem::take(&mut play_mode.should_open_game_window));

    open_window_by_kind(world, WINDOW_KIND_GAME);
}

fn update_play_button_system(
    mut ent: EntitiesHolderMut,
    simulated_world: Res<SimulatedWorldResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
) {
    let is_playing = simulated_world.is_playing();
    let is_enabled = is_playing || can_play(&opened_asset);

    let buttons = ent.query_filtered::<EntityId, WithFilter<PlayButtonComponent>>().iter().collect::<Vec<_>>();

    for ent_button in buttons {
        if let Some(image_c) = ent.get_component_mut::<ImageComponent>(ent_button) {
            image_c.color = if is_enabled { BUTTON_COLOR } else { BUTTON_COLOR_DISABLED };
        }

        let children = ent.get_component::<ParentComponent>(ent_button).map(|p| p.children.iter().copied().collect::<Vec<_>>()).unwrap_or_default();

        for ent_text in children {
            continue_if_not!(Some(text_c) = ent.get_component_mut::<TextComponent>(ent_text));

            let text = if is_playing { "Stop" } else { "Play" };
            if text_c.text.as_str() != text {
                text_c.text.clear();
                text_c.text.push_str(text);
            }

            text_c.color = if is_enabled { TEXT_COLOR } else { TEXT_COLOR_DISABLED };
        }
    }
}
