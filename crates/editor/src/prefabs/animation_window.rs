use crate::{
    features::{
        animation_window::*,
        inspector_window::{FONT_SIZE, utils::entries::spawn_input_area_ent},
        ui_interaction::HoverAreaComponent,
    },
    prefabs::WindowComponent,
    *,
};

pub const ANIMATION_TOOLBAR_HEIGHT: f32 = 22.0;
pub const ANIMATION_MARGIN: f32 = 4.0;
pub const ANIMATION_GAP: f32 = 2.0;
pub const ANIMATION_TRACK_LIST_WIDTH: f32 = 260.0;
pub const ANIMATION_RULER_HEIGHT: f32 = 18.0;
pub const ANIMATION_ROW_HEIGHT: f32 = 18.0;
pub const ANIMATION_SCROLLBAR_SIZE: f32 = 10.0;
pub const ANIMATION_TRACK_ROWS: usize = 48;
pub const ANIMATION_RULER_LABELS: usize = 16;

const FIELD_WIDTH: f32 = 72.0;
const LABEL_WIDTH: f32 = 44.0;
const SMALL_FONT_SIZE: UiVal = UiVal::px(12.0);
const BUTTON_FONT_PX: f32 = 14.0;
// glyph width / height of the editor font
const GLYPH_RATIO: f32 = 0.7;

pub fn init_window_as_animation(mut world: WorldDataMut, ent_window: EntityId) {
    let (res, mut ent, _) = world.as_tuple_mut();
    let assets = res.get::<StandardAssetsResource>().unwrap();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;
    let ent_scroll_view = ent.get_component::<ChildComponent>(ent_scroll_content).unwrap().parent();

    let ent_root = ent.create_entity();
    ent.set_components(
        ent_root,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent::default(),
            ParentComponent::default(),
        ),
    );
    set_parent(ent.as_mut(), ent_root, ent_scroll_view);

    // toolbar
    let ent_toolbar = spawn_row(ent.as_mut(), ent_root, ANIMATION_MARGIN);
    spawn_button(
        ent.as_mut(),
        assets,
        ent_toolbar,
        AnimationPreviewButtonComponent,
        "Preview",
        "Preview",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_toolbar,
        AnimationRecordButtonComponent,
        "Record",
        "Record",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_toolbar,
        AnimationPlayButtonComponent,
        "Play",
        "Pause",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_toolbar,
        AnimationTabButtonComponent,
        "Curves",
        "Dope sheet",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_toolbar,
        AnimationFrameButtonComponent,
        "Frame",
        "Frame",
    );
    spawn_label(ent.as_mut(), assets, ent_toolbar, "Time".into(), LABEL_WIDTH, FONT_SIZE);
    let time_field = spawn_field(ent.as_mut(), assets, ent_toolbar, AnimationField::Time);
    let status_text = spawn_label(ent.as_mut(), assets, ent_toolbar, "".into(), 600.0, FONT_SIZE);

    // selected key
    let ent_key_row = spawn_row(
        ent.as_mut(),
        ent_root,
        ANIMATION_MARGIN + ANIMATION_TOOLBAR_HEIGHT + ANIMATION_GAP,
    );
    let mut key_fields = Vec::new();
    for (label, field) in [
        ("Key", AnimationField::KeyTime),
        ("Value", AnimationField::KeyValue),
        ("In", AnimationField::KeyInTangent),
        ("Out", AnimationField::KeyOutTangent),
    ] {
        spawn_label(ent.as_mut(), assets, ent_key_row, label.into(), LABEL_WIDTH, FONT_SIZE);
        key_fields.push(spawn_field(ent.as_mut(), assets, ent_key_row, field));
    }
    spawn_button(
        ent.as_mut(),
        assets,
        ent_key_row,
        AnimationInterpolationButtonComponent,
        "Hermite",
        "Constant",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_key_row,
        AnimationAutoTangentButtonComponent,
        "Auto",
        "Auto",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_key_row,
        AnimationDeleteButtonComponent,
        "Delete keys",
        "Delete keys",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_key_row,
        AnimationDeleteTrackButtonComponent,
        "Delete track",
        "Delete track",
    );

    // body: track list on the left, ruler + timeline on the right, scrollbars along the timeline
    let body_top = ANIMATION_MARGIN + (ANIMATION_TOOLBAR_HEIGHT + ANIMATION_GAP) * 2.0;
    let content_top = body_top + ANIMATION_RULER_HEIGHT;
    let bar = ANIMATION_SCROLLBAR_SIZE;

    let track_list = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::new(0.0, content_top),
        Vec2::new(0.0, bar),
        Some(ANIMATION_TRACK_LIST_WIDTH),
        Vec4::new(0.15, 0.15, 0.15, 1.0),
    );
    ent.set_component(
        track_list,
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
        },
    );

    let mut track_rows = FfiVec::new();
    for idx in 0..ANIMATION_TRACK_ROWS {
        let ent_row = spawn_text(ent.as_mut(), assets, track_list, "".into(), SMALL_FONT_SIZE);
        ent.set_components(
            ent_row,
            (
                LocalRectComponent {
                    scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(ANIMATION_ROW_HEIGHT).into()),
                    ..Default::default()
                },
                ButtonComponent::default(),
                AnimationTrackRowComponent { idx },
            ),
        );
        track_rows.push(ent_row);
    }

    let timeline_left = ANIMATION_TRACK_LIST_WIDTH + ANIMATION_GAP;
    let ruler = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::new(timeline_left, body_top),
        Vec2::new(bar, 0.0),
        None,
        Vec4::new(0.2, 0.2, 0.2, 1.0),
    );
    ent.set_component(
        ruler,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(timeline_left), UiVal::px(body_top)),
            parent_padding_max: Vec2::new(UiVal::px(bar), UiVal::px(0.0)),
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(ANIMATION_RULER_HEIGHT).into()),
            ..Default::default()
        },
    );

    let content = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::new(timeline_left, content_top),
        Vec2::new(bar, bar),
        None,
        Vec4::new(0.11, 0.11, 0.11, 1.0),
    );

    let scrollbar_color = Vec4::new(0.17, 0.17, 0.17, 1.0);
    let h_scroll = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::splat(0.0),
        Vec2::splat(0.0),
        None,
        scrollbar_color,
    );
    ent.set_component(
        h_scroll,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(timeline_left), UiVal::px(0.0)),
            parent_padding_max: Vec2::new(UiVal::px(bar), UiVal::px(0.0)),
            anchor: Vec2::new(0.0, 1.0),
            pivot: Vec2::new(0.0, 1.0),
            scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(bar).into()),
            ..Default::default()
        },
    );
    let v_scroll = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::splat(0.0),
        Vec2::splat(0.0),
        None,
        scrollbar_color,
    );
    ent.set_component(
        v_scroll,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(0.0), UiVal::px(content_top)),
            parent_padding_max: Vec2::new(UiVal::px(0.0), UiVal::px(bar)),
            anchor: Vec2::new(1.0, 0.0),
            pivot: Vec2::new(1.0, 0.0),
            scale: Vec2::new(UiVal::px(bar).into(), UiVal::ph(1.0).into()),
            ..Default::default()
        },
    );
    let h_thumb = spawn_thumb(ent.as_mut(), assets, h_scroll);
    let v_thumb = spawn_thumb(ent.as_mut(), assets, v_scroll);

    let mut ruler_labels = FfiVec::new();
    for _ in 0..ANIMATION_RULER_LABELS {
        let ent_label = spawn_text(ent.as_mut(), assets, ruler, "".into(), SMALL_FONT_SIZE);
        ent.set_component(
            ent_label,
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                scale: Vec2::new(UiVal::px(48.0).into(), UiVal::ph(1.0).into()),
                ..Default::default()
            },
        );
        ruler_labels.push(ent_label);
    }

    let ruler_ticks = spawn_lines(ent.as_mut(), assets, ruler, Vec4::new(0.55, 0.55, 0.55, 1.0), 1.0);
    let ruler_cursor = spawn_lines(ent.as_mut(), assets, ruler, CURSOR_COLOR, 2.0);

    let lines = AnimationLines {
        grid: spawn_lines(ent.as_mut(), assets, content, Vec4::new(0.2, 0.2, 0.2, 1.0), 1.0),
        curves: spawn_lines(ent.as_mut(), assets, content, Vec4::new(0.45, 0.6, 0.45, 1.0), 1.5),
        selected_curve: spawn_lines(ent.as_mut(), assets, content, Vec4::new(0.5, 0.95, 0.5, 1.0), 2.0),
        tangents: spawn_lines(ent.as_mut(), assets, content, Vec4::new(0.85, 0.85, 0.35, 1.0), 1.0),
        keys: spawn_lines(ent.as_mut(), assets, content, Vec4::new(0.8, 0.8, 0.8, 1.0), KEY_SIZE),
        selected_keys: spawn_lines(ent.as_mut(), assets, content, Vec4::new(1.0, 0.75, 0.2, 1.0), KEY_SIZE),
        cursor: spawn_lines(ent.as_mut(), assets, content, CURSOR_COLOR, 3.0),
        ruler_ticks,
        ruler_cursor,
    };

    ent.set_component(
        ent_root,
        AnimationWindowUiComponent {
            status_text,
            time_field,
            key_fields: [key_fields[0], key_fields[1], key_fields[2], key_fields[3]],
            track_list,
            track_rows,
            ruler,
            ruler_labels,
            content,
            h_scroll,
            h_thumb,
            v_scroll,
            v_thumb,
            lines,
        },
    );
}

// sized for the longest text the button shows
fn spawn_button<C: 'static>(
    mut ent: EntitiesHolderMut,
    assets: &StandardAssetsResource,
    ent_parent: EntityId,
    component: C,
    text: &str,
    longest_text: &str,
) {
    let width = longest_text.len().max(text.len()) as f32 * BUTTON_FONT_PX * GLYPH_RATIO + 14.0;

    let ent_button = ent.create_entity();
    ent.set_components(
        ent_button,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::px(width).into(), UiVal::pd(1.0).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            ButtonComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets.material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::new(0.3, 0.3, 0.3, 1.0),
                ..Default::default()
            },
            component,
        ),
    );
    set_parent(ent.as_mut(), ent_button, ent_parent);

    let ent_text = spawn_text(ent.as_mut(), assets, ent_button, text.into(), UiVal::px(BUTTON_FONT_PX));
    if let Some(text_c) = ent.get_component_mut::<TextComponent>(ent_text) {
        text_c.horizontal_align = HorizontalAlign::Middle;
    }
}

fn spawn_thumb(mut ent: EntitiesHolderMut, assets: &StandardAssetsResource, ent_parent: EntityId) -> EntityId {
    let ent_thumb = ent.create_entity();

    ent.set_components(
        ent_thumb,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                scale: Vec2::new(UiVal::px(0.0).into(), UiVal::px(0.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets.material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::new(0.4, 0.4, 0.4, 1.0),
                ..Default::default()
            },
        ),
    );
    set_parent(ent.as_mut(), ent_thumb, ent_parent);

    ent_thumb
}

fn spawn_row(mut ent: EntitiesHolderMut, ent_parent: EntityId, top: f32) -> EntityId {
    let ent_row = ent.create_entity();

    ent.set_components(
        ent_row,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                offset: Vec2::new(UiVal::px(ANIMATION_MARGIN), UiVal::px(top)),
                scale: Vec2::new(UiVal::px(2000.0).into(), UiVal::px(ANIMATION_TOOLBAR_HEIGHT).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(ANIMATION_GAP),
                spacing: UiSpacing::Chunk,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_row, ent_parent);

    ent_row
}

// a background area filling the parent minus `padding_min` / `padding_max`, or `width` wide from the left
fn spawn_area(
    mut ent: EntitiesHolderMut,
    assets: &StandardAssetsResource,
    ent_parent: EntityId,
    padding_min: Vec2<f32>,
    padding_max: Vec2<f32>,
    width: Option<f32>,
    color: Vec4<f32>,
) -> EntityId {
    let ent_area = ent.create_entity();

    let mut rect = LocalRectComponent {
        parent_padding_min: padding_min.map(UiVal::px),
        parent_padding_max: padding_max.map(UiVal::px),
        ..Default::default()
    };
    if let Some(width) = width {
        rect.anchor = Vec2::new(0.0, 0.0);
        rect.pivot = Vec2::new(0.0, 0.0);
        rect.scale = Vec2::new(UiVal::px(width).into(), UiVal::ph(1.0).into());
    }

    ent.set_components(
        ent_area,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            rect,
            ParentComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets.material_panel.clone(),
            },
            ImageComponent {
                color,
                ..Default::default()
            },
            HoverAreaComponent::default(),
            ChildrenRectMaskComponent,
        ),
    );
    set_parent(ent.as_mut(), ent_area, ent_parent);

    ent_area
}

fn spawn_lines(
    mut ent: EntitiesHolderMut,
    assets: &StandardAssetsResource,
    ent_parent: EntityId,
    color: Vec4<f32>,
    thickness_px: f32,
) -> EntityId {
    let ent_lines = ent.create_entity();

    ent.set_components(
        ent_lines,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets.material_panel.clone(),
            },
            UiLineComponent {
                points: FfiVec::new(),
                thickness_px,
                is_segments: true,
                color,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_lines, ent_parent);

    ent_lines
}

fn spawn_text(
    mut ent: EntitiesHolderMut,
    assets: &StandardAssetsResource,
    ent_parent: EntityId,
    text: FfiString,
    font_size: UiVal,
) -> EntityId {
    let ent_text = ent.create_entity();

    ent.set_components(
        ent_text,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets.material_text.clone(),
            },
            TextComponent {
                color: Vec4::splat(0.9),
                font: assets.font.clone(),
                font_size,
                horizontal_align: HorizontalAlign::Left,
                vertical_align: VerticalAlign::Middle,
                horizontal_spacing: UiVal::px(0.0),
                is_y_inverted: true,
                text,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_text, ent_parent);

    ent_text
}

fn spawn_label(
    mut ent: EntitiesHolderMut,
    assets: &StandardAssetsResource,
    ent_parent: EntityId,
    text: FfiString,
    width: f32,
    font_size: UiVal,
) -> EntityId {
    let ent_label = spawn_text(ent.as_mut(), assets, ent_parent, text, font_size);
    ent.set_component(
        ent_label,
        LocalRectComponent {
            scale: Vec2::new(UiVal::px(width).into(), UiVal::ph(1.0).into()),
            ..Default::default()
        },
    );
    ent_label
}

fn spawn_field(
    mut ent: EntitiesHolderMut,
    assets: &StandardAssetsResource,
    ent_parent: EntityId,
    field: AnimationField,
) -> EntityId {
    let ent_container = ent.create_entity();

    ent.set_components(
        ent_container,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::px(FIELD_WIDTH).into(), UiVal::ph(1.0).into()),
                ..Default::default()
            },
            ParentComponent::default(),
        ),
    );
    set_parent(ent.as_mut(), ent_container, ent_parent);

    let ent_field = spawn_input_area_ent(
        ent.as_mut(),
        ent_container,
        "".into(),
        assets.material_panel.clone(),
        assets.material_text.clone(),
        assets.font.clone(),
    );
    ent.add_component(ent_field, AnimationFieldComponent { field }).ok().unwrap();

    ent_field
}
