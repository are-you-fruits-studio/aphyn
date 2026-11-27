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
    theme::{self, PAL},
    *,
};

const TOOLBAR_HEIGHT: f32 = theme::CONTROL_H;
const TOOLBAR_MARGIN: f32 = 4.0;
const TOOLBAR_GAP: f32 = 2.0;
const LIGHT_BUTTON_WIDTH: f32 = 52.0;

const PANEL_PADDING: f32 = 4.0;
const PANEL_ROW_HEIGHT: f32 = theme::ROW_H;
const PANEL_ROW_GAP: f32 = 2.0;
const PANEL_LABEL_WIDTH: f32 = 72.0;
const PANEL_FIELD_WIDTH: f32 = 56.0;
const PANEL_ROW_WIDTH: f32 = PANEL_LABEL_WIDTH + (PANEL_FIELD_WIDTH + PANEL_ROW_GAP) * 3.0;
const PANEL_ROWS: f32 = 5.0;
const SWITCH_WIDTH: f32 = 28.0;
const SWITCH_HEIGHT: f32 = 14.0;
const SWITCH_KNOB: f32 = 10.0;

pub fn init_window_as_scene(mut world: WorldDataMut, ent_window: EntityId) {
    let ent_image = init_window_as_view(world.as_mut(), ent_window, SceneViewImageComponent);

    let (res, mut ent, _) = world.as_tuple_mut();
    let assets_res = res.get::<StandardAssetsResource>().unwrap();

    let buttons = [
        ("None", 48.0, Some(GizmoTool::None)),
        ("Move", 48.0, Some(GizmoTool::Move)),
        ("Rotate", 60.0, Some(GizmoTool::Rotate)),
        ("Scale", 52.0, Some(GizmoTool::Scale)),
        ("Global", 60.0, None),
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

    spawn_view_hint(
        ent.as_mut(),
        assets_res,
        ent_image,
        "RMB look - WASD fly - double-click in Hierarchy to focus",
        1.0,
    );

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
                color: PAL.surface_overlay,
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

    let ent_title_row = spawn_panel_row(ent.as_mut(), ent_content);
    spawn_panel_text(
        ent.as_mut(),
        assets_res,
        ent_title_row,
        "SCENE LIGHT - EDITOR ONLY".into(),
        PANEL_ROW_WIDTH,
        PAL.text_3,
    );

    let ent_override_row = spawn_panel_row(ent.as_mut(), ent_content);
    spawn_panel_text(
        ent.as_mut(),
        assets_res,
        ent_override_row,
        "Override ambient".into(),
        PANEL_ROW_WIDTH - SWITCH_WIDTH - PANEL_ROW_GAP,
        PAL.text_2,
    );
    spawn_switch(
        ent.as_mut(),
        assets_res,
        ent_override_row,
        SceneAmbientOverrideButtonComponent,
    );

    let ent_shadows_row = spawn_panel_row(ent.as_mut(), ent_content);
    spawn_panel_text(
        ent.as_mut(),
        assets_res,
        ent_shadows_row,
        "Shadows".into(),
        PANEL_ROW_WIDTH - SWITCH_WIDTH - PANEL_ROW_GAP,
        PAL.text_2,
    );
    spawn_switch(ent.as_mut(), assets_res, ent_shadows_row, SceneShadowsButtonComponent);

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

// the track is the button; the light panel system colors it and moves the knob (its only child)
fn spawn_switch<C: 'static + Component>(
    mut ent: EntitiesHolderMut,
    assets_res: &StandardAssetsResource,
    ent_parent: EntityId,
    marker: C,
) {
    let ent_track = ent.create_entity();
    let ent_knob = ent.create_entity();

    ent.set_components(
        ent_track,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::px(SWITCH_WIDTH).into(), UiVal::px(SWITCH_HEIGHT).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            ButtonComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_panel.clone(),
            },
            ImageComponent {
                color: PAL.night_600,
                ..Default::default()
            },
            marker,
        ),
    );
    ent.set_components(
        ent_knob,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.5),
                pivot: Vec2::new(0.0, 0.5),
                offset: Vec2::new(UiVal::px(2.0), UiVal::px(0.0)),
                scale: Vec2::splat(UiVal::px(SWITCH_KNOB).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_panel.clone(),
            },
            ImageComponent {
                color: PAL.text_1,
                ..Default::default()
            },
        ),
    );
    set_parent(ent.as_mut(), ent_track, ent_parent);
    set_parent(ent.as_mut(), ent_knob, ent_track);
}

fn spawn_panel_label(ent: EntitiesHolderMut, assets_res: &StandardAssetsResource, ent_parent: EntityId, text: FfiString) {
    spawn_panel_text(ent, assets_res, ent_parent, text, PANEL_LABEL_WIDTH, PAL.text_2);
}

fn spawn_panel_text(
    mut ent: EntitiesHolderMut,
    assets_res: &StandardAssetsResource,
    ent_parent: EntityId,
    text: FfiString,
    width: f32,
    color: Vec4<f32>,
) {
    let ent_label = ent.create_entity();

    ent.set_components(
        ent_label,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::px(width).into(), UiVal::px(PANEL_ROW_HEIGHT).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_text.clone(),
            },
            TextComponent {
                color,
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

pub fn init_window_as_game(mut world: WorldDataMut, ent_window: EntityId) {
    let ent_image = init_window_as_view(world.as_mut(), ent_window, GameViewImageComponent);

    let (res, ent, _) = world.as_tuple_mut();
    let assets_res = res.get::<StandardAssetsResource>().unwrap();
    spawn_view_hint(ent, assets_res, ent_image, "Click the view to capture input", 0.0);
}

// a muted line in a view's corner: top (anchor_y 0) or bottom (anchor_y 1)
fn spawn_view_hint(mut ent: EntitiesHolderMut, assets_res: &StandardAssetsResource, ent_image: EntityId, text: &str, anchor_y: f32) {
    let ent_hint = ent.create_entity();

    ent.set_components(
        ent_hint,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, anchor_y),
                pivot: Vec2::new(0.0, anchor_y),
                offset: Vec2::new(UiVal::px(theme::PAD_X), UiVal::px(0.0)),
                scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(theme::ROW_H).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_text.clone(),
            },
            TextComponent {
                color: PAL.text_3,
                font: assets_res.font.clone(),
                font_size: UiVal::px(theme::FONT),
                horizontal_align: HorizontalAlign::Left,
                vertical_align: VerticalAlign::Middle,
                horizontal_spacing: UiVal::px(0.0),
                is_y_inverted: true,
                text: text.into(),
            },
        ),
    );
    set_parent(ent.as_mut(), ent_hint, ent_image);
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
