use crate::{
    features::{
        game_window::GameViewImageComponent,
        inspector_window::{FONT_SIZE, utils::entries::spawn_input_area_ent},
        scene_light::{
            SceneAmbientOverrideButtonComponent, SceneLightButtonComponent, SceneLightField, SceneLightFieldComponent,
            SceneLightPanelComponent, SceneLightSettings, SceneShadowsButtonComponent,
        },
        scene_window::SceneViewImageComponent,
        transform_gizmo::{GizmoSpaceButtonComponent, GizmoTool, GizmoToolButtonComponent},
        ui_interaction::HoverAreaComponent,
    },
    prefabs::{WindowComponent, create_header_button},
    *,
};

const TOOLBAR_HEIGHT: f32 = 22.0;
const TOOLBAR_MARGIN: f32 = 4.0;
const TOOLBAR_GAP: f32 = 1.0;
const LIGHT_BUTTON_WIDTH: f32 = 64.0;

const PANEL_PADDING: f32 = 4.0;
const PANEL_ROW_HEIGHT: f32 = 20.0;
const PANEL_ROW_GAP: f32 = 2.0;
const PANEL_LABEL_WIDTH: f32 = 72.0;
const PANEL_FIELD_WIDTH: f32 = 56.0;
const PANEL_ROW_WIDTH: f32 = PANEL_LABEL_WIDTH + (PANEL_FIELD_WIDTH + PANEL_ROW_GAP) * 3.0;
const PANEL_ROWS: f32 = 4.0;

pub fn init_window_as_scene(mut world: WorldDataMut, ent_window: EntityId) {
    let ent_image = init_window_as_view(world.as_mut(), ent_window, SceneViewImageComponent);

    let (res, mut ent, _) = world.as_tuple_mut();
    let assets_res = res.get::<StandardAssetsResource>().unwrap();

    let buttons = [
        ("None", 64.0, Some(GizmoTool::None)),
        ("Move", 64.0, Some(GizmoTool::Move)),
        ("Rotate", 90.0, Some(GizmoTool::Rotate)),
        ("Scale", 76.0, Some(GizmoTool::Scale)),
        ("Global", 90.0, None),
    ];
    let toolbar_width =
        buttons.iter().map(|(_, width, _)| width).sum::<f32>() + LIGHT_BUTTON_WIDTH + TOOLBAR_GAP * buttons.len() as f32;

    let ent_toolbar = ent.create_entity();

    ent.set_components(
        ent_toolbar,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                offset: Vec2::new(UiVal::px(TOOLBAR_MARGIN), UiVal::px(TOOLBAR_MARGIN)),
                scale: Vec2::new(UiVal::px(toolbar_width).into(), UiVal::px(TOOLBAR_HEIGHT).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(TOOLBAR_GAP),
                spacing: UiSpacing::Chunk,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_toolbar, ent_image);

    for (text, width, tool) in buttons {
        match tool {
            Some(tool) => create_header_button(
                ent.as_mut(),
                assets_res,
                ent_toolbar,
                GizmoToolButtonComponent { tool },
                text.into(),
                UiVal::px(width),
            ),
            None => create_header_button(
                ent.as_mut(),
                assets_res,
                ent_toolbar,
                GizmoSpaceButtonComponent,
                text.into(),
                UiVal::px(width),
            ),
        }
    }

    create_header_button(
        ent.as_mut(),
        assets_res,
        ent_toolbar,
        SceneLightButtonComponent,
        "Light".into(),
        UiVal::px(LIGHT_BUTTON_WIDTH),
    );
}

pub fn spawn_scene_light_panel(
    mut ent: EntitiesHolderMut,
    assets_res: &StandardAssetsResource,
    ent_image: EntityId,
    settings: &SceneLightSettings,
) {
    let ent_panel = ent.create_entity();
    let ent_content = ent.create_entity();

    let panel_height = PANEL_ROW_HEIGHT * PANEL_ROWS + PANEL_ROW_GAP * (PANEL_ROWS - 1.0) + PANEL_PADDING * 2.0;

    ent.set_components(
        ent_panel,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                offset: Vec2::new(
                    UiVal::px(TOOLBAR_MARGIN),
                    UiVal::px(TOOLBAR_MARGIN + TOOLBAR_HEIGHT + TOOLBAR_GAP),
                ),
                scale: Vec2::new(
                    UiVal::px(PANEL_ROW_WIDTH + PANEL_PADDING * 2.0).into(),
                    UiVal::px(panel_height).into(),
                ),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::new(0.15, 0.15, 0.15, 1.0),
                ..Default::default()
            },
            HoverAreaComponent::default(),
            SceneLightPanelComponent,
        ),
    );
    set_parent(ent.as_mut(), ent_panel, ent_image);

    ent.set_components(
        ent_content,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::splat(UiVal::px(PANEL_PADDING)),
                parent_padding_max: Vec2::splat(UiVal::px(PANEL_PADDING)),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(PANEL_ROW_GAP),
                spacing: UiSpacing::Chunk,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_content, ent_panel);

    let ent_override_row = spawn_panel_row(ent.as_mut(), ent_content);
    create_header_button(
        ent.as_mut(),
        assets_res,
        ent_override_row,
        SceneAmbientOverrideButtonComponent,
        "Override ambient".into(),
        UiVal::px(PANEL_ROW_WIDTH),
    );

    let ent_shadows_row = spawn_panel_row(ent.as_mut(), ent_content);
    create_header_button(
        ent.as_mut(),
        assets_res,
        ent_shadows_row,
        SceneShadowsButtonComponent,
        "Shadows".into(),
        UiVal::px(PANEL_ROW_WIDTH),
    );

    for (label, field, values) in [
        ("Ambient", SceneLightField::AmbientLight, settings.ambient_light),
        ("Rotation", SceneLightField::DefaultLightEuler, settings.default_light_euler),
    ] {
        let ent_row = spawn_panel_row(ent.as_mut(), ent_content);
        spawn_panel_label(ent.as_mut(), assets_res, ent_row, label.into());

        for (axis, value) in values.into_array().into_iter().enumerate() {
            let ent_container = ent.create_entity();

            ent.set_components(
                ent_container,
                (
                    GlobalRectComponent::default(),
                    GlobalDisableableComponent::default(),
                    LocalRectComponent {
                        scale: Vec2::new(UiVal::px(PANEL_FIELD_WIDTH).into(), UiVal::px(PANEL_ROW_HEIGHT).into()),
                        ..Default::default()
                    },
                    ParentComponent::default(),
                ),
            );
            set_parent(ent.as_mut(), ent_container, ent_row);

            let ent_field = spawn_input_area_ent(
                ent.as_mut(),
                ent_container,
                value.to_string().into(),
                assets_res.material_panel.clone(),
                assets_res.material_text.clone(),
                assets_res.font.clone(),
            );
            ent.add_component(ent_field, SceneLightFieldComponent { field, axis })
                .ok()
                .unwrap();
        }
    }
}

fn spawn_panel_row(mut ent: EntitiesHolderMut, ent_parent: EntityId) -> EntityId {
    let ent_row = ent.create_entity();

    ent.set_components(
        ent_row,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::px(PANEL_ROW_WIDTH).into(), UiVal::px(PANEL_ROW_HEIGHT).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(PANEL_ROW_GAP),
                spacing: UiSpacing::Chunk,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_row, ent_parent);

    ent_row
}

fn spawn_panel_label(mut ent: EntitiesHolderMut, assets_res: &StandardAssetsResource, ent_parent: EntityId, text: FfiString) {
    let ent_label = ent.create_entity();

    ent.set_components(
        ent_label,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::px(PANEL_LABEL_WIDTH).into(), UiVal::px(PANEL_ROW_HEIGHT).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_text.clone(),
            },
            TextComponent {
                color: Vec4::splat(1.0),
                font: assets_res.font.clone(),
                font_size: FONT_SIZE,
                horizontal_align: HorizontalAlign::Left,
                vertical_align: VerticalAlign::Middle,
                horizontal_spacing: UiVal::px(0.0),
                is_y_inverted: true,
                text,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_label, ent_parent);
}

pub fn init_window_as_game(world: WorldDataMut, ent_window: EntityId) {
    init_window_as_view(world, ent_window, GameViewImageComponent);
}

fn init_window_as_view<C: 'static + Component>(mut world: WorldDataMut, ent_window: EntityId, view_image_c: C) -> EntityId {
    let mut ent = world.entities_mut();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;
    let ent_scroll_view = ent.get_component::<ChildComponent>(ent_scroll_content).unwrap().parent();

    let ent_image = ent.create_entity();

    ent.set_components(
        ent_image,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent::default(),
            ImageComponent::default(),
            HoverAreaComponent::default(),
            view_image_c,
        ),
    );
    set_parent(ent.as_mut(), ent_image, ent_scroll_view);

    ent_image
}
