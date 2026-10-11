use crate::{
    features::inspector_window::{
        FIELD_HEIGHT, data::{AddComponentInputComponent, InspectorWindowContentComponent}, utils::entries::spawn_input_area_ent,
    }, prefabs::WindowComponent, *,
};

// todo: move
pub fn init_window_as_inspector(mut world: WorldDataMut, ent_window: EntityId) {
    let (res, mut ent, evt) = world.as_tuple_mut();

    let assets_res = res.as_ref().get::<StandardAssetsResource>().unwrap();
    let font = assets_res.font.clone();
    let material_text = res.as_ref().get::<StandardAssetsResource>().unwrap().material_text.clone();
    let material_panel = assets_res.material_panel.clone();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;

    let ent_add_component_input = spawn_input_area_ent(
        ent.as_mut(),
        ent_scroll_content,
        "".into(),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );
    let ent_add_component_variants_container = ent.create_entity();

    ent.set_components(ent_add_component_input, (
        ParentComponent { children: vec![ent_add_component_variants_container].into() },
        AddComponentInputComponent { variants_container: ent_add_component_variants_container },
    ));

    ent.set_components(ent_add_component_variants_container, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            offset: Vec2::new(UiVal::px(0.0), FIELD_HEIGHT),
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
            z: -100.0,
            ..Default::default()
        },
        ChildComponent { parent: ent_add_component_input },
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
            ..Default::default()
        },
        ParentComponent { children: vec![].into() },
        BatchedMeshComponent::default(),
        StandardMaterialComponent { material: material_panel },
        ImageComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#757575ff").unwrap()),
            ..Default::default()
        },
    ));

    let ent_scroll_content_asset_type = ent.create_entity();
    let ent_scroll_content_asset_container = ent.create_entity();

    let parent_c = ent.get_component_mut::<ParentComponent>(ent_scroll_content).unwrap();
    parent_c.children.push(ent_scroll_content_asset_type);
    parent_c.children.push(ent_scroll_content_asset_container);

    ent.set_component(ent_scroll_content, InspectorWindowContentComponent {
        asset_type_text: ent_scroll_content_asset_type,
        content_container: ent_scroll_content_asset_container,
    });

    ent.set_components(ent_scroll_content_asset_type, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(20.0).into()),
            ..Default::default()
        },
        ChildComponent {
            parent: ent_scroll_content,
        },
        BatchedMeshComponent::default(),
        StandardMaterialComponent { material: material_text },
        TextComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
            font,
            font_size: UiVal::px(18.0),
            is_y_inverted: true,
            text: String::new().into(),
            horizontal_spacing: UiVal::px(0.0),
            vertical_align: VerticalAlign::Middle,
            horizontal_align: HorizontalAlign::Left,
        },
        ParentComponent { children: vec![].into() },
    ));

    ent.set_components(ent_scroll_content_asset_container, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
            ..Default::default()
        },
        ChildComponent {
            parent: ent_scroll_content,
        },
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
            ..Default::default()
        },
        ParentComponent { children: vec![].into() },
    ));
}