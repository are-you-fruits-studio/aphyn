use std::time::Instant;

use aphyn::*;

use crate::{
    PROJECT_ASSETS_SUBPATH, SYSTEM_GROUP,
    features::{
        clipboard::{CopiedAnimationKey, EditorClipboard, EditorClipboardResource},
        game_window::GameViewFocusResource,
        input_field::{
            InputFieldComponent, InputFieldTextChangedEvent, SelectedInputFieldResource, apply_input_field_context_menu_system,
            is_editor_ctrl_hotkey_just_pressed, update_selected_input_field_text_system,
        },
        inspector_window::{
            data::{EntityFieldEditedEvent, InspectedEntityResource, OpenProjectResource, OpenedEntitiesAssetResource},
            systems::{
                adjust_non_rigid_composite_system, apply_inspector_dropdown_change_to_simulated_world_system,
                apply_inspector_field_text_change_to_simulated_world_system, apply_inspector_number_drag_to_simulated_world_system,
                copy_selected_entity_hotkey_system, open_entities_asset, paste_entities_hotkey_system,
                save_simulated_entities_to_asset_system, toggle_inspector_checkbox_system,
            },
            utils::{
                find_in_parents,
                reflection::{EditorValue, leaf_text, parse_leaf_text},
                serialization::save_asset_from_world_res,
            },
        },
        project_window_selection::{copy_selected_file_hotkey_system, paste_files_hotkey_system},
        transform_gizmo::transform_gizmo_interaction_system,
        ui_interaction::{ButtonComponent, HoverAreaComponent, UiPressResource, check_ui_interaction_system},
        world_preload::{SimulatedWorld, SimulatedWorldResource, SimulationMode, tick_simulated_world_system},
    },
    prefabs::{ANIMATION_ROW_HEIGHT, ANIMATION_TRACK_ROWS},
    theme::PAL,
};

pub const KEY_SIZE: f32 = 8.0;

const VIEW_PADDING: f32 = 10.0;
const PICK_RADIUS: f32 = 6.0;
const TANGENT_HANDLE_LENGTH: f32 = 40.0;
const CURVE_SAMPLES: usize = 160;
const TIME_SNAP: f32 = 0.01;

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(AnimationWindowResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(update_animation_window_root_system)
        .insert_child_system(click_animation_window_buttons_system)
        .insert_child_system(apply_animation_window_fields_system)
        .insert_child_system(animation_window_mouse_system)
        .insert_child_system(record_animation_keys_system)
        .insert_child_system(preview_animation_system)
        .insert_child_system(draw_animation_window_system)
        .insert_child_system(update_animation_window_focus_system)
        .insert_child_system(copy_paste_animation_keys_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(update_animation_window_root_system);

    // the window last clicked in owns Ctrl+C / Ctrl+V
    update
        .order_system(check_ui_interaction_system)
        .before_system(update_animation_window_focus_system);
    update
        .order_system(update_animation_window_focus_system)
        .before_system(copy_paste_animation_keys_system)
        .before_system(copy_selected_entity_hotkey_system)
        .before_system(paste_entities_hotkey_system)
        .before_system(copy_selected_file_hotkey_system)
        .before_system(paste_files_hotkey_system);
    update
        .order_system(update_animation_window_root_system)
        .before_system(copy_paste_animation_keys_system);
    update
        .order_system(copy_paste_animation_keys_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(update_animation_window_root_system)
        .before_system(click_animation_window_buttons_system)
        .before_system(apply_animation_window_fields_system)
        .before_system(animation_window_mouse_system)
        .before_system(record_animation_keys_system)
        .before_system(preview_animation_system)
        .before_system(draw_animation_window_system);

    update
        .order_system(update_selected_input_field_text_system)
        .before_system(apply_animation_window_fields_system);
    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(apply_animation_window_fields_system);

    // edits are keyed before the preview writes the clip back over them
    update
        .order_system(apply_inspector_field_text_change_to_simulated_world_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(apply_inspector_number_drag_to_simulated_world_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(apply_inspector_dropdown_change_to_simulated_world_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(adjust_non_rigid_composite_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(toggle_inspector_checkbox_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(transform_gizmo_interaction_system)
        .before_system(record_animation_keys_system);

    update
        .order_system(click_animation_window_buttons_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(apply_animation_window_fields_system)
        .before_system(record_animation_keys_system);
    update
        .order_system(animation_window_mouse_system)
        .before_system(record_animation_keys_system);

    update
        .order_system(record_animation_keys_system)
        .before_system(preview_animation_system);
    update
        .order_system(preview_animation_system)
        .before_system(tick_simulated_world_system)
        .before_system(save_simulated_entities_to_asset_system)
        .before_system(draw_animation_window_system);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AnimationWindowMode {
    #[default]
    Idle,
    Preview,
    Record,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AnimationWindowTab {
    #[default]
    DopeSheet,
    Curves,
}

#[derive(Copy, Clone, Debug)]
enum AnimationWindowDrag {
    Scrub,
    Key {
        track: usize,
        key: usize,
    },
    Tangent {
        track: usize,
        key: usize,
    },
    HorizontalScroll {
        start_mouse: Vec2<f32>,
        start_view: AnimationView,
    },
    VerticalScroll {
        start_mouse: Vec2<f32>,
        start_view: AnimationView,
    },
}

#[derive(Resource, Default)]
pub struct AnimationWindowResource {
    pub mode: AnimationWindowMode,
    pub tab: AnimationWindowTab,
    // the simulated entity with the edited `AnimationComponent`
    pub root: EntityId,
    pub time: f32,
    pub is_playing: bool,
    // (track, key), the first one is shown in the key fields
    selected_keys: Vec<(usize, usize)>,
    selected_track: Option<usize>,
    drag: Option<AnimationWindowDrag>,
    last_tick: Option<Instant>,
    pub view: AnimationView,
    view_fit: ViewFit,
    // the window last clicked in, so Ctrl+C / Ctrl+V act on keys
    pub is_focused: bool,
}

impl AnimationWindowResource {
    fn primary_key(&self) -> Option<(usize, usize)> {
        self.selected_keys.first().copied()
    }
}

#[derive(Component, Default, Debug)]
pub struct AnimationWindowComponent;

#[derive(Copy, Clone, Debug)]
pub struct AnimationLines {
    pub grid: EntityId,
    pub curves: EntityId,
    pub selected_curve: EntityId,
    pub tangents: EntityId,
    pub keys: EntityId,
    pub selected_keys: EntityId,
    pub cursor: EntityId,
    pub ruler_ticks: EntityId,
    pub ruler_cursor: EntityId,
}

#[derive(Component)]
pub struct AnimationWindowUiComponent {
    pub status_text: EntityId,
    pub time_field: EntityId,
    // time, value, in tangent, out tangent of the selected key
    pub key_fields: [EntityId; 4],
    pub track_list: EntityId,
    pub track_rows: FfiVec<EntityId>,
    pub ruler: EntityId,
    pub ruler_labels: FfiVec<EntityId>,
    pub content: EntityId,
    pub h_scroll: EntityId,
    pub h_thumb: EntityId,
    pub v_scroll: EntityId,
    pub v_thumb: EntityId,
    pub lines: AnimationLines,
}

#[derive(Component, Default, Debug)]
pub struct AnimationPreviewButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationRecordButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationPlayButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationTabButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationInterpolationButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationAutoTangentButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationDeleteButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationDeleteTrackButtonComponent;
#[derive(Component, Default, Debug)]
pub struct AnimationFrameButtonComponent;

#[derive(Component, Default, Debug)]
pub struct AnimationTrackRowComponent {
    pub idx: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnimationField {
    Time,
    KeyTime,
    KeyValue,
    KeyInTangent,
    KeyOutTangent,
}

#[derive(Component, Debug)]
pub struct AnimationFieldComponent {
    pub field: AnimationField,
}

// simulated world access

fn clip_handle(ent: EntitiesHolderRef, root: EntityId) -> Option<AssetHandle<AnimationClip>> {
    Some(ent.get_component::<AnimationComponent>(root)?.clip.clone())
}

fn clip_of<'r>(world: &'r SimulatedWorld, root: EntityId) -> Option<&'r AnimationClip> {
    let data = world.data.as_ref();
    let handle = clip_handle(data.entities(), root)?;
    world
        .data
        .as_ref()
        .resources()
        .get::<AssetStorageResource<AnimationClip>>()?
        .get(&handle)
}

fn clip_key(world: &SimulatedWorld, root: EntityId) -> Option<String> {
    let data = world.data.as_ref();
    let handle = clip_handle(data.entities(), root)?;
    let storage = data.resources().get::<AssetStorageResource<AnimationClip>>()?;
    storage.get_registration(&handle).map(str::to_string)
}

// edits the root's clip and writes it to its asset file
fn edit_clip<R>(
    world: &mut SimulatedWorld,
    root: EntityId,
    open_project: &OpenProjectResource,
    f: impl FnOnce(&mut AnimationClip) -> R,
) -> Option<R> {
    let result = {
        let (res, ent, _) = world.data.as_mut().into_tuple_mut();
        let handle = clip_handle(ent.as_ref(), root)?;
        f(res.into_get_mut::<AssetStorageResource<AnimationClip>>()?.get_mut(&handle)?)
    };

    save_clip(world, root, open_project);
    Some(result)
}

fn save_clip(world: &SimulatedWorld, root: EntityId, open_project: &OpenProjectResource) {
    return_if_not!(Some(key) = clip_key(world, root));
    return_if_not!(Some(serialized) = save_asset_from_world_res(world.data.as_ref().resources(), &key));

    let path = open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH + &key;
    let json_str = serde_json::to_string_pretty(&serialized.to_json()).unwrap();

    if let Err(err) = std::fs::write(&path, json_str.into_bytes()) {
        eprintln!("failed to write to {path:?}. {err}");
    }
}

// names from the top-level ancestor down to `entity`, to find it again after the asset is reopened
fn name_path(ent: EntitiesHolderRef, entity: EntityId) -> Option<Vec<String>> {
    let mut names = Vec::new();
    let mut current = entity;

    loop {
        names.push(
            ent.get_component::<NameComponent>(current)
                .map(|n| n.0.to_string())
                .unwrap_or_default(),
        );
        match ent.get_component::<ChildComponent>(current) {
            Some(child_c) => current = child_c.parent(),
            None => break,
        }
    }

    names.reverse();
    ent.contains_entity(entity).then_some(names)
}

fn find_by_name_path(ent: EntitiesHolderRef, names: &[String]) -> Option<EntityId> {
    let (top, rest) = names.split_first()?;
    let name_of = |e: EntityId| {
        ent.get_component::<NameComponent>(e)
            .map(|n| n.0.to_string())
            .unwrap_or_default()
    };

    let mut current = ent
        .query::<EntityId>()
        .iter()
        .find(|e| ent.get_component::<ChildComponent>(*e).is_none() && name_of(*e) == *top)?;

    for name in rest {
        let parent = ent.get_component::<ParentComponent>(current)?;
        current = *parent.children().iter().find(|c| name_of(**c) == *name)?;
    }

    Some(current)
}

fn start_preview(window: &mut AnimationWindowResource, opened_asset: &mut OpenedEntitiesAssetResource) {
    return_if!(opened_asset.asset_type.is_none());
    opened_asset.is_saving_paused = true;
    window.mode = AnimationWindowMode::Preview;
    window.last_tick = None;
}

// restores the animated values by reopening the asset, whose registered copy wasn't updated meanwhile
fn stop_preview(
    window: &mut AnimationWindowResource,
    simulated_world: &mut SimulatedWorldResource,
    opened_asset: &mut OpenedEntitiesAssetResource,
    inspected_entity: &mut InspectedEntityResource,
) {
    window.mode = AnimationWindowMode::Idle;
    window.is_playing = false;
    window.drag = None;

    let is_paused = std::mem::take(&mut opened_asset.is_saving_paused);
    return_if!(!is_paused || simulated_world.is_playing());
    return_if_not!(Some(world) = &mut simulated_world.0);

    let selection = name_path(world.data.entities(), inspected_entity.selected_entity);
    let root = name_path(world.data.entities(), window.root);
    let opened = std::mem::take(opened_asset);

    open_entities_asset(world, opened_asset, inspected_entity, &opened.asset_key, &opened.path);

    let ent = world.data.entities();
    if let Some(entity) = selection.and_then(|names| find_by_name_path(ent, &names)) {
        inspected_entity.selected_entity = entity;
    }
    window.root = root
        .and_then(|names| find_by_name_path(ent, &names))
        .unwrap_or(EntityId::EMPTY);
}

// systems

pub fn update_animation_window_root_system(
    mut window: ResMut<AnimationWindowResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
) {
    let window = &mut *window;
    let is_playing = simulated_world.is_playing();

    if window.mode != AnimationWindowMode::Idle {
        let has_root = simulated_world
            .0
            .as_ref()
            .is_some_and(|w| w.data.entities().get_component::<AnimationComponent>(window.root).is_some());

        if is_playing || !opened_asset.is_saving_paused {
            // the world was rebuilt or another asset opened: there is nothing to restore
            window.mode = AnimationWindowMode::Idle;
            window.is_playing = false;
            opened_asset.is_saving_paused = false;
        } else if !has_root {
            stop_preview(window, &mut simulated_world, &mut opened_asset, &mut inspected_entity);
        }
    }

    let Some(world) = &simulated_world.0 else {
        window.root = EntityId::EMPTY;
        return;
    };
    let ent = world.data.entities();

    if window.mode == AnimationWindowMode::Idle {
        let root = find_root_of(ent, inspected_entity.selected_entity, |e| {
            ent.get_component::<AnimationComponent>(e).is_some()
        })
        .unwrap_or(EntityId::EMPTY);

        if root != window.root {
            window.root = root;
            window.selected_keys.clear();
            window.selected_track = None;
            window.drag = None;
            window.view_fit = ViewFit::All;
        }
    }

    let clip = clip_of(world, window.root);
    let track_count = clip.map_or(0, |c| c.tracks.len() as usize);

    window.selected_keys.retain(|(track, key)| {
        clip.and_then(|c| c.tracks.get(*track as u64))
            .is_some_and(|t| (*key as u64) < t.keys.len())
    });
    if window.selected_track.is_some_and(|t| t >= track_count) {
        window.selected_track = None;
    }

    if window.view_fit != ViewFit::None {
        let tracks = curve_tracks(clip, window.selected_track, Some(&anim_types(world)));
        fit_view(
            &mut window.view,
            std::mem::take(&mut window.view_fit),
            clip,
            &tracks,
            Some(&anim_types(world)),
        );
    }
    window.view.first_row = window.view.first_row.min(track_count.saturating_sub(1));

    if is_playing && let Some(anim) = ent.get_component::<AnimationComponent>(window.root) {
        window.time = anim.sample_time(clip.map_or(0.0, AnimationClip::duration));
    }
}

pub fn click_animation_window_buttons_system(
    preview_q: WorldQuery<&ButtonComponent, WithFilter<AnimationPreviewButtonComponent>>,
    record_q: WorldQuery<&ButtonComponent, WithFilter<AnimationRecordButtonComponent>>,
    play_q: WorldQuery<&ButtonComponent, WithFilter<AnimationPlayButtonComponent>>,
    tab_q: WorldQuery<&ButtonComponent, WithFilter<AnimationTabButtonComponent>>,
    interpolation_q: WorldQuery<&ButtonComponent, WithFilter<AnimationInterpolationButtonComponent>>,
    auto_q: WorldQuery<&ButtonComponent, WithFilter<AnimationAutoTangentButtonComponent>>,
    delete_q: WorldQuery<&ButtonComponent, WithFilter<AnimationDeleteButtonComponent>>,
    delete_track_q: WorldQuery<&ButtonComponent, WithFilter<AnimationDeleteTrackButtonComponent>>,
    row_q: WorldQuery<(&ButtonComponent, &AnimationTrackRowComponent)>,
    frame_q: WorldQuery<&ButtonComponent, WithFilter<AnimationFrameButtonComponent>>,
    mut window: ResMut<AnimationWindowResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    open_project: Res<OpenProjectResource>,
) {
    let window = &mut *window;

    if clicked(&tab_q) {
        window.tab = match window.tab {
            AnimationWindowTab::DopeSheet => AnimationWindowTab::Curves,
            AnimationWindowTab::Curves => AnimationWindowTab::DopeSheet,
        };
    }

    if let Some((_, row_c)) = row_q.iter().find(|(b, _)| b.was_clicked_this_frame) {
        window.selected_track = Some(window.view.first_row + row_c.idx);
        window.view_fit = ViewFit::Values;
    }

    if clicked(&frame_q) {
        window.view_fit = ViewFit::All;
    }

    return_if!(simulated_world.is_playing() || window.root == EntityId::EMPTY);

    if clicked(&preview_q) {
        match window.mode {
            AnimationWindowMode::Idle => start_preview(window, &mut opened_asset),
            _ => stop_preview(window, &mut simulated_world, &mut opened_asset, &mut inspected_entity),
        }
    }

    if clicked(&record_q) {
        match window.mode {
            AnimationWindowMode::Idle => {
                start_preview(window, &mut opened_asset);
                if window.mode == AnimationWindowMode::Preview {
                    window.mode = AnimationWindowMode::Record;
                }
            }
            AnimationWindowMode::Preview => window.mode = AnimationWindowMode::Record,
            AnimationWindowMode::Record => window.mode = AnimationWindowMode::Preview,
        }
    }

    if clicked(&play_q) {
        if window.mode == AnimationWindowMode::Idle {
            start_preview(window, &mut opened_asset);
        }
        window.is_playing = !window.is_playing && window.mode != AnimationWindowMode::Idle;
        window.last_tick = None;
    }

    return_if_not!(Some(world) = &mut simulated_world.0);
    let root = window.root;
    let selected = window.selected_keys.clone();

    if clicked(&interpolation_q) && !selected.is_empty() {
        edit_clip(world, root, &open_project, |clip| {
            for (track, key) in &selected {
                let k = &mut clip.tracks[*track as u64].keys[*key as u64];
                k.interpolation = match k.interpolation {
                    KeyInterpolation::Constant => KeyInterpolation::Linear,
                    KeyInterpolation::Linear => KeyInterpolation::Hermite,
                    KeyInterpolation::Hermite => KeyInterpolation::Constant,
                };
            }
            clip.mark_edited();
        });
    }

    if clicked(&auto_q) && !selected.is_empty() {
        let anim = anim_types(world);
        edit_clip(world, root, &open_project, |clip| {
            let enable = !clip.tracks[selected[0].0 as u64].keys[selected[0].1 as u64].auto_tangent;
            for (track, key) in &selected {
                clip.tracks[*track as u64].keys[*key as u64].auto_tangent = enable;
                clip.recompute_auto_tangents(&anim, *track);
            }
        });
    }

    if clicked(&delete_q) && !selected.is_empty() {
        let anim = anim_types(world);
        edit_clip(world, root, &open_project, |clip| {
            let mut sorted = selected.clone();
            sorted.sort_by(|a, b| b.cmp(a));
            for (track, key) in sorted {
                clip.remove_key(&anim, track, key);
            }
        });
        window.selected_keys.clear();
    }

    if clicked(&delete_track_q)
        && let Some(track) = window.selected_track
    {
        edit_clip(world, root, &open_project, |clip| clip.remove_track(track));
        window.selected_keys.clear();
        window.selected_track = None;
    }
}

pub fn apply_animation_window_fields_system(
    text_changed_evt: Evt<InputFieldTextChangedEvent>,
    field_q: WorldQuery<(&InputFieldComponent, &AnimationFieldComponent)>,
    text_q: WorldQuery<&TextComponent>,
    mut window: ResMut<AnimationWindowResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    open_project: Res<OpenProjectResource>,
) {
    let window = &mut *window;
    let is_playing = simulated_world.is_playing();

    for evt in text_changed_evt.iter() {
        continue_if_not!(Some((input_c, field_c)) = field_q.get(evt.input_field));
        continue_if_not!(Some(text_c) = text_q.get(input_c.text));
        let text = text_c.text.as_str().trim();

        if field_c.field == AnimationField::Time {
            if let Ok(time) = text.parse::<f32>() {
                window.time = time.max(0.0);
            }
            continue;
        }

        continue_if!(is_playing);
        continue_if_not!(Some((track, key)) = window.primary_key());
        continue_if_not!(Some(world) = &mut simulated_world.0);
        let root = window.root;
        let anim = anim_types(world);

        match field_c.field {
            AnimationField::KeyTime => {
                continue_if_not!(Ok(time) = text.parse::<f32>());
                let moved = edit_clip(world, root, &open_project, |clip| clip.move_key(&anim, track, key, time)).flatten();
                if let Some(new_key) = moved {
                    window.selected_keys = vec![(track, new_key)];
                }
            }
            AnimationField::KeyValue => {
                continue_if_not!(Some(value) = parse_key_value(world, root, track, key, text));
                edit_clip(world, root, &open_project, |clip| {
                    clip.tracks[track as u64].keys[key as u64].value = value;
                    clip.recompute_auto_tangents(&anim, track);
                });
            }
            AnimationField::KeyInTangent | AnimationField::KeyOutTangent => {
                continue_if_not!(Ok(slope) = text.parse::<f32>());
                let is_in = field_c.field == AnimationField::KeyInTangent;
                edit_clip(world, root, &open_project, |clip| {
                    let k = &mut clip.tracks[track as u64].keys[key as u64];
                    if is_in {
                        k.in_tangent = slope;
                    } else {
                        k.out_tangent = slope;
                    }
                    k.auto_tangent = false;
                    clip.mark_edited();
                });
            }
            AnimationField::Time => {}
        }
    }
}

// a track's component as the editor shows it: its serialized name, or its type name
fn track_component_label(world: Option<&SimulatedWorld>, component: AnyTypeId) -> String {
    let Some(world) = world else {
        return String::new();
    };
    let res = world.data.as_ref().resources();
    if let Some(name) = res.get::<SerializersResource>().and_then(|s| s.name_of(component)) {
        return name.to_string();
    }
    world
        .data
        .types()
        .ffi()
        .get(component.id())
        .map(|info| info.short().name().to_string())
        .unwrap_or_default()
}

// the simulated world in edit mode has no animation module, so the ids are looked up on demand
fn anim_types(world: &SimulatedWorld) -> AnimationTypes {
    AnimationTypes::new(world.data.types())
}

// the key's value parsed from text through the serializers, like an inspector leaf
fn parse_key_value(world: &SimulatedWorld, root: EntityId, track: usize, key: usize, text: &str) -> Option<AnyBox> {
    let clip = clip_of(world, root)?;
    let track = clip.tracks.get(track as u64)?;
    let current = track.keys.get(key as u64)?;
    let res = world.data.as_ref().resources();

    save_load_with_asset_serializers_from_world(res, EntityIdCtx::default(), |state| {
        let (_, kind) = leaf_text(state, current.value.as_any_ref());
        let primitive = parse_leaf_text(kind, text)?;
        state
            .into_ctx(&mut |_| ())
            .deserialize_default_any_by_id(track.value_type, "", &SerializedValue::Primitive(primitive))
    })
    .flatten()
}

fn key_value_text(world: &SimulatedWorld, value: AnyRef) -> String {
    if let Some(v) = anim_types(world).numeric_to_f64(value) {
        return format_number(v);
    }
    let res = world.data.as_ref().resources();
    save_load_with_asset_serializers_from_world(res, EntityIdCtx::default(), |state| leaf_text(state, value).0.to_string())
        .unwrap_or_default()
}

fn format_number(v: f64) -> String {
    let text = format!("{:.3}", v);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.into() }
}

// the visible part of the timeline; it changes only by scrolling, zooming or re-fitting
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AnimationView {
    pub time_start: f32,
    pub time_end: f32,
    pub value_min: f64,
    pub value_max: f64,
    // the dope sheet's (and track list's) first visible track
    pub first_row: usize,
}

impl Default for AnimationView {
    fn default() -> Self {
        Self {
            time_start: 0.0,
            time_end: 1.15,
            value_min: -1.0,
            value_max: 1.0,
            first_row: 0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
enum ViewFit {
    #[default]
    None,
    Values,
    All,
}

fn fit_view(view: &mut AnimationView, fit: ViewFit, clip: Option<&AnimationClip>, tracks: &[usize], anim: Option<&AnimationTypes>) {
    if fit == ViewFit::All {
        view.time_start = 0.0;
        view.time_end = clip.map_or(0.0, AnimationClip::duration).max(1.0) * 1.15;
        view.first_row = 0;
    }

    let (value_min, value_max) = value_bounds(clip, tracks, view.time_start, view.time_end, anim).unwrap_or((-1.0, 1.0));
    let (value_min, value_max) = if value_max - value_min < 1e-6 {
        (value_min - 1.0, value_max + 1.0)
    } else {
        (value_min, value_max)
    };
    let margin = (value_max - value_min) * 0.1;
    view.value_min = value_min - margin;
    view.value_max = value_max + margin;
}

fn value_bounds(
    clip: Option<&AnimationClip>,
    tracks: &[usize],
    time_start: f32,
    time_end: f32,
    anim: Option<&AnimationTypes>,
) -> Option<(f64, f64)> {
    let clip = clip?;
    let anim = anim?;
    let mut bounds: Option<(f64, f64)> = None;

    for &idx in tracks {
        let track = &clip.tracks[idx as u64];
        for i in 0..=CURVE_SAMPLES {
            let t = time_start + (time_end - time_start) * i as f32 / CURVE_SAMPLES as f32;
            if let Some(v) = sample_numeric(anim, &track.keys, t) {
                bounds = Some(bounds.map_or((v, v), |(min, max)| (min.min(v), max.max(v))));
            }
        }
    }

    bounds
}

// mapping between clip time / values and pixels of the timeline content
#[derive(Copy, Clone)]
struct TimelineView {
    min: Vec2<f32>,
    size: Vec2<f32>,
    view: AnimationView,
}

impl TimelineView {
    fn new(rect: &GlobalRectComponent, view: &AnimationView) -> Self {
        Self {
            min: rect.center - rect.scale * 0.5,
            size: rect.scale,
            view: *view,
        }
    }

    fn inner(&self) -> Vec2<f32> {
        (self.size - Vec2::splat(VIEW_PADDING * 2.0)).map(|v| v.max(1.0))
    }

    fn time_span(&self) -> f32 {
        (self.view.time_end - self.view.time_start).max(1e-4)
    }

    fn value_span(&self) -> f64 {
        (self.view.value_max - self.view.value_min).max(1e-9)
    }

    fn x(&self, time: f32) -> f32 {
        self.min.x + VIEW_PADDING + (time - self.view.time_start) / self.time_span() * self.inner().x
    }

    fn time(&self, x: f32) -> f32 {
        self.view.time_start + (x - self.min.x - VIEW_PADDING) / self.inner().x * self.time_span()
    }

    fn y(&self, value: f64) -> f32 {
        let t = (value - self.view.value_min) / self.value_span();
        self.min.y + self.size.y - VIEW_PADDING - t as f32 * self.inner().y
    }

    fn value(&self, y: f32) -> f64 {
        let t = (self.min.y + self.size.y - VIEW_PADDING - y) / self.inner().y;
        self.view.value_min + t as f64 * self.value_span()
    }

    fn visible_rows(&self) -> usize {
        ((self.size.y / ANIMATION_ROW_HEIGHT).ceil() as usize).min(ANIMATION_TRACK_ROWS)
    }

    // tracks shown in the dope sheet and the track list, top to bottom
    fn row_tracks(&self, track_count: usize) -> std::ops::Range<usize> {
        let start = self.view.first_row.min(track_count);
        start..(start + self.visible_rows()).min(track_count)
    }

    fn row_y(&self, track: usize) -> f32 {
        self.min.y + (track as f32 - self.view.first_row as f32 + 0.5) * ANIMATION_ROW_HEIGHT
    }

    // px per second and px per value unit, y pointing up
    fn scale(&self) -> Vec2<f32> {
        Vec2::new(self.inner().x / self.time_span(), self.inner().y / self.value_span() as f32)
    }

    fn key_position(
        &self,
        tab: AnimationWindowTab,
        track_idx: usize,
        key: &AnimationKey,
        anim: &AnimationTypes,
    ) -> Option<Vec2<f32>> {
        let y = match tab {
            AnimationWindowTab::DopeSheet => self.row_y(track_idx),
            AnimationWindowTab::Curves => self.y(anim.numeric_to_f64(key.value.as_any_ref())?),
        };
        Some(Vec2::new(self.x(key.time), y))
    }

    // the out handle; the in handle mirrors it through the key
    fn tangent_handle(&self, key_pos: Vec2<f32>, slope: f32) -> Vec2<f32> {
        let scale = self.scale();
        let dir = Vec2::new(scale.x, -slope * scale.y);
        let len = (dir.x * dir.x + dir.y * dir.y).sqrt().max(f32::EPSILON);
        key_pos + dir * (TANGENT_HANDLE_LENGTH / len)
    }
}

// what the scrollbars scroll over: the clip (plus some room) joined with the visible part
struct ScrollExtents {
    time: (f32, f32),
    values: (f64, f64),
    rows: usize,
}

fn scroll_extents(
    view: &AnimationView,
    clip: Option<&AnimationClip>,
    tracks: &[usize],
    anim: Option<&AnimationTypes>,
) -> ScrollExtents {
    let duration = clip.map_or(0.0, AnimationClip::duration).max(1.0) * 1.15;
    let keys = value_bounds(clip, tracks, 0.0, duration, anim).unwrap_or((view.value_min, view.value_max));

    ScrollExtents {
        time: (view.time_start.min(0.0), view.time_end.max(duration)),
        values: (keys.0.min(view.value_min), keys.1.max(view.value_max)),
        rows: clip.map_or(0, |c| c.tracks.len() as usize),
    }
}

// the numeric tracks drawn in the curves tab: the selected one, or all
fn curve_tracks(clip: Option<&AnimationClip>, selected_track: Option<usize>, anim: Option<&AnimationTypes>) -> Vec<usize> {
    let (Some(clip), Some(anim)) = (clip, anim) else {
        return Vec::new();
    };
    let numeric = |idx: &usize| clip.tracks.get(*idx as u64).is_some_and(|t| t.is_numeric(anim));

    match selected_track.filter(numeric) {
        Some(track) => vec![track],
        None => (0..clip.tracks.len() as usize).filter(numeric).collect(),
    }
}

fn distance(a: Vec2<f32>, b: Vec2<f32>) -> f32 {
    let d = a - b;
    (d.x * d.x + d.y * d.y).sqrt()
}

pub fn animation_window_mouse_system(
    input: Res<InputResource>,
    ui_q: WorldQuery<&AnimationWindowUiComponent>,
    area_q: WorldQuery<(&GlobalRectComponent, &HoverAreaComponent)>,
    mut window: ResMut<AnimationWindowResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    open_project: Res<OpenProjectResource>,
) {
    let window = &mut *window;

    if !input.mouse.is_pressed(MouseButton::Left) {
        window.drag = None;
    }

    return_if_not!(Some(ui) = ui_q.iter().next());
    return_if_not!(Some((content_rect, content_hover)) = area_q.get(ui.content));
    return_if_not!(Some((_, ruler_hover)) = area_q.get(ui.ruler));
    return_if_not!(Some((_, list_hover)) = area_q.get(ui.track_list));
    return_if_not!(Some((h_rect, h_hover)) = area_q.get(ui.h_scroll));
    return_if_not!(Some((v_rect, v_hover)) = area_q.get(ui.v_scroll));
    return_if_not!(Some(world) = &mut simulated_world.0);
    let anim = anim_types(world);

    let mouse = Vec2::from_array(input.mouse.position.map(|v| v as f32));
    let root = window.root;
    let clip = clip_of(world, root);
    let tracks = curve_tracks(clip, window.selected_track, Some(&anim));
    let view = TimelineView::new(content_rect, &window.view);
    let extents = scroll_extents(&window.view, clip, &tracks, Some(&anim));
    let snap = |t: f32| (t / TIME_SNAP).round() * TIME_SNAP;

    // wheel: tracks / values, Shift: time, Ctrl: zoom time (Ctrl+Shift: zoom values)
    let is_hovered = [content_hover, ruler_hover, list_hover, h_hover, v_hover]
        .iter()
        .any(|h| h.is_hovered);
    let [wheel_x, wheel_y] = input.mouse.scroll_delta_lines;
    if is_hovered && (wheel_x != 0.0 || wheel_y != 0.0) {
        let is_shift = input.keyboard.is_pressed(KeyCode::ShiftLeft) || input.keyboard.is_pressed(KeyCode::ShiftRight);
        let is_ctrl = input.keyboard.is_pressed(KeyCode::ControlLeft) || input.keyboard.is_pressed(KeyCode::ControlRight);
        let v = &mut window.view;

        if is_ctrl {
            let factor = 0.85_f32.powf(wheel_y);
            if is_shift {
                let pivot = view.value(mouse.y);
                v.value_min = pivot + (v.value_min - pivot) * factor as f64;
                v.value_max = pivot + (v.value_max - pivot) * factor as f64;
            } else {
                let pivot = view.time(mouse.x);
                v.time_start = pivot + (v.time_start - pivot) * factor;
                v.time_end = pivot + (v.time_end - pivot) * factor;
            }
        } else {
            let time_lines = if is_shift { wheel_y } else { 0.0 } - wheel_x;
            let shift = -time_lines * 0.1 * view.time_span();
            v.time_start += shift;
            v.time_end += shift;

            if !is_shift {
                match window.tab {
                    AnimationWindowTab::DopeSheet => {
                        let rows = v.first_row as f32 - wheel_y * 3.0;
                        v.first_row = (rows.max(0.0) as usize).min(extents.rows.saturating_sub(1));
                    }
                    AnimationWindowTab::Curves => {
                        let shift = wheel_y as f64 * 0.1 * view.value_span();
                        v.value_min += shift;
                        v.value_max += shift;
                    }
                }
            }
        }
    }

    if input.mouse.is_just_pressed(MouseButton::Left) {
        if h_hover.is_hovered {
            window.drag = Some(AnimationWindowDrag::HorizontalScroll {
                start_mouse: mouse,
                start_view: window.view,
            });
        } else if v_hover.is_hovered {
            window.drag = Some(AnimationWindowDrag::VerticalScroll {
                start_mouse: mouse,
                start_view: window.view,
            });
        } else if ruler_hover.is_hovered {
            window.drag = Some(AnimationWindowDrag::Scrub);
        } else if content_hover.is_hovered && !simulated_world_is_playing(world) {
            window.drag = pick(window, &view, clip, mouse, &anim);

            if let Some(AnimationWindowDrag::Key { track, key }) = window.drag {
                let is_shift = input.keyboard.is_pressed(KeyCode::ShiftLeft) || input.keyboard.is_pressed(KeyCode::ShiftRight);
                if is_shift {
                    match window.selected_keys.iter().position(|k| *k == (track, key)) {
                        Some(idx) => _ = window.selected_keys.remove(idx),
                        None => window.selected_keys.push((track, key)),
                    }
                    window.drag = None;
                } else if !window.selected_keys.contains(&(track, key)) {
                    window.selected_keys = vec![(track, key)];
                } else {
                    window.selected_keys.retain(|k| *k != (track, key));
                    window.selected_keys.insert(0, (track, key));
                }
                window.selected_track = Some(track);
            } else if window.drag.is_none() {
                window.selected_keys.clear();
            }
        }
    }

    let is_editable = !simulated_world_is_playing(world);

    match window.drag {
        Some(AnimationWindowDrag::Scrub) => window.time = snap(view.time(mouse.x)).max(0.0),
        Some(AnimationWindowDrag::HorizontalScroll { start_mouse, start_view }) => {
            let span = extents.time.1 - extents.time.0;
            let shift = (mouse.x - start_mouse.x) / h_rect.scale.x.max(1.0) * span;
            window.view.time_start = start_view.time_start + shift;
            window.view.time_end = start_view.time_end + shift;
        }
        Some(AnimationWindowDrag::VerticalScroll { start_mouse, start_view }) => {
            let t = (mouse.y - start_mouse.y) / v_rect.scale.y.max(1.0);
            match window.tab {
                AnimationWindowTab::DopeSheet => {
                    let rows = start_view.first_row as f32 + t * extents.rows.max(view.visible_rows()) as f32;
                    window.view.first_row = (rows.round().max(0.0) as usize).min(extents.rows.saturating_sub(1));
                }
                AnimationWindowTab::Curves => {
                    let shift = -(t as f64) * (extents.values.1 - extents.values.0);
                    window.view.value_min = start_view.value_min + shift;
                    window.view.value_max = start_view.value_max + shift;
                }
            }
        }
        Some(AnimationWindowDrag::Key { track, key }) if is_editable => {
            let time = snap(view.time(mouse.x));
            let value = view.value(mouse.y);
            let tab = window.tab;

            let moved = edit_clip(world, root, &open_project, |clip| {
                let t = clip.tracks.get_mut(track as u64)?;
                if tab == AnimationWindowTab::Curves
                    && let Some(new_value) = anim.numeric_from_f64(t.value_type, value)
                {
                    t.keys.get_mut(key as u64)?.value = new_value;
                }
                clip.move_key(&anim, track, key, time)
            })
            .flatten();

            match moved {
                Some(new_key) => {
                    window.drag = Some(AnimationWindowDrag::Key { track, key: new_key });
                    window.selected_keys = vec![(track, new_key)];
                }
                None => window.drag = None,
            }
        }
        Some(AnimationWindowDrag::Tangent { track, key }) if is_editable => {
            return_if_not!(Some(clip) = clip);
            return_if_not!(Some(k) = clip.tracks.get(track as u64).and_then(|t| t.keys.get(key as u64)));
            return_if_not!(Some(value) = anim.numeric_to_f64(k.value.as_any_ref()));

            let key_pos = Vec2::new(view.x(k.time), view.y(value));
            let scale = view.scale();
            let d = mouse - key_pos;
            return_if!(d.x.abs() < 1.0);
            let slope = (-d.y / scale.y) / (d.x / scale.x);

            edit_clip(world, root, &open_project, |clip| {
                let k = &mut clip.tracks[track as u64].keys[key as u64];
                k.in_tangent = slope;
                k.out_tangent = slope;
                k.auto_tangent = false;
                clip.mark_edited();
            });
        }
        _ => {}
    }
}

fn simulated_world_is_playing(world: &SimulatedWorld) -> bool {
    world.mode == SimulationMode::Play
}

fn pick(
    window: &AnimationWindowResource,
    view: &TimelineView,
    clip: Option<&AnimationClip>,
    mouse: Vec2<f32>,
    anim: &AnimationTypes,
) -> Option<AnimationWindowDrag> {
    let clip = clip?;

    if window.tab == AnimationWindowTab::Curves
        && let Some((track, key)) = window.primary_key()
        && let Some(k) = clip.tracks.get(track as u64).and_then(|t| t.keys.get(key as u64))
        && let Some(pos) = view.key_position(window.tab, track, k, anim)
    {
        let out_handle = view.tangent_handle(pos, k.out_tangent);
        let in_handle = pos * 2.0 - view.tangent_handle(pos, k.in_tangent);
        if distance(out_handle, mouse) < PICK_RADIUS || distance(in_handle, mouse) < PICK_RADIUS {
            return Some(AnimationWindowDrag::Tangent { track, key });
        }
    }

    let tracks: Vec<usize> = match window.tab {
        AnimationWindowTab::DopeSheet => view.row_tracks(clip.tracks.len() as usize).collect(),
        AnimationWindowTab::Curves => curve_tracks(Some(clip), window.selected_track, Some(anim)),
    };

    let mut best: Option<(f32, usize, usize)> = None;
    for track_idx in tracks {
        let track = &clip.tracks[track_idx as u64];
        for (key_idx, key) in track.keys.iter().enumerate() {
            continue_if_not!(Some(pos) = view.key_position(window.tab, track_idx, key, anim));
            let d = distance(pos, mouse);
            if d < PICK_RADIUS && best.is_none_or(|(b, ..)| d < b) {
                best = Some((d, track_idx, key_idx));
            }
        }
    }

    best.map(|(_, track, key)| AnimationWindowDrag::Key { track, key })
}

pub fn record_animation_keys_system(
    field_edited_evt: Evt<EntityFieldEditedEvent>,
    window: Res<AnimationWindowResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    open_project: Res<OpenProjectResource>,
) {
    return_if!(window.mode != AnimationWindowMode::Record || field_edited_evt.is_empty());
    return_if!(simulated_world.is_playing());
    return_if_not!(Some(world) = &mut simulated_world.0);

    let root = window.root;
    let anim = anim_types(world);
    let time = window.time;

    let mut recorded = Vec::new();
    {
        let data = world.data.as_ref();
        let ent = data.entities();
        return_if_not!(Some(registry) = data.resources().get::<ReflectionRegistryResource>());

        for evt in field_edited_evt.iter() {
            continue_if!(evt.component == world.data.types().type_id_of::<AnimationComponent>().any_type_id());
            continue_if_not!(Some(entity_path) = entity_path_from(ent, root, evt.entity));
            continue_if_not!(Some(component) = ent.get_component_any(evt.entity, evt.component));

            for leaf in collect_leaves(registry, &anim, component, &evt.path) {
                let path = AnimationPath {
                    entity: entity_path.clone(),
                    component: evt.component,
                    field: leaf.field,
                    leaf: leaf.leaf,
                };
                recorded.push((path, leaf.value_type, leaf.value));
            }
        }
    }

    return_if!(recorded.is_empty());

    edit_clip(world, root, &open_project, |clip| {
        for (path, value_type, mut value) in recorded {
            if matches!(path.leaf, AnimatedLeaf::QuatEuler(_))
                && let Some(track) = clip.track_index(&path).map(|idx| &clip.tracks[idx as u64])
                && let Some(reference) = sample_numeric(&anim, &track.keys, time)
                && let Some(angle) = anim.numeric_to_f64(value.as_any_ref())
            {
                value = anim.f64_value(unwrap_degrees_near(angle, reference));
            }
            clip.set_key(&anim, path, value_type, time, value);
        }
    });
}

pub fn preview_animation_system(mut window: ResMut<AnimationWindowResource>, mut simulated_world: ResMut<SimulatedWorldResource>) {
    let window = &mut *window;

    return_if!(window.mode == AnimationWindowMode::Idle || simulated_world.is_playing());
    return_if_not!(Some(world) = &mut simulated_world.0);

    let now = Instant::now();
    let delta_time = window.last_tick.map_or(0.0, |last| now.duration_since(last).as_secs_f32());
    window.last_tick = Some(now);

    let anim_ids = anim_types(world);
    let (res, mut ent, _) = world.data.as_mut().into_tuple_mut();
    let res = res.as_ref();
    let (Some(registry), Some(clips)) = (
        res.get::<ReflectionRegistryResource>(),
        res.get::<AssetStorageResource<AnimationClip>>(),
    ) else {
        return;
    };

    animate_entity(ent.as_mut(), registry, &anim_ids, clips, window.root, |anim, duration| {
        if window.is_playing && duration > 0.0 {
            window.time += delta_time;
            if window.time > duration {
                match anim.wrap {
                    AnimationWrapMode::Once => {
                        window.time = duration;
                        window.is_playing = false;
                    }
                    AnimationWrapMode::Loop | AnimationWrapMode::PingPong => window.time = window.time.rem_euclid(duration),
                }
            }
        }

        // `Loop` wraps the clip's end to its start; preview shows the end instead
        anim.time = if anim.wrap == AnimationWrapMode::Loop && window.time >= duration && duration > 0.0 {
            duration - 1e-4
        } else {
            window.time
        };
    });
}

// drawing

pub(crate) fn set_text(ent: &mut EntitiesHolderMut, entity: EntityId, text: &str, color: Vec4<f32>) {
    if let Some(text_c) = ent.get_component_mut::<TextComponent>(entity) {
        if text_c.text.as_str() != text {
            text_c.text.clear();
            text_c.text.push_str(text);
        }
        text_c.color = color;
    }
}

pub(crate) fn set_field_text(ent: &mut EntitiesHolderMut, selected_input: &SelectedInputFieldResource, field: EntityId, text: &str) {
    return_if!(selected_input.selected == field);
    return_if_not!(Some(text_entity) = ent.get_component::<InputFieldComponent>(field).map(|f| f.text));
    set_text(ent, text_entity, text, PAL.text_1);
}

pub(crate) fn set_points(ent: &mut EntitiesHolderMut, entity: EntityId, points: Vec<Vec2<f32>>) {
    if let Some(line_c) = ent.get_component_mut::<UiLineComponent>(entity) {
        line_c.points = points.into();
    }
}

fn button_entities<C: 'static + Component>(ent: &EntitiesHolderMut) -> Vec<EntityId> {
    ent.query_filtered::<EntityId, WithFilter<C>>().iter().collect()
}

pub(crate) fn style_buttons<C: 'static + Component>(ent: &mut EntitiesHolderMut, color: Vec4<f32>, text: Option<&str>) {
    for ent_button in button_entities::<C>(ent) {
        if let Some(image_c) = ent.get_component_mut::<ImageComponent>(ent_button) {
            image_c.color = color;
        }
        let Some(text) = text else {
            continue;
        };
        let children = ent
            .get_component::<ParentComponent>(ent_button)
            .map(|p| p.children().to_vec())
            .unwrap_or_default();
        for ent_text in children {
            set_text(ent, ent_text, text, PAL.text_1);
        }
    }
}

fn ruler_step(view: &TimelineView) -> f32 {
    let px_per_second = view.scale().x;
    [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
        .into_iter()
        .find(|step| step * px_per_second >= 60.0)
        .unwrap_or(100.0)
}

pub fn draw_animation_window_system(
    mut ent: EntitiesHolderMut,
    window: Res<AnimationWindowResource>,
    simulated_world: Res<SimulatedWorldResource>,
    selected_input: Res<SelectedInputFieldResource>,
) {
    return_if_not!(
        Some(ui) = ent.query::<&AnimationWindowUiComponent>().iter().next().map(|ui| (
            ui.status_text,
            ui.time_field,
            ui.key_fields,
            ui.track_rows.clone(),
            ui.ruler,
            ui.ruler_labels.clone(),
            ui.content,
            ui.h_scroll,
            ui.h_thumb,
            ui.v_scroll,
            ui.v_thumb,
            ui.lines,
        ))
    );
    let (status_text, time_field, key_fields, track_rows, ruler, ruler_labels, content, h_scroll, h_thumb, v_scroll, v_thumb, lines) =
        ui;

    let is_playing = simulated_world.is_playing();
    let world = simulated_world.0.as_ref();
    let clip = world.and_then(|w| clip_of(w, window.root));
    let anim = world.map(anim_types);

    // toolbar
    let is_recording = window.mode == AnimationWindowMode::Record;
    style_buttons::<AnimationPreviewButtonComponent>(
        &mut ent,
        if window.mode != AnimationWindowMode::Idle {
            PAL.accent
        } else {
            PAL.surface_raised
        },
        None,
    );
    style_buttons::<AnimationRecordButtonComponent>(&mut ent, if is_recording { PAL.danger } else { PAL.surface_raised }, None);
    style_buttons::<AnimationPlayButtonComponent>(
        &mut ent,
        if window.is_playing { PAL.accent } else { PAL.surface_raised },
        Some(if window.is_playing { "Pause" } else { "Play" }),
    );
    style_buttons::<AnimationTabButtonComponent>(
        &mut ent,
        PAL.surface_raised,
        Some(match window.tab {
            AnimationWindowTab::DopeSheet => "Dope sheet",
            AnimationWindowTab::Curves => "Curves",
        }),
    );

    let root_name = world
        .and_then(|w| {
            w.data
                .entities()
                .get_component::<NameComponent>(window.root)
                .map(|n| n.0.to_string())
        })
        .unwrap_or_else(|| "unnamed".into());
    let status = match (world, clip) {
        (None, _) => "No project world".to_string(),
        _ if window.root == EntityId::EMPTY => "Select an entity with an AnimationComponent".to_string(),
        (Some(w), None) => format!(
            "{root_name}: the AnimationComponent has no clip {}",
            clip_key(w, window.root).unwrap_or_default()
        ),
        (Some(w), Some(_)) => {
            let mode = match (is_playing, window.mode) {
                (true, _) => " | play mode (read-only)",
                (false, AnimationWindowMode::Idle) => "",
                (false, AnimationWindowMode::Preview) => " | previewing",
                (false, AnimationWindowMode::Record) => " | recording edits",
            };
            format!("{root_name} | {}{mode}", clip_key(w, window.root).unwrap_or_default())
        }
    };
    set_text(&mut ent, status_text, &status, PAL.text_1);
    set_field_text(&mut ent, &selected_input, time_field, &format_number(window.time as f64));

    // selected key
    let primary = window.primary_key().and_then(|(t, k)| {
        let track = clip?.tracks.get(t as u64)?;
        Some((track, track.keys.get(k as u64)?))
    });
    let key_texts = match (primary, world) {
        (Some((track, key)), Some(w)) => [
            format_number(key.time as f64),
            key_value_text(w, key.value.as_any_ref()),
            if track.is_numeric(&anim_types(w)) {
                format_number(key.in_tangent as f64)
            } else {
                String::new()
            },
            if track.is_numeric(&anim_types(w)) {
                format_number(key.out_tangent as f64)
            } else {
                String::new()
            },
        ],
        _ => Default::default(),
    };
    for (field, text) in key_fields.into_iter().zip(key_texts.iter()) {
        set_field_text(&mut ent, &selected_input, field, text);
    }
    style_buttons::<AnimationInterpolationButtonComponent>(
        &mut ent,
        PAL.surface_raised,
        Some(match primary.map(|(_, k)| k.interpolation) {
            Some(KeyInterpolation::Constant) => "Constant",
            Some(KeyInterpolation::Linear) => "Linear",
            Some(KeyInterpolation::Hermite) => "Hermite",
            None => "-",
        }),
    );
    style_buttons::<AnimationAutoTangentButtonComponent>(
        &mut ent,
        if primary.is_some_and(|(_, k)| k.auto_tangent) {
            PAL.accent
        } else {
            PAL.surface_raised
        },
        None,
    );

    // timeline
    let (Some(content_rect), Some(ruler_rect)) = (
        ent.get_component::<GlobalRectComponent>(content).copied(),
        ent.get_component::<GlobalRectComponent>(ruler).copied(),
    ) else {
        return;
    };

    let tracks = curve_tracks(clip, window.selected_track, anim.as_ref());
    let view = TimelineView::new(&content_rect, &window.view);
    let track_count = clip.map_or(0, |c| c.tracks.len() as usize);
    let row_tracks = view.row_tracks(track_count);
    let to_content = |p: Vec2<f32>| (p - view.min) / view.size.map(|v| v.max(1.0));
    let ruler_min = ruler_rect.center - ruler_rect.scale * 0.5;
    let to_ruler_x = |x: f32| (x - ruler_min.x) / ruler_rect.scale.x.max(1.0);

    // track list, scrolled with the dope sheet
    for (row, ent_row) in track_rows.iter().enumerate() {
        let track_idx = window.view.first_row + row;
        let text = clip
            .and_then(|c| c.tracks.get(track_idx as u64))
            .map(|t| t.path.label(&track_component_label(world, t.path.component)))
            .unwrap_or_default();
        let color = if window.selected_track == Some(track_idx) {
            PAL.accent_fg
        } else {
            PAL.text_1
        };
        set_text(&mut ent, *ent_row, &text, color);
    }

    let step = ruler_step(&view);
    let mut grid = Vec::new();
    let mut ticks = Vec::new();
    let mut label_idx = 0;
    let mut time = (window.view.time_start.max(0.0) / step).ceil() * step;
    while time <= window.view.time_end {
        let x = view.x(time);
        grid.push(to_content(Vec2::new(x, view.min.y)));
        grid.push(to_content(Vec2::new(x, view.min.y + view.size.y)));
        ticks.push(Vec2::new(to_ruler_x(x), 0.6));
        ticks.push(Vec2::new(to_ruler_x(x), 1.0));

        if let Some(ent_label) = ruler_labels.get(label_idx as u64) {
            set_text(&mut ent, *ent_label, &format_number(time as f64), PAL.text_1);
            if let Some(rect_c) = ent.get_component_mut::<LocalRectComponent>(*ent_label) {
                rect_c.offset = Vec2::new(UiVal::px(x - ruler_min.x + 3.0), UiVal::px(0.0));
            }
            label_idx += 1;
        }
        time += step;
    }
    for ent_label in ruler_labels.iter().skip(label_idx) {
        set_text(&mut ent, *ent_label, "", PAL.text_1);
    }

    if window.tab == AnimationWindowTab::DopeSheet {
        for row in 1..=view.visible_rows() {
            let y = view.min.y + row as f32 * ANIMATION_ROW_HEIGHT;
            grid.push(to_content(Vec2::new(view.min.x, y)));
            grid.push(to_content(Vec2::new(view.min.x + view.size.x, y)));
        }
    }

    let half_key = Vec2::new(KEY_SIZE * 0.5, 0.0);
    let mut keys = Vec::new();
    let mut selected_keys = Vec::new();
    let mut curves = Vec::new();
    let mut selected_curve = Vec::new();
    let mut tangents = Vec::new();

    if let (Some(clip), Some(anim)) = (clip, anim.as_ref()) {
        let visible: Vec<usize> = match window.tab {
            AnimationWindowTab::DopeSheet => row_tracks.clone().collect(),
            AnimationWindowTab::Curves => tracks.clone(),
        };

        for &track_idx in &visible {
            let track = &clip.tracks[track_idx as u64];

            if window.tab == AnimationWindowTab::Curves {
                let target = if window.selected_track == Some(track_idx) {
                    &mut selected_curve
                } else {
                    &mut curves
                };
                let mut prev = None;
                for i in 0..=CURVE_SAMPLES {
                    let t = window.view.time_start + view.time_span() * i as f32 / CURVE_SAMPLES as f32;
                    continue_if_not!(Some(v) = sample_numeric(anim, &track.keys, t));
                    let p = to_content(Vec2::new(view.x(t), view.y(v)));
                    if let Some(prev) = prev {
                        target.push(prev);
                        target.push(p);
                    }
                    prev = Some(p);
                }
            }

            for (key_idx, key) in track.keys.iter().enumerate() {
                continue_if_not!(Some(pos) = view.key_position(window.tab, track_idx, key, anim));
                let is_selected = window.selected_keys.contains(&(track_idx, key_idx));
                let target = if is_selected { &mut selected_keys } else { &mut keys };
                target.push(to_content(pos - half_key));
                target.push(to_content(pos + half_key));

                if is_selected && window.tab == AnimationWindowTab::Curves && window.primary_key() == Some((track_idx, key_idx)) {
                    let out_handle = view.tangent_handle(pos, key.out_tangent);
                    let in_handle = pos * 2.0 - view.tangent_handle(pos, key.in_tangent);
                    for handle in [in_handle, out_handle] {
                        tangents.push(to_content(pos));
                        tangents.push(to_content(handle));
                        tangents.push(to_content(handle - Vec2::new(3.0, 0.0)));
                        tangents.push(to_content(handle + Vec2::new(3.0, 0.0)));
                    }
                }
            }
        }
    }

    let cursor_x = view.x(window.time);
    let cursor = vec![
        to_content(Vec2::new(cursor_x, view.min.y)),
        to_content(Vec2::new(cursor_x, view.min.y + view.size.y)),
    ];
    let ruler_cursor = vec![Vec2::new(to_ruler_x(cursor_x), 0.0), Vec2::new(to_ruler_x(cursor_x), 1.0)];

    set_points(&mut ent, lines.grid, grid);
    set_points(&mut ent, lines.ruler_ticks, ticks);
    set_points(&mut ent, lines.keys, keys);
    set_points(&mut ent, lines.selected_keys, selected_keys);
    set_points(&mut ent, lines.curves, curves);
    set_points(&mut ent, lines.selected_curve, selected_curve);
    set_points(&mut ent, lines.tangents, tangents);
    set_points(&mut ent, lines.cursor, cursor);
    set_points(&mut ent, lines.ruler_cursor, ruler_cursor);

    // scrollbar thumbs: the visible part of the scroll extents
    let extents = scroll_extents(&window.view, clip, &tracks, anim.as_ref());
    let (time_min, time_max) = extents.time;
    let h_range = (
        (window.view.time_start - time_min) / (time_max - time_min).max(1e-4),
        (window.view.time_end - time_min) / (time_max - time_min).max(1e-4),
    );
    let v_range = match window.tab {
        AnimationWindowTab::DopeSheet => {
            let total = track_count.max(view.visible_rows()).max(1) as f32;
            (
                row_tracks.start as f32 / total,
                (row_tracks.start + view.visible_rows()) as f32 / total,
            )
        }
        AnimationWindowTab::Curves => {
            let (value_min, value_max) = extents.values;
            let span = (value_max - value_min).max(1e-9);
            (
                ((value_max - window.view.value_max) / span) as f32,
                ((value_max - window.view.value_min) / span) as f32,
            )
        }
    };
    set_thumb(&mut ent, h_scroll, h_thumb, h_range, true);
    set_thumb(&mut ent, v_scroll, v_thumb, v_range, false);
}

fn set_thumb(ent: &mut EntitiesHolderMut, bar: EntityId, thumb: EntityId, (start, end): (f32, f32), is_horizontal: bool) {
    return_if_not!(Some(bar_rect) = ent.get_component::<GlobalRectComponent>(bar).copied());
    return_if_not!(Some(rect_c) = ent.get_component_mut::<LocalRectComponent>(thumb));

    let (start, end) = (start.clamp(0.0, 1.0), end.clamp(0.0, 1.0));
    let length = if is_horizontal { bar_rect.scale.x } else { bar_rect.scale.y };
    let (offset, size) = (start * length, ((end - start) * length).max(8.0));

    if is_horizontal {
        rect_c.offset = Vec2::new(UiVal::px(offset), UiVal::px(0.0));
        rect_c.scale = Vec2::new(UiVal::px(size).into(), UiVal::ph(1.0).into());
    } else {
        rect_c.offset = Vec2::new(UiVal::px(0.0), UiVal::px(offset));
        rect_c.scale = Vec2::new(UiVal::pw(1.0).into(), UiVal::px(size).into());
    }
}

pub(crate) fn clicked<C: 'static + Component>(q: &WorldQuery<&ButtonComponent, WithFilter<C>>) -> bool {
    q.iter().any(|b| b.was_clicked_this_frame)
}

pub fn update_animation_window_focus_system(
    press_res: Res<UiPressResource>,
    parent_q: WorldQuery<&ChildComponent>,
    panel_q: WorldQuery<&AnimationWindowComponent>,
    mut window: ResMut<AnimationWindowResource>,
) {
    return_if_not!(Some(pressed) = press_res.pressed_this_frame);

    window.is_focused =
        panel_q.get(pressed).is_some() || find_in_parents(parent_q, pressed, |e| panel_q.get(e).map(|_| ())).is_some();
}

pub fn copy_paste_animation_keys_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    mut window: ResMut<AnimationWindowResource>,
    mut clipboard: ResMut<EditorClipboardResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    open_project: Res<OpenProjectResource>,
) {
    let window = &mut *window;
    return_if!(!window.is_focused);

    let is_copy = is_editor_ctrl_hotkey_just_pressed(&input_res, &selected_input, &game_focus, KeyCode::KeyC);
    let is_paste = is_editor_ctrl_hotkey_just_pressed(&input_res, &selected_input, &game_focus, KeyCode::KeyV);
    return_if!(!is_copy && !is_paste);

    let is_playing = simulated_world.is_playing();
    return_if_not!(Some(world) = &mut simulated_world.0);
    let root = window.root;
    let anim = anim_types(world);

    if is_copy {
        return_if_not!(Some(clip) = clip_of(world, root));
        return_if_not!(Some(registry) = world.data.as_ref().resources().get::<ReflectionRegistryResource>());

        let selected = window
            .selected_keys
            .iter()
            .filter_map(|(t, k)| {
                let track = clip.tracks.get(*t as u64)?;
                Some((track, track.keys.get(*k as u64)?))
            })
            .collect::<Vec<_>>();
        let earliest = selected.iter().map(|(_, key)| key.time).fold(f32::INFINITY, f32::min);

        let copied = selected
            .into_iter()
            .filter_map(|(track, key)| {
                Some(CopiedAnimationKey {
                    path: track.path.clone(),
                    value_type: track.value_type.clone(),
                    time: key.time - earliest,
                    value: EditorValue(reflect_clone(registry, key.value.as_any_ref())?),
                    interpolation: key.interpolation,
                    in_tangent: key.in_tangent,
                    out_tangent: key.out_tangent,
                    auto_tangent: key.auto_tangent,
                })
            })
            .collect::<Vec<_>>();

        if !copied.is_empty() {
            clipboard.0 = Some(EditorClipboard::AnimationKeys(copied));
        }
        return;
    }

    return_if!(is_playing || clip_of(world, root).is_none());
    return_if_not!(Some(copied) = clipboard.animation_keys());

    let values = {
        return_if_not!(Some(registry) = world.data.as_ref().resources().get::<ReflectionRegistryResource>());
        copied
            .iter()
            .filter_map(|c| Some((c, reflect_clone(registry, c.value.0.as_any_ref())?)))
            .collect::<Vec<_>>()
    };
    let time = window.time;

    let pasted = edit_clip(world, root, &open_project, |clip| {
        let mut touched = Vec::new();

        for (copied, value) in values {
            let key_time = (time + copied.time).max(0.0);
            let (track, key) = clip.set_key(&anim, copied.path.clone(), copied.value_type, key_time, value);

            let k = &mut clip.tracks[track as u64].keys[key as u64];
            k.interpolation = copied.interpolation;
            k.in_tangent = copied.in_tangent;
            k.out_tangent = copied.out_tangent;
            k.auto_tangent = copied.auto_tangent;
            touched.push((copied.path.clone(), key_time));
        }

        // later keys can shift the indices of earlier ones, so look them all up at the end
        let mut selection = Vec::new();
        for (path, key_time) in touched {
            if let Some(track) = clip.track_index(&path) {
                clip.recompute_auto_tangents(&anim, track);
                if let Some(key) = clip.tracks[track as u64].key_index_at(key_time) {
                    selection.push((track, key));
                }
            }
        }
        selection
    });

    if let Some(selection) = pasted {
        window.selected_keys = selection;
    }
}
