use fruits_engine::*;

use crate::{SYSTEM_GROUP, components::{HierarchyWindowComponent, ProjectWindowComponent}, features::{inspector_window::data::InspectorWindowComponent, ui_interaction::{ButtonComponent, check_ui_interaction_system}}, prefabs::{self, WindowComponent}};

pub fn register_feature(mut world: WorldBuilderMut) {
    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(open_windows_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(open_windows_system);
}

#[derive(Component, Default, Debug)]
pub struct TestWindowButtonComponent;

#[derive(Component, Default, Debug)]
pub struct ProjectWindowButtonComponent;

#[derive(Component, Default, Debug)]
pub struct HierarchyWindowButtonComponent;

#[derive(Component, Default, Debug)]
pub struct InspectorWindowButtonComponent;

#[derive(Component, Default, Debug)]
pub struct TestWindowComponent;

pub const WINDOW_KIND_TEST: &str = "Test Window";
pub const WINDOW_KIND_PROJECT: &str = "Project Window";
pub const WINDOW_KIND_HIERARCHY: &str = "Hierarchy Window";
pub const WINDOW_KIND_INSPECTOR: &str = "Inspector Window";

pub fn open_windows_system(mut world: WorldDataMut) {
    open_clicked_window::<TestWindowButtonComponent>(world.as_mut(), WINDOW_KIND_TEST);
    open_clicked_window::<ProjectWindowButtonComponent>(world.as_mut(), WINDOW_KIND_PROJECT);
    open_clicked_window::<HierarchyWindowButtonComponent>(world.as_mut(), WINDOW_KIND_HIERARCHY);
    open_clicked_window::<InspectorWindowButtonComponent>(world.as_mut(), WINDOW_KIND_INSPECTOR);
}

fn open_clicked_window<SpecButtonComponent: 'static + Component>(world: WorldDataMut, kind: &str) {
    let did_click = world
        .entities()
        .query_filtered::<&ButtonComponent, WithFilter<SpecButtonComponent>>()
        .iter()
        .any(|b| b.was_clicked_this_frame);

    if did_click {
        open_window_by_kind(world, kind);
    }
}

pub fn open_window_by_kind(world: WorldDataMut, kind: &str) -> Option<EntityId> {
    match kind {
        WINDOW_KIND_TEST => open_specific_window(world, kind, TestWindowComponent, |_, _| {}),
        WINDOW_KIND_PROJECT => open_specific_window(world, kind, ProjectWindowComponent::default(), |_, _| {}),
        WINDOW_KIND_HIERARCHY => open_specific_window(world, kind, HierarchyWindowComponent::default(), |_, _| {}),
        WINDOW_KIND_INSPECTOR => {
            open_specific_window(world, kind, InspectorWindowComponent::default(), prefabs::init_window_as_inspector)
        }
        _ => None,
    }
}

fn open_specific_window<SpecWindowComponent: 'static + Component>(
    mut world: WorldDataMut,
    title: &str,
    component: SpecWindowComponent,
    init: impl FnOnce(WorldDataMut, EntityId),
) -> Option<EntityId> {
    if !world.entities().query::<&SpecWindowComponent>().is_empty() {
        return None;
    }

    let ent_window = prefabs::window(world.as_mut());

    init(world.as_mut(), ent_window);

    let mut ent = world.entities_mut();

    ent.set_component(ent_window, component);

    let window_c = *ent.get_component_mut::<WindowComponent>(ent_window).unwrap();

    let text_c = ent.get_component_mut::<TextComponent>(window_c.title).unwrap();
    text_c.text.clear();
    text_c.text.push_str(title);

    Some(ent_window)
}