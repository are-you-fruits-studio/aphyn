use crate::{
    components::{HierarchyButtonAddComponent, HierarchyButtonRemoveComponent, HierarchyWindowContentComponent},
    features::inspector_window::utils::entries::{spawn_default_layout_ent, spawn_text_ent},
    prefabs::WindowComponent,
    *,
};

pub fn init_window_as_hierarchy(mut world: WorldDataMut, ent_window: EntityId) {
    let (res, mut ent, _) = world.as_tuple_mut();

    let assets_res = res.as_ref().get::<StandardAssetsResource>().unwrap();
    let font = assets_res.font.clone();
    let material_text = assets_res.material_text.clone();
    let material_panel = assets_res.material_panel.clone();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;

    let ent_buttons = spawn_default_layout_ent(ent.as_mut(), ent_scroll_content, false);
    ent.get_component_mut::<RectChildAlignComponent>(ent_buttons).unwrap().direction = UiDirection::Horizontal;

    let (ent_btn_add, ent_btn_add_text) = spawn_text_ent(
        ent.as_mut(),
        ent_buttons,
        "+".into(),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );
    let (ent_btn_remove, ent_btn_remove_text) = spawn_text_ent(
        ent.as_mut(),
        ent_buttons,
        "-".into(),
        material_panel,
        material_text,
        font,
    );

    for (ent_btn, ent_btn_text) in [(ent_btn_add, ent_btn_add_text), (ent_btn_remove, ent_btn_remove_text)] {
        ent.get_component_mut::<LocalRectComponent>(ent_btn).unwrap().scale.x = UiVal::pd(0.5).into();
        ent.set_component(ent_btn, ButtonComponent::default());
        ent.get_component_mut::<TextComponent>(ent_btn_text).unwrap().horizontal_align = HorizontalAlign::Middle;
    }
    ent.set_component(ent_btn_add, HierarchyButtonAddComponent);
    ent.set_component(ent_btn_remove, HierarchyButtonRemoveComponent);

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
