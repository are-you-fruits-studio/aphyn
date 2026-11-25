use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    components::{HierarchyWindowComponent, ProjectWindowComponent},
    features::{
        animation_window::AnimationWindowComponent,
        generic_dropdown::{GenericDropdownResource, select_generic_dropdown_system},
        inspector_window::data::InspectorWindowComponent,
        ui_interaction::ButtonComponent,
        ui_window::{attach_panel, find_group_of_panel, set_active_panel},
    },
    prefabs::{self, WindowComponent},
};

// todo: bundle registration params
pub fn register_feature(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(WindowsDropdownResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(open_windows_dropdown_system)
        .insert_child_system(open_windows_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(open_windows_dropdown_system)
        .before_system(open_windows_system);
}

#[derive(Component, Default, Debug)]
pub struct WindowsButtonComponent;

#[derive(Component, Default, Debug)]
pub struct SceneWindowComponent;

#[derive(Component, Default, Debug)]
pub struct GameWindowComponent;

#[derive(Resource, Default)]
pub struct WindowsDropdownResource {
    request_id: u64,
}

pub const WINDOW_KIND_PROJECT: &str = "Project Window";
pub const WINDOW_KIND_HIERARCHY: &str = "Hierarchy Window";
pub const WINDOW_KIND_INSPECTOR: &str = "Inspector Window";
pub const WINDOW_KIND_SCENE: &str = "Scene Window";
pub const WINDOW_KIND_GAME: &str = "Game Window";
pub const WINDOW_KIND_ANIMATION: &str = "Animation Window";

const WINDOW_KINDS: [&str; 6] = [
    WINDOW_KIND_PROJECT,
    WINDOW_KIND_HIERARCHY,
    WINDOW_KIND_INSPECTOR,
    WINDOW_KIND_SCENE,
    WINDOW_KIND_GAME,
    WINDOW_KIND_ANIMATION,
];

fn open_windows_dropdown_system(
    button_q: WorldQuery<(&ButtonComponent, &GlobalRectComponent), WithFilter<WindowsButtonComponent>>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut dropdown_res: ResMut<WindowsDropdownResource>,
) {
    let Some((_, rect_c)) = button_q.iter().find(|(b, _)| b.was_clicked_this_frame) else {
        return;
    };

    let position = rect_c.center + Vec2::new(-rect_c.scale.x, rect_c.scale.y) * 0.5;
    let variants = WINDOW_KINDS.iter().map(|k| FfiString::from(*k)).collect();
    dropdown_res.request_id = generic_dropdown_res.request(variants, position);
}

pub fn open_windows_system(mut world: WorldDataMut) {
    let mut res = world.resources_mut();
    let mut request_id = res.get::<WindowsDropdownResource>().unwrap().request_id;
    let idx = res.get::<GenericDropdownResource>().unwrap().take_selected(&mut request_id);
    res.get_mut::<WindowsDropdownResource>().unwrap().request_id = request_id;

    if let Some(idx) = idx {
        let kind = WINDOW_KINDS[idx as usize];

        if !focus_window_by_kind(world.as_mut(), kind) {
            open_window_by_kind(world, kind);
        }
    }
}

pub fn focus_window_by_kind(mut world: WorldDataMut, kind: &str) -> bool {
    let ent_panel = world
        .entities()
        .query::<(EntityId, &WindowComponent)>()
        .iter()
        .find(|(_, window_c)| window_c.kind.as_str() == kind)
        .map(|(ent, _)| ent);

    let Some(ent_panel) = ent_panel else {
        return false;
    };

    if let Some(ent_group) = find_group_of_panel(world.entities(), ent_panel) {
        set_active_panel(world.entities_mut(), ent_group, ent_panel);
    }

    true
}

pub fn open_window_by_kind(mut world: WorldDataMut, kind: &str) -> Option<EntityId> {
    let ent_panel = open_panel_by_kind(world.as_mut(), kind)?;
    let ent_group = prefabs::window_group(world.as_mut());

    attach_panel(world.entities_mut(), ent_group, ent_panel);

    Some(ent_group)
}

pub fn open_panel_by_kind(world: WorldDataMut, kind: &str) -> Option<EntityId> {
    match kind {
        WINDOW_KIND_PROJECT => open_specific_window(
            world,
            kind,
            ProjectWindowComponent::default(),
            prefabs::init_window_as_project,
        ),
        WINDOW_KIND_HIERARCHY => open_specific_window(
            world,
            kind,
            HierarchyWindowComponent::default(),
            prefabs::init_window_as_hierarchy,
        ),
        WINDOW_KIND_INSPECTOR => open_specific_window(
            world,
            kind,
            InspectorWindowComponent::default(),
            prefabs::init_window_as_inspector,
        ),
        WINDOW_KIND_SCENE => open_specific_window(world, kind, SceneWindowComponent, prefabs::init_window_as_scene),
        WINDOW_KIND_GAME => open_specific_window(world, kind, GameWindowComponent, prefabs::init_window_as_game),
        WINDOW_KIND_ANIMATION => open_specific_window(world, kind, AnimationWindowComponent, prefabs::init_window_as_animation),
        _ => None,
    }
}

fn open_specific_window<SpecWindowComponent: 'static + Component>(
    mut world: WorldDataMut,
    kind: &str,
    component: SpecWindowComponent,
    init: impl FnOnce(WorldDataMut, EntityId),
) -> Option<EntityId> {
    if !world.entities().query::<&SpecWindowComponent>().is_empty() {
        return None;
    }

    let ent_panel = prefabs::panel(world.as_mut(), kind);

    init(world.as_mut(), ent_panel);

    world.entities_mut().set_component(ent_panel, component);

    Some(ent_panel)
}
