use crate::{
    components::{HierarchyWindowContentComponent, HierarchyWindowEmptyAreaComponent},
    prefabs::WindowComponent,
    *,
};

pub fn init_window_as_hierarchy(mut world: WorldDataMut, ent_window: EntityId) {
    let mut ent = world.entities_mut();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;
    let ent_scroll_view = ent.get_component::<ChildComponent>(ent_scroll_content).unwrap().parent;

    ent.set_components(ent_scroll_view, (ButtonComponent::default(), HierarchyWindowEmptyAreaComponent));

    // in front of the scroll view, so entries win the ui raycast over the empty area button
    ent.get_component_mut::<LocalRectComponent>(ent_scroll_content).unwrap().z = -1.0;

    let ent_entries_container = ent.create_entity();

    ent.get_component_mut::<ParentComponent>(ent_scroll_content).unwrap().children.push(ent_entries_container);

    ent.set_components(ent_entries_container, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
            ..Default::default()
        },
        ChildComponent { parent: ent_scroll_content },
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
            ..Default::default()
        },
        ParentComponent { children: vec![].into() },
    ));

    ent.set_component(ent_scroll_content, HierarchyWindowContentComponent {
        entries_container: ent_entries_container,
    });
}
