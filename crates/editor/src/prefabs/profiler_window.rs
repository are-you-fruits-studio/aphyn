use crate::{
    features::{
        inspector_window::{FONT_SIZE, utils::entries::spawn_input_area_ent},
        profiler_window::*,
    },
    prefabs::{
        ANIMATION_GAP, ANIMATION_MARGIN, ANIMATION_TOOLBAR_HEIGHT, WindowComponent, spawn_area, spawn_button, spawn_label,
        spawn_lines, spawn_row, spawn_text,
    },
    theme::PAL,
    *,
};

pub const PROFILER_HISTOGRAM_HEIGHT: f32 = 90.0;
pub const PROFILER_RULER_HEIGHT: f32 = 18.0;
pub const PROFILER_LANE_LABEL_WIDTH: f32 = 72.0;
pub const PROFILER_DETAILS_HEIGHT: f32 = 40.0;
pub const PROFILER_MAX_LANES: usize = 32;
pub const PROFILER_RULER_LABELS: usize = 16;
pub const PROFILER_SEGMENT_LABELS: usize = 64;
pub const PROFILER_SMALL_FONT_PX: f32 = 12.0;

const FIELD_WIDTH: f32 = 56.0;
const SEGMENT_BORDER_Z: f32 = -0.1;
const SEGMENT_FILL_Z: f32 = -0.2;
const FRAME_LINE_Z: f32 = -0.25;
const SEGMENT_LABEL_Z: f32 = -0.3;

pub fn init_window_as_profiler(mut world: WorldDataMut, ent_window: EntityId) {
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

    let small_font = UiVal::px(PROFILER_SMALL_FONT_PX);

    // toolbar
    let ent_toolbar = spawn_row(ent.as_mut(), ent_root, ANIMATION_MARGIN);
    spawn_button(
        ent.as_mut(),
        assets,
        ent_toolbar,
        ProfilerRecordButtonComponent,
        "Record",
        "Recording",
    );
    spawn_button(
        ent.as_mut(),
        assets,
        ent_toolbar,
        ProfilerClearButtonComponent,
        "Clear",
        "Clear",
    );
    spawn_label(ent.as_mut(), assets, ent_toolbar, "Frames".into(), 52.0, FONT_SIZE);
    let frames_field = spawn_frames_field(ent.as_mut(), assets, ent_toolbar);
    let status_text = spawn_label(ent.as_mut(), assets, ent_toolbar, "".into(), 900.0, FONT_SIZE);

    // histogram of frame times
    let histogram_top = ANIMATION_MARGIN + ANIMATION_TOOLBAR_HEIGHT + ANIMATION_GAP;
    let histogram = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::splat(0.0),
        Vec2::splat(0.0),
        None,
        PAL.bg_sunken,
    );
    ent.set_component(
        histogram,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(0.0), UiVal::px(histogram_top)),
            parent_padding_max: Vec2::new(UiVal::px(0.0), UiVal::px(0.0)),
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(PROFILER_HISTOGRAM_HEIGHT).into()),
            ..Default::default()
        },
    );
    let histogram_grid = spawn_lines(ent.as_mut(), assets, histogram, PAL.border_strong, 1.0);
    let histogram_bars = spawn_lines(ent.as_mut(), assets, histogram, PAL.bloom_600, 1.0);
    let histogram_bars_warning = spawn_lines(ent.as_mut(), assets, histogram, PAL.warning, 1.0);
    let histogram_bars_danger = spawn_lines(ent.as_mut(), assets, histogram, PAL.danger, 1.0);
    let histogram_selected = spawn_lines(ent.as_mut(), assets, histogram, PAL.accent_fg, 1.0);
    let histogram_label = spawn_text(ent.as_mut(), assets, histogram, "".into(), small_font);
    ent.set_component(
        histogram_label,
        LocalRectComponent {
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            offset: Vec2::new(UiVal::px(4.0), UiVal::px(0.0)),
            scale: Vec2::new(UiVal::px(400.0).into(), UiVal::px(PROFILER_SMALL_FONT_PX + 4.0).into()),
            ..Default::default()
        },
    );

    // timeline: lane labels on the left, ruler + lanes on the right, details at the bottom
    let ruler_top = histogram_top + PROFILER_HISTOGRAM_HEIGHT + ANIMATION_GAP;
    let lanes_top = ruler_top + PROFILER_RULER_HEIGHT;
    let lanes_left = PROFILER_LANE_LABEL_WIDTH + ANIMATION_GAP;
    let lanes_bottom = PROFILER_DETAILS_HEIGHT + ANIMATION_GAP;

    let lane_list = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::new(0.0, lanes_top),
        Vec2::new(0.0, lanes_bottom),
        Some(PROFILER_LANE_LABEL_WIDTH),
        PAL.surface_panel,
    );
    let mut lane_labels = FfiVec::new();
    for _ in 0..PROFILER_MAX_LANES {
        lane_labels.push(spawn_positioned_text(ent.as_mut(), assets, lane_list, small_font));
    }

    let ruler = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::splat(0.0),
        Vec2::splat(0.0),
        None,
        PAL.surface_raised,
    );
    ent.set_component(
        ruler,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(lanes_left), UiVal::px(ruler_top)),
            parent_padding_max: Vec2::new(UiVal::px(0.0), UiVal::px(0.0)),
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(PROFILER_RULER_HEIGHT).into()),
            ..Default::default()
        },
    );
    let mut ruler_labels = FfiVec::new();
    for _ in 0..PROFILER_RULER_LABELS {
        ruler_labels.push(spawn_positioned_text(ent.as_mut(), assets, ruler, small_font));
    }
    let ruler_ticks = spawn_lines(ent.as_mut(), assets, ruler, PAL.border_strong, 1.0);

    let lanes = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::new(lanes_left, lanes_top),
        Vec2::new(0.0, lanes_bottom),
        None,
        PAL.bg_sunken,
    );
    let lanes_grid = spawn_lines(ent.as_mut(), assets, lanes, PAL.border_subtle, 1.0);
    let segment_borders = spawn_lines(ent.as_mut(), assets, lanes, PAL.bg_sunken, 1.0);
    let segments = span_colors().map(|color| spawn_lines(ent.as_mut(), assets, lanes, color, 1.0));
    let hovered_segment = spawn_lines(ent.as_mut(), assets, lanes, PAL.text_1, 1.0);
    let selected_segment = spawn_lines(ent.as_mut(), assets, lanes, PAL.accent_fg, 1.0);
    let frame_lines = spawn_lines(ent.as_mut(), assets, lanes, PAL.night_500, 1.0);
    let selected_frame_lines = spawn_lines(ent.as_mut(), assets, lanes, PAL.accent_fg, 1.0);

    // siblings share a z otherwise, which leaves their draw order undefined; lower z is in front
    set_z(ent.as_mut(), segment_borders, SEGMENT_BORDER_Z);
    for ent_lines in segments.into_iter().chain([hovered_segment, selected_segment]) {
        set_z(ent.as_mut(), ent_lines, SEGMENT_FILL_Z);
    }
    for ent_lines in [frame_lines, selected_frame_lines] {
        set_z(ent.as_mut(), ent_lines, FRAME_LINE_Z);
    }

    let mut segment_labels = FfiVec::new();
    for _ in 0..PROFILER_SEGMENT_LABELS {
        let ent_label = spawn_positioned_text(ent.as_mut(), assets, lanes, small_font);
        if let Some(text_c) = ent.get_component_mut::<TextComponent>(ent_label) {
            text_c.color = PAL.text_2;
        }
        set_z(ent.as_mut(), ent_label, SEGMENT_LABEL_Z);
        segment_labels.push(ent_label);
    }

    let details = spawn_area(
        ent.as_mut(),
        assets,
        ent_root,
        Vec2::splat(0.0),
        Vec2::splat(0.0),
        None,
        PAL.surface_panel,
    );
    ent.set_component(
        details,
        LocalRectComponent {
            anchor: Vec2::new(0.0, 1.0),
            pivot: Vec2::new(0.0, 1.0),
            scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(PROFILER_DETAILS_HEIGHT).into()),
            ..Default::default()
        },
    );
    ent.set_component(
        details,
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
        },
    );
    let mut details_lines = [EntityId::EMPTY; 2];
    for line in &mut details_lines {
        *line = spawn_text(ent.as_mut(), assets, details, "".into(), FONT_SIZE);
        ent.set_component(
            *line,
            LocalRectComponent {
                offset: Vec2::new(UiVal::px(ANIMATION_MARGIN), UiVal::px(0.0)),
                scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(PROFILER_DETAILS_HEIGHT * 0.5).into()),
                ..Default::default()
            },
        );
    }

    ent.set_component(
        ent_root,
        ProfilerWindowUiComponent {
            status_text,
            frames_field,
            histogram,
            histogram_grid,
            histogram_bars,
            histogram_bars_warning,
            histogram_bars_danger,
            histogram_selected,
            histogram_label,
            lane_labels,
            ruler,
            ruler_labels,
            ruler_ticks,
            lanes,
            lanes_grid,
            frame_lines,
            selected_frame_lines,
            segments,
            segment_borders,
            selected_segment,
            hovered_segment,
            segment_labels,
            details_lines,
        },
    );
}

// a text placed by `offset` / `scale` from the parent's top-left corner
fn spawn_positioned_text(
    mut ent: EntitiesHolderMut,
    assets: &StandardAssetsResource,
    ent_parent: EntityId,
    font_size: UiVal,
) -> EntityId {
    let ent_text = spawn_text(ent.as_mut(), assets, ent_parent, "".into(), font_size);
    ent.set_component(
        ent_text,
        LocalRectComponent {
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            scale: Vec2::new(UiVal::px(0.0).into(), UiVal::px(0.0).into()),
            ..Default::default()
        },
    );
    ent_text
}

fn spawn_frames_field(mut ent: EntitiesHolderMut, assets: &StandardAssetsResource, ent_parent: EntityId) -> EntityId {
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
    ent.add_component(ent_field, ProfilerFramesFieldComponent).ok().unwrap();

    ent_field
}

fn set_z(mut ent: EntitiesHolderMut, entity: EntityId, z: f32) {
    if let Some(rect_c) = ent.get_component_mut::<LocalRectComponent>(entity) {
        rect_c.z = z;
    }
}
