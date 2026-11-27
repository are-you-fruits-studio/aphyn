use crate::{
    features::{
        input_field::InputFieldComponent,
        inspector_window::{
            FIELD_HEIGHT,
            data::{AddComponentInputComponent, InspectorWindowContentComponent},
            utils::entries::spawn_input_area_ent,
        },
    },
    prefabs::WindowComponent,
    theme::{self, PAL},
    *,
};

// todo: move
pub fn init_window_as_inspector(mut world: WorldDataMut, ent_window: EntityId) {
    let (res, mut ent, _evt) = world.as_tuple_mut();

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
    let ent_add_component_placeholder = ent.create_entity();

    ent.set_components(
        ent_add_component_input,
        (AddComponentInputComponent {
            variants_container: ent_add_component_variants_container,
            placeholder: ent_add_component_placeholder,
        },),
    );

    ent.set_components(
        ent_add_component_variants_container,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                offset: Vec2::new(UiVal::px(0.0), FIELD_HEIGHT),
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
                z: -100.0,
                ..Default::default()
            },
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(0.0),
                spacing: UiSpacing::Chunk,
                ..Default::default()
            },
            ParentComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.surface_panel,
                ..Default::default()
            },
        ),
    );
    set_parent(ent.as_mut(), ent_add_component_variants_container, ent_add_component_input);

    let ent_add_component_text = ent
        .get_component::<InputFieldComponent>(ent_add_component_input)
        .unwrap()
        .text;
    let ent_add_component_background = ent.get_component::<ChildComponent>(ent_add_component_text).unwrap().parent();

    ent.set_components(
        ent_add_component_placeholder,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalDisableableComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::new(UiVal::px(theme::PAD_X), UiVal::px(0.0)),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: PAL.text_3,
                font: font.clone(),
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: "+ Add component…".into(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Left,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_add_component_placeholder, ent_add_component_background);

    for e in [
        ent_add_component_input,
        ent_add_component_background,
        ent_add_component_text,
        ent_add_component_variants_container,
    ] {
        ent.set_components(
            e,
            (LocalDisableableComponent::default(), GlobalDisableableComponent::default()),
        );
    }

    let ent_scroll_content_asset_type = ent.create_entity();
    let ent_header_badge = ent.create_entity();
    let ent_play_banner = ent.create_entity();
    let ent_play_banner_text = ent.create_entity();
    let ent_scroll_content_asset_container = ent.create_entity();

    ent.set_component(
        ent_scroll_content,
        InspectorWindowContentComponent {
            asset_type_text: ent_scroll_content_asset_type,
            badge_text: ent_header_badge,
            play_banner: ent_play_banner,
            content_container: ent_scroll_content_asset_container,
        },
    );

    ent.set_components(
        ent_scroll_content_asset_type,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(theme::PANEL_HEADER_H).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: PAL.text_1,
                font: font.clone(),
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: String::new().into(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Left,
            },
            ParentComponent::default(),
        ),
    );

    let text = |color: Vec4<f32>, align: HorizontalAlign, text: &str| {
        (
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color,
                font: font.clone(),
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: text.into(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: align,
            },
        )
    };

    ent.set_components(
        ent_header_badge,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                parent_padding_max: Vec2::new(UiVal::px(theme::PAD_X), UiVal::px(0.0)),
                ..Default::default()
            },
        ),
    );
    ent.set_components(ent_header_badge, text(PAL.text_3, HorizontalAlign::Right, ""));
    ent.set_components(
        ent_header_badge,
        (
            MaterialComponent {
                material: assets_res.material_text_mono.clone(),
            },
            TextComponent {
                color: PAL.text_3,
                font: assets_res.font_mono.clone(),
                font_size: UiVal::px(theme::FONT_MONO),
                is_y_inverted: true,
                text: FfiString::new(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Right,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_header_badge, ent_scroll_content_asset_type);

    ent.set_components(
        ent_play_banner,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalDisableableComponent { is_disabled: true },
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(0.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.warning_soft_on_panel,
                ..Default::default()
            },
        ),
    );
    ent.set_components(
        ent_play_banner_text,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::new(UiVal::px(theme::PAD_X), UiVal::px(0.0)),
                ..Default::default()
            },
        ),
    );
    ent.set_components(
        ent_play_banner_text,
        text(PAL.warning, HorizontalAlign::Left, "Play mode: edits here are not saved"),
    );
    set_parent(ent.as_mut(), ent_play_banner_text, ent_play_banner);

    ent.set_components(
        ent_scroll_content_asset_container,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
                ..Default::default()
            },
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(0.0),
                spacing: UiSpacing::Chunk,
                ..Default::default()
            },
            ParentComponent::default(),
        ),
    );

    set_parent(ent.as_mut(), ent_scroll_content_asset_type, ent_scroll_content);
    set_parent(ent.as_mut(), ent_play_banner, ent_scroll_content);
    set_parent(ent.as_mut(), ent_scroll_content_asset_container, ent_scroll_content);
}
