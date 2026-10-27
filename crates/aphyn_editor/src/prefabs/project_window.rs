use crate::{components::ProjectWindowEmptyAreaComponent, prefabs::WindowComponent, *};

pub fn init_window_as_project(mut world: WorldDataMut, ent_window: EntityId) {
    let mut ent = world.entities_mut();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;
    let ent_scroll_view = ent.get_component::<ChildComponent>(ent_scroll_content).unwrap().parent;

    ent.set_components(ent_scroll_view, (ButtonComponent::default(), ProjectWindowEmptyAreaComponent));

    // in front of the scroll view, so entries win the ui raycast over the empty area button
    ent.get_component_mut::<LocalRectComponent>(ent_scroll_content).unwrap().z = -1.0;
}
