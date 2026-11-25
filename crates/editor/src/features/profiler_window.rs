use std::collections::VecDeque;

use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        animation_window::{clicked, set_field_text, set_points, set_text, style_buttons},
        game_window::render_game_window_system,
        input_field::{
            InputFieldComponent, InputFieldTextChangedEvent, SelectedInputFieldResource, apply_input_field_context_menu_system,
            update_selected_input_field_text_system,
        },
        play_mode::toggle_play_mode_system,
        ui_interaction::{ButtonComponent, HoverAreaComponent, check_ui_interaction_system},
        world_preload::{SimulatedWorldResource, tick_simulated_world_system},
    },
    prefabs::{PROFILER_LANE_LABEL_WIDTH, PROFILER_MAX_LANES, PROFILER_RULER_HEIGHT, PROFILER_SMALL_FONT_PX},
};

pub const HISTOGRAM_BAR_COLOR: Vec4<f32> = Vec4::new(0.45, 0.6, 0.45, 1.0);
pub const SELECTED_COLOR: Vec4<f32> = Vec4::new(1.0, 0.75, 0.2, 1.0);
pub const HOVERED_COLOR: Vec4<f32> = Vec4::new(0.92, 0.92, 0.92, 1.0);
pub const SEGMENT_BORDER_COLOR: Vec4<f32> = Vec4::new(0.04, 0.04, 0.04, 1.0);
pub const FRAME_LINE_COLOR: Vec4<f32> = Vec4::new(0.5, 0.5, 0.5, 1.0);
// indexed by `ProfileSpanKind`
pub const SPAN_COLORS: [Vec4<f32>; ProfileSpanKind::COUNT] = [
    Vec4::new(0.45, 0.62, 0.9, 1.0),
    Vec4::new(0.75, 0.5, 0.85, 1.0),
    Vec4::new(0.9, 0.55, 0.4, 1.0),
    Vec4::new(0.45, 0.78, 0.45, 1.0),
    Vec4::new(0.35, 0.75, 0.72, 1.0),
    Vec4::new(0.8, 0.8, 0.38, 1.0),
];

const BUTTON_COLOR: Vec4<f32> = Vec4::new(0.3, 0.3, 0.3, 1.0);
const BUTTON_COLOR_RECORD: Vec4<f32> = Vec4::new(0.8, 0.25, 0.25, 1.0);
const TEXT_COLOR: Vec4<f32> = Vec4::splat(0.9);
const LABEL_COLOR: Vec4<f32> = Vec4::new(0.05, 0.05, 0.05, 1.0);

const DEFAULT_CAPACITY: usize = 300;
const MIN_CAPACITY: usize = 10;
const MAX_CAPACITY: usize = 5000;
const VIEW_PADDING: f32 = 6.0;
const MAX_LANE_HEIGHT: f32 = 34.0;
const MIN_LANE_HEIGHT: f32 = 10.0;
// the part of a lane above the jobs row
const MAIN_ROW_RATIO: f32 = 0.55;
const HISTOGRAM_LABEL_HEIGHT: f32 = 16.0;
const DRAG_THRESHOLD: f32 = 3.0;
const SEGMENT_BORDER_PX: f32 = 1.0;
const SEGMENT_LANE_MARGIN_PX: f32 = 2.0;
const MIN_LABEL_CHARS: usize = 3;
const MIN_LABEL_ROW_PX: f32 = 11.0;
// glyph width / height of the editor font
const GLYPH_RATIO: f32 = 0.7;
const NS_PER_MS: f64 = 1_000_000.0;

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(ProfilerResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(sync_profiler_system)
        .insert_child_system(collect_profiler_frame_system)
        .insert_child_system(click_profiler_buttons_system)
        .insert_child_system(apply_profiler_fields_system)
        .insert_child_system(profiler_window_mouse_system)
        .insert_child_system(draw_profiler_window_system);

    // a frame is the simulated tick followed by the game view render
    update
        .order_system(toggle_play_mode_system)
        .before_system(sync_profiler_system);
    update
        .order_system(sync_profiler_system)
        .before_system(tick_simulated_world_system);
    update
        .order_system(tick_simulated_world_system)
        .before_system(render_game_window_system);
    update
        .order_system(render_game_window_system)
        .before_system(collect_profiler_frame_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(click_profiler_buttons_system);
    update
        .order_system(check_ui_interaction_system)
        .before_system(profiler_window_mouse_system);
    update
        .order_system(update_selected_input_field_text_system)
        .before_system(apply_profiler_fields_system);
    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(apply_profiler_fields_system);

    update
        .order_system(click_profiler_buttons_system)
        .before_system(sync_profiler_system);
    update
        .order_system(collect_profiler_frame_system)
        .before_system(draw_profiler_window_system);
    update
        .order_system(click_profiler_buttons_system)
        .before_system(draw_profiler_window_system);
    update
        .order_system(apply_profiler_fields_system)
        .before_system(draw_profiler_window_system);
    update
        .order_system(profiler_window_mouse_system)
        .before_system(draw_profiler_window_system);
}

#[derive(Component, Default, Debug)]
pub struct ProfilerWindowComponent;

#[derive(Component, Default, Debug)]
pub struct ProfilerRecordButtonComponent;
#[derive(Component, Default, Debug)]
pub struct ProfilerClearButtonComponent;
#[derive(Component, Default, Debug)]
pub struct ProfilerFramesFieldComponent;

#[derive(Component)]
pub struct ProfilerWindowUiComponent {
    pub status_text: EntityId,
    pub frames_field: EntityId,
    pub histogram: EntityId,
    pub histogram_grid: EntityId,
    pub histogram_bars: EntityId,
    pub histogram_selected: EntityId,
    pub histogram_label: EntityId,
    pub lane_labels: FfiVec<EntityId>,
    pub ruler: EntityId,
    pub ruler_labels: FfiVec<EntityId>,
    pub ruler_ticks: EntityId,
    pub lanes: EntityId,
    pub lanes_grid: EntityId,
    pub frame_lines: EntityId,
    pub selected_frame_lines: EntityId,
    // indexed by `ProfileSpanKind`
    pub segments: [EntityId; ProfileSpanKind::COUNT],
    pub segment_borders: EntityId,
    pub selected_segment: EntityId,
    pub hovered_segment: EntityId,
    pub segment_labels: FfiVec<EntityId>,
    pub details_lines: [EntityId; 2],
}

// spans are kept with the frame during which they were collected; background jobs may have started earlier
pub struct ProfiledFrame {
    pub number: u64,
    pub start_ns: u64,
    pub end_ns: u64,
    pub spans: Vec<ProfileSpan>,
    // the pool's frame worker count, to tell frame workers from background workers
    pub frame_threads: u32,
    spans_start_ns: u64,
    spans_end_ns: u64,
}

impl ProfiledFrame {
    fn duration_ns(&self) -> u64 {
        self.end_ns.saturating_sub(self.start_ns)
    }

    fn lanes_count(&self) -> usize {
        self.spans.iter().map(|s| s.thread as usize + 1).max().unwrap_or(1)
    }
}

// (frame number, index in its spans)
type SpanRef = (u64, usize);

#[derive(Copy, Clone, Debug)]
struct ProfilerView {
    start_ns: f64,
    end_ns: f64,
}

impl Default for ProfilerView {
    fn default() -> Self {
        Self {
            start_ns: 0.0,
            end_ns: 16.0 * NS_PER_MS,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum ProfilerDrag {
    Histogram,
    Lanes {
        start_mouse: Vec2<f32>,
        start_view: ProfilerView,
        picked: Option<SpanRef>,
        is_panning: bool,
    },
}

#[derive(Resource)]
pub struct ProfilerResource {
    pub is_recording: bool,
    pub capacity: usize,
    // one recorder for the whole session, so every frame shares one time axis
    recorder: ProfileRecorder,
    pending_frame_start: Option<u64>,
    frames: VecDeque<ProfiledFrame>,
    next_frame_number: u64,
    lanes_count: usize,
    selected_frame: Option<u64>,
    selected_span: Option<SpanRef>,
    hovered_span: Option<SpanRef>,
    view: ProfilerView,
    drag: Option<ProfilerDrag>,
}

impl Default for ProfilerResource {
    fn default() -> Self {
        Self {
            is_recording: false,
            capacity: DEFAULT_CAPACITY,
            recorder: ProfileRecorder::new(),
            pending_frame_start: None,
            frames: VecDeque::new(),
            next_frame_number: 0,
            lanes_count: 0,
            selected_frame: None,
            selected_span: None,
            hovered_span: None,
            view: ProfilerView::default(),
            drag: None,
        }
    }
}

impl ProfilerResource {
    fn frame(&self, number: u64) -> Option<&ProfiledFrame> {
        let first = self.frames.front()?.number;
        self.frames.get(number.checked_sub(first)? as usize)
    }

    fn selected(&self) -> Option<&ProfiledFrame> {
        self.frame(self.selected_frame?)
    }

    fn span(&self, (frame, idx): SpanRef) -> Option<(&ProfiledFrame, &ProfileSpan)> {
        let frame = self.frame(frame)?;
        Some((frame, frame.spans.get(idx)?))
    }

    fn frame_threads(&self) -> u32 {
        self.frames.back().map_or(0, |f| f.frame_threads)
    }

    fn select_frame(&mut self, number: u64) {
        if self.selected_frame == Some(number) {
            return;
        }

        self.selected_frame = Some(number);

        if let Some(frame) = self.selected() {
            let padding = (frame.duration_ns() as f64).max(1.0) * 0.02;
            self.view = ProfilerView {
                start_ns: frame.start_ns as f64 - padding,
                end_ns: frame.end_ns as f64 + padding,
            };
        }
    }

    fn push(&mut self, frame: ProfiledFrame) {
        self.lanes_count = self.lanes_count.max(frame.lanes_count());
        self.frames.push_back(frame);
        self.trim();
    }

    fn trim(&mut self) {
        let mut removed = false;
        while self.frames.len() > self.capacity {
            self.frames.pop_front();
            removed = true;
        }

        if removed {
            self.lanes_count = self.frames.iter().map(ProfiledFrame::lanes_count).max().unwrap_or(0);
        }
        if self.selected_frame.is_some() && self.selected().is_none() {
            self.selected_frame = None;
        }
        if self.selected_span.is_some_and(|s| self.span(s).is_none()) {
            self.selected_span = None;
        }
    }

    fn clear(&mut self) {
        self.frames.clear();
        _ = self.recorder.take_spans();
        self.lanes_count = 0;
        self.selected_frame = None;
        self.selected_span = None;
        self.hovered_span = None;
        self.drag = None;
    }

    fn visible_frames(&self) -> impl Iterator<Item = &ProfiledFrame> {
        let view = self.view;
        self.frames
            .iter()
            .filter(move |f| f.spans_start_ns as f64 <= view.end_ns && f.spans_end_ns as f64 >= view.start_ns)
    }

    // the ruler counts from the selected frame's start
    fn time_origin_ns(&self) -> f64 {
        self.selected().or(self.frames.front()).map_or(0.0, |f| f.start_ns as f64)
    }
}

pub fn sync_profiler_system(mut profiler: ResMut<ProfilerResource>, mut simulated_world: ResMut<SimulatedWorldResource>) {
    let is_playing = simulated_world.is_playing();
    let is_capturing = profiler.is_recording && is_playing;

    profiler.pending_frame_start = None;
    return_if_not!(Some(world) = &mut simulated_world.0);

    if !is_capturing {
        world.profiler = None;
        return;
    }

    if world.profiler.is_none() {
        world.profiler = Some(profiler.recorder.clone());
    }
    profiler.pending_frame_start = Some(profiler.recorder.now_ns());
}

pub fn collect_profiler_frame_system(mut profiler: ResMut<ProfilerResource>, simulated_world: Res<SimulatedWorldResource>) {
    return_if_not!(Some(start_ns) = profiler.pending_frame_start.take());

    let end_ns = profiler.recorder.now_ns();
    let spans = profiler.recorder.take_spans().clone_to_vec();
    let number = profiler.next_frame_number;
    profiler.next_frame_number += 1;

    let frame = ProfiledFrame {
        number,
        start_ns,
        end_ns,
        frame_threads: simulated_world.frame_threads_count() as u32,
        spans_start_ns: spans.iter().map(|s| s.start_ns).min().unwrap_or(start_ns).min(start_ns),
        spans_end_ns: spans.iter().map(|s| s.end_ns).max().unwrap_or(end_ns).max(end_ns),
        spans,
    };
    profiler.push(frame);
}

pub fn click_profiler_buttons_system(
    record_q: WorldQuery<&ButtonComponent, WithFilter<ProfilerRecordButtonComponent>>,
    clear_q: WorldQuery<&ButtonComponent, WithFilter<ProfilerClearButtonComponent>>,
    mut profiler: ResMut<ProfilerResource>,
) {
    if clicked(&record_q) {
        profiler.is_recording = !profiler.is_recording;
    }

    if clicked(&clear_q) {
        profiler.clear();
    }
}

pub fn apply_profiler_fields_system(
    text_changed_evt: Evt<InputFieldTextChangedEvent>,
    field_q: WorldQuery<&InputFieldComponent, WithFilter<ProfilerFramesFieldComponent>>,
    text_q: WorldQuery<&TextComponent>,
    mut profiler: ResMut<ProfilerResource>,
) {
    for evt in text_changed_evt.iter() {
        continue_if_not!(Some(input_c) = field_q.get(evt.input_field));
        continue_if_not!(Some(text_c) = text_q.get(input_c.text));
        continue_if_not!(Ok(capacity) = text_c.text.as_str().trim().parse::<usize>());

        profiler.capacity = capacity.clamp(MIN_CAPACITY, MAX_CAPACITY);
        profiler.trim();
    }
}

#[derive(Copy, Clone)]
struct HistogramView {
    min: Vec2<f32>,
    size: Vec2<f32>,
    capacity: usize,
    len: usize,
}

impl HistogramView {
    fn new(rect: &GlobalRectComponent, profiler: &ProfilerResource) -> Self {
        Self {
            min: rect.center - rect.scale * 0.5,
            size: rect.scale.map(|v| v.max(1.0)),
            capacity: profiler.capacity.max(1),
            len: profiler.frames.len(),
        }
    }

    fn slot_width(&self) -> f32 {
        self.size.x / self.capacity as f32
    }

    // the newest frame is in the rightmost slot
    fn x(&self, frame_idx: usize) -> f32 {
        let slot = self.capacity.saturating_sub(self.len) + frame_idx;
        self.min.x + (slot as f32 + 0.5) * self.slot_width()
    }

    fn frame_idx(&self, x: f32) -> Option<usize> {
        let slot = ((x - self.min.x) / self.slot_width()).floor();
        let first_slot = self.capacity.saturating_sub(self.len) as f32;
        (slot >= first_slot && slot < self.capacity as f32).then(|| (slot - first_slot) as usize)
    }

    fn to_local(&self, p: Vec2<f32>) -> Vec2<f32> {
        (p - self.min) / self.size
    }
}

#[derive(Copy, Clone)]
struct LanesView {
    min: Vec2<f32>,
    size: Vec2<f32>,
    view: ProfilerView,
    lane_height: f32,
}

impl LanesView {
    fn new(rect: &GlobalRectComponent, view: ProfilerView, lanes_count: usize) -> Self {
        let size = rect.scale.map(|v| v.max(1.0));
        Self {
            min: rect.center - rect.scale * 0.5,
            size,
            view,
            lane_height: (size.y / lanes_count.max(1) as f32).clamp(MIN_LANE_HEIGHT, MAX_LANE_HEIGHT),
        }
    }

    fn inner_width(&self) -> f32 {
        (self.size.x - VIEW_PADDING * 2.0).max(1.0)
    }

    fn span(&self) -> f64 {
        (self.view.end_ns - self.view.start_ns).max(1.0)
    }

    fn x(&self, ns: f64) -> f32 {
        self.min.x + VIEW_PADDING + ((ns - self.view.start_ns) / self.span()) as f32 * self.inner_width()
    }

    fn ns(&self, x: f32) -> f64 {
        self.view.start_ns + ((x - self.min.x - VIEW_PADDING) / self.inner_width()) as f64 * self.span()
    }

    fn max_x(&self) -> f32 {
        self.min.x + self.size.x
    }

    fn lane_top(&self, lane: usize) -> f32 {
        self.min.y + lane as f32 * self.lane_height
    }

    // systems and render features on top, the jobs they started below
    fn row(&self, lane: usize, is_job: bool) -> (f32, f32) {
        let top = self.lane_top(lane);
        let split = top + (self.lane_height * MAIN_ROW_RATIO).round();

        match is_job {
            false => (top + SEGMENT_LANE_MARGIN_PX, split),
            true => (split + 1.0, top + self.lane_height - SEGMENT_LANE_MARGIN_PX),
        }
    }

    // the drawn horizontal extent of a span, None when outside the view
    fn span_extent(&self, span: &ProfileSpan) -> Option<(f32, f32)> {
        let x0 = self.x(span.start_ns as f64).max(self.min.x);
        let x1 = self.x(span.end_ns as f64).min(self.max_x());
        (x1 >= self.min.x && x0 <= self.max_x()).then_some((x0, x1.max(x0 + 1.0)))
    }

    fn pick(&self, profiler: &ProfilerResource, mouse: Vec2<f32>) -> Option<SpanRef> {
        let lane = ((mouse.y - self.min.y) / self.lane_height).floor();
        if lane < 0.0 {
            return None;
        }
        let lane = lane as usize;
        let is_job = mouse.y >= self.row(lane, false).1;

        profiler.visible_frames().find_map(|frame| {
            frame
                .spans
                .iter()
                .position(|span| {
                    span.thread as usize == lane
                        && span.kind.is_job() == is_job
                        && self
                            .span_extent(span)
                            .is_some_and(|(x0, x1)| mouse.x >= x0 - 1.0 && mouse.x <= x1.max(x0 + 2.0) + 1.0)
                })
                .map(|idx| (frame.number, idx))
        })
    }

    fn to_local(&self, p: Vec2<f32>) -> Vec2<f32> {
        (p - self.min) / self.size
    }
}

pub fn profiler_window_mouse_system(
    input: Res<InputResource>,
    ui_q: WorldQuery<&ProfilerWindowUiComponent>,
    area_q: WorldQuery<(&GlobalRectComponent, &HoverAreaComponent)>,
    mut profiler: ResMut<ProfilerResource>,
) {
    let profiler = &mut *profiler;
    profiler.hovered_span = None;

    return_if_not!(Some(ui) = ui_q.iter().next());
    return_if_not!(Some((histogram_rect, histogram_hover)) = area_q.get(ui.histogram));
    return_if_not!(Some((lanes_rect, lanes_hover)) = area_q.get(ui.lanes));
    return_if_not!(Some((_, ruler_hover)) = area_q.get(ui.ruler));

    let mouse = Vec2::from_array(input.mouse.position.map(|v| v as f32));
    let histogram = HistogramView::new(histogram_rect, profiler);
    let lanes = LanesView::new(lanes_rect, profiler.view, profiler.lanes_count);

    let [wheel_x, wheel_y] = input.mouse.scroll_delta_lines;
    if (lanes_hover.is_hovered || ruler_hover.is_hovered) && (wheel_x != 0.0 || wheel_y != 0.0) {
        let pivot = lanes.ns(mouse.x);
        let factor = 0.85_f64.powf(wheel_y as f64);
        let shift = -wheel_x as f64 * 0.1 * lanes.span();
        let v = &mut profiler.view;
        v.start_ns = pivot + (v.start_ns - pivot) * factor + shift;
        v.end_ns = pivot + (v.end_ns - pivot) * factor + shift;
    }

    if input.mouse.is_just_pressed(MouseButton::Left) {
        if histogram_hover.is_hovered {
            profiler.drag = Some(ProfilerDrag::Histogram);
        } else if lanes_hover.is_hovered {
            profiler.drag = Some(ProfilerDrag::Lanes {
                start_mouse: mouse,
                start_view: profiler.view,
                picked: lanes.pick(profiler, mouse),
                is_panning: false,
            });
        }
    }

    match profiler.drag {
        Some(ProfilerDrag::Histogram) => {
            if let Some(number) = histogram
                .frame_idx(mouse.x)
                .and_then(|i| profiler.frames.get(i))
                .map(|f| f.number)
            {
                profiler.select_frame(number);
            }
        }
        Some(ProfilerDrag::Lanes {
            start_mouse,
            start_view,
            picked,
            is_panning,
        }) => {
            let dx = mouse.x - start_mouse.x;
            let is_panning = is_panning || dx.abs() > DRAG_THRESHOLD;

            if is_panning {
                let shift = -(dx / lanes.inner_width()) as f64 * (start_view.end_ns - start_view.start_ns);
                profiler.view.start_ns = start_view.start_ns + shift;
                profiler.view.end_ns = start_view.end_ns + shift;
            }

            profiler.drag = Some(ProfilerDrag::Lanes {
                start_mouse,
                start_view,
                picked,
                is_panning,
            });
        }
        None => {}
    }

    if !input.mouse.is_pressed(MouseButton::Left) {
        if let Some(ProfilerDrag::Lanes {
            picked,
            is_panning: false,
            ..
        }) = profiler.drag
        {
            profiler.selected_span = picked;
        }
        profiler.drag = None;
    }

    let is_panning = matches!(profiler.drag, Some(ProfilerDrag::Lanes { is_panning: true, .. }));
    if lanes_hover.is_hovered && !is_panning {
        let lanes = LanesView::new(lanes_rect, profiler.view, profiler.lanes_count);
        profiler.hovered_span = lanes.pick(profiler, mouse);
    }
}

pub fn draw_profiler_window_system(
    mut ent: EntitiesHolderMut,
    profiler: Res<ProfilerResource>,
    simulated_world: Res<SimulatedWorldResource>,
    selected_input: Res<SelectedInputFieldResource>,
) {
    return_if_not!(
        Some(ui) = ent.query::<&ProfilerWindowUiComponent>().iter().next().map(|ui| UiEntities {
            status_text: ui.status_text,
            frames_field: ui.frames_field,
            histogram: ui.histogram,
            histogram_grid: ui.histogram_grid,
            histogram_bars: ui.histogram_bars,
            histogram_selected: ui.histogram_selected,
            histogram_label: ui.histogram_label,
            lane_labels: ui.lane_labels.clone_to_vec(),
            ruler: ui.ruler,
            ruler_labels: ui.ruler_labels.clone_to_vec(),
            ruler_ticks: ui.ruler_ticks,
            lanes: ui.lanes,
            lanes_grid: ui.lanes_grid,
            frame_lines: ui.frame_lines,
            selected_frame_lines: ui.selected_frame_lines,
            segments: ui.segments,
            segment_borders: ui.segment_borders,
            selected_segment: ui.selected_segment,
            hovered_segment: ui.hovered_segment,
            segment_labels: ui.segment_labels.clone_to_vec(),
            details_lines: ui.details_lines,
        })
    );

    let selected = profiler.selected();
    let max_ns = profiler.frames.iter().map(ProfiledFrame::duration_ns).max().unwrap_or(0);
    let frame_threads = profiler.frame_threads();

    // toolbar
    style_buttons::<ProfilerRecordButtonComponent>(
        &mut ent,
        if profiler.is_recording {
            BUTTON_COLOR_RECORD
        } else {
            BUTTON_COLOR
        },
        Some(if profiler.is_recording { "Recording" } else { "Record" }),
    );
    set_field_text(&mut ent, &selected_input, ui.frames_field, &profiler.capacity.to_string());

    let mut status = format!("{} frames", profiler.frames.len());
    if let Some(frame) = selected {
        status += &format!(
            " | frame {}: {} ms",
            frame.number,
            format_ms(frame.duration_ns() as f64 / NS_PER_MS, 0.01)
        );
    }
    if profiler.is_recording && !simulated_world.is_playing() {
        status += " | records in play mode";
    }
    set_text(&mut ent, ui.status_text, &status, TEXT_COLOR);

    draw_histogram(&mut ent, &ui, &profiler, max_ns);

    // timeline
    let Some(lanes_rect) = ent.get_component::<GlobalRectComponent>(ui.lanes).copied() else {
        return;
    };
    let Some(ruler_rect) = ent.get_component::<GlobalRectComponent>(ui.ruler).copied() else {
        return;
    };

    let lanes_count = profiler.lanes_count.min(PROFILER_MAX_LANES);
    let view = LanesView::new(&lanes_rect, profiler.view, lanes_count);

    for (lane, ent_label) in ui.lane_labels.iter().enumerate() {
        let text = if lane < lanes_count {
            lane_name(lane as u32, frame_threads)
        } else {
            String::new()
        };
        set_text(&mut ent, *ent_label, &text, TEXT_COLOR);
        place(
            &mut ent,
            *ent_label,
            Vec2::new(4.0, lane as f32 * view.lane_height),
            Vec2::new(PROFILER_LANE_LABEL_WIDTH - 4.0, view.lane_height),
        );
    }

    draw_ruler(&mut ent, &ui, &view, &ruler_rect, profiler.time_origin_ns());

    let mut grid = Vec::new();
    for lane in 1..lanes_count {
        let y = view.lane_top(lane);
        grid.push(view.to_local(Vec2::new(view.min.x, y)));
        grid.push(view.to_local(Vec2::new(view.max_x(), y)));
    }
    set_points(&mut ent, ui.lanes_grid, grid);

    // frame separators
    let bottom = view.min.y + view.size.y;
    let vertical = |x: f32| {
        [
            view.to_local(Vec2::new(x + 0.5, view.min.y)),
            view.to_local(Vec2::new(x + 0.5, bottom)),
        ]
    };
    let in_view = |ns: u64| (ns as f64) >= view.view.start_ns && (ns as f64) <= view.view.end_ns;

    let mut frame_lines = Vec::new();
    let mut selected_frame_lines = Vec::new();
    for frame in profiler.visible_frames() {
        if profiler.selected_frame == Some(frame.number) {
            for ns in [frame.start_ns, frame.end_ns] {
                if in_view(ns) {
                    selected_frame_lines.extend(vertical(view.x(ns as f64).floor()));
                }
            }
        } else if in_view(frame.start_ns) {
            frame_lines.extend(vertical(view.x(frame.start_ns as f64).floor()));
        }
    }
    set_lines(&mut ent, ui.frame_lines, frame_lines, 1.0);
    set_lines(&mut ent, ui.selected_frame_lines, selected_frame_lines, 1.0);

    // segments
    let mut segments: [Vec<Vec2<f32>>; ProfileSpanKind::COUNT] = Default::default();
    let mut selected_segment = Vec::new();
    let mut hovered_segment = Vec::new();
    let mut borders = Vec::new();
    let mut label_candidates = Vec::new();

    for frame in profiler.visible_frames() {
        for (idx, span) in frame.spans.iter().enumerate() {
            continue_if!(span.thread as usize >= PROFILER_MAX_LANES);
            continue_if_not!(Some((x0, x1)) = view.span_extent(span));

            let (x0, x1) = (x0.floor(), x1.ceil().max(x0.floor() + 1.0));
            let (y0, y1) = view.row(span.thread as usize, span.kind.is_job());
            let (y0, y1) = (y0.floor(), y1.floor().max(y0.floor() + 1.0));
            push_rect(&mut borders, &view, x0, x1, y0, y1);

            // the fill is inset so the border shows on every side, except on segments too narrow for it
            let inset = if x1 - x0 > SEGMENT_BORDER_PX * 3.0 && y1 - y0 > SEGMENT_BORDER_PX * 3.0 {
                SEGMENT_BORDER_PX
            } else {
                0.0
            };

            let span_ref = (frame.number, idx);
            let target = if profiler.selected_span == Some(span_ref) {
                &mut selected_segment
            } else if profiler.hovered_span == Some(span_ref) {
                &mut hovered_segment
            } else {
                &mut segments[span.kind as usize]
            };
            push_rect(target, &view, x0 + inset, x1 - inset, y0 + inset, y1 - inset);

            if y1 - y0 >= MIN_LABEL_ROW_PX {
                label_candidates.push((x1 - x0, Vec2::new(x0, y0), y1 - y0, span));
            }
        }
    }

    set_lines(&mut ent, ui.segment_borders, borders, 1.0);
    for (ent_lines, points) in ui.segments.iter().zip(segments) {
        set_lines(&mut ent, *ent_lines, points, 1.0);
    }
    set_lines(&mut ent, ui.hovered_segment, hovered_segment, 1.0);
    set_lines(&mut ent, ui.selected_segment, selected_segment, 1.0);

    // name labels on the widest segments that fit some characters
    let char_px = PROFILER_SMALL_FONT_PX * GLYPH_RATIO;
    label_candidates.retain(|(width, ..)| (width - 6.0) / char_px >= MIN_LABEL_CHARS as f32);
    label_candidates.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));

    let mut labels = ui.segment_labels.iter();
    for ((width, top_left, height, span), ent_label) in label_candidates.into_iter().zip(&mut labels) {
        let max_chars = ((width - 6.0) / char_px) as usize;
        let text = truncate(&span_label(span), max_chars);
        set_text(&mut ent, *ent_label, &text, LABEL_COLOR);
        place(
            &mut ent,
            *ent_label,
            top_left - view.min + Vec2::new(3.0, 0.0),
            Vec2::new(width - 3.0, height),
        );
    }
    for ent_label in labels {
        set_text(&mut ent, *ent_label, "", TEXT_COLOR);
    }

    // details
    let span = profiler.selected_span.and_then(|s| profiler.span(s));
    let (line_0, line_1) = match span {
        Some((frame, span)) => (span_title(span), span_details(frame, span)),
        None if !profiler.frames.is_empty() => (
            "Select a frame in the histogram, then click a segment to see its details".into(),
            String::new(),
        ),
        None => ("Press Record and enter play mode".into(), String::new()),
    };
    set_text(&mut ent, ui.details_lines[0], &line_0, TEXT_COLOR);
    set_text(&mut ent, ui.details_lines[1], &line_1, TEXT_COLOR);
}

struct UiEntities {
    status_text: EntityId,
    frames_field: EntityId,
    histogram: EntityId,
    histogram_grid: EntityId,
    histogram_bars: EntityId,
    histogram_selected: EntityId,
    histogram_label: EntityId,
    lane_labels: Vec<EntityId>,
    ruler: EntityId,
    ruler_labels: Vec<EntityId>,
    ruler_ticks: EntityId,
    lanes: EntityId,
    lanes_grid: EntityId,
    frame_lines: EntityId,
    selected_frame_lines: EntityId,
    segments: [EntityId; ProfileSpanKind::COUNT],
    segment_borders: EntityId,
    selected_segment: EntityId,
    hovered_segment: EntityId,
    segment_labels: Vec<EntityId>,
    details_lines: [EntityId; 2],
}

fn draw_histogram(ent: &mut EntitiesHolderMut, ui: &UiEntities, profiler: &ProfilerResource, max_ns: u64) {
    return_if_not!(Some(rect) = ent.get_component::<GlobalRectComponent>(ui.histogram).copied());

    let view = HistogramView::new(&rect, profiler);
    let scale_ns = (max_ns as f64).max(NS_PER_MS);
    let bottom = view.min.y + view.size.y;
    let bar_space = (view.size.y - HISTOGRAM_LABEL_HEIGHT).max(1.0);
    let y = |ns: f64| bottom - (ns / scale_ns) as f32 * bar_space;

    let mut bars = Vec::new();
    let mut selected = Vec::new();
    for (idx, frame) in profiler.frames.iter().enumerate() {
        let x = view.x(idx);
        let target = if profiler.selected_frame == Some(frame.number) {
            &mut selected
        } else {
            &mut bars
        };
        target.push(view.to_local(Vec2::new(x, bottom)));
        target.push(view.to_local(Vec2::new(x, y(frame.duration_ns() as f64).min(bottom - 1.0))));
    }

    let thickness = (view.slot_width() - 1.0).max(1.0);
    set_lines(ent, ui.histogram_bars, bars, thickness);
    set_lines(ent, ui.histogram_selected, selected, thickness.max(2.0));

    // 60 and 30 fps lines
    let mut grid = Vec::new();
    for ms in [1000.0 / 60.0, 1000.0 / 30.0] {
        let ns = ms * NS_PER_MS;
        if ns <= scale_ns {
            grid.push(view.to_local(Vec2::new(view.min.x, y(ns))));
            grid.push(view.to_local(Vec2::new(view.min.x + view.size.x, y(ns))));
        }
    }
    set_points(ent, ui.histogram_grid, grid);

    let label = if profiler.frames.is_empty() {
        String::new()
    } else {
        format!(
            "max {} ms | lines: 16.7 ms, 33.3 ms",
            format_ms(max_ns as f64 / NS_PER_MS, 0.01)
        )
    };
    set_text(ent, ui.histogram_label, &label, TEXT_COLOR);
}

fn draw_ruler(ent: &mut EntitiesHolderMut, ui: &UiEntities, view: &LanesView, ruler_rect: &GlobalRectComponent, origin_ns: f64) {
    let ruler_min = ruler_rect.center - ruler_rect.scale * 0.5;
    let to_ruler_x = |x: f32| (x - ruler_min.x) / ruler_rect.scale.x.max(1.0);

    let px_per_ms = view.inner_width() as f64 / (view.span() / NS_PER_MS);
    let step_ms = ruler_step_ms(px_per_ms);

    let mut ticks = Vec::new();
    let mut labels = ui.ruler_labels.iter();
    let mut ms = ((view.view.start_ns - origin_ns) / NS_PER_MS / step_ms).ceil() * step_ms;

    while origin_ns + ms * NS_PER_MS <= view.view.end_ns {
        let x = view.x(origin_ns + ms * NS_PER_MS);
        ticks.push(Vec2::new(to_ruler_x(x), 0.6));
        ticks.push(Vec2::new(to_ruler_x(x), 1.0));

        if let Some(ent_label) = labels.next() {
            set_text(ent, *ent_label, &format!("{} ms", format_ms(ms, step_ms)), TEXT_COLOR);
            place(
                ent,
                *ent_label,
                Vec2::new(x - ruler_min.x + 3.0, 0.0),
                Vec2::new(80.0, PROFILER_RULER_HEIGHT),
            );
        }
        ms += step_ms;
    }
    for ent_label in labels {
        set_text(ent, *ent_label, "", TEXT_COLOR);
    }

    set_points(ent, ui.ruler_ticks, ticks);
}

fn ruler_step_ms(px_per_ms: f64) -> f64 {
    let mut step = 0.0001;
    loop {
        for factor in [1.0, 2.0, 5.0] {
            if step * factor * px_per_ms >= 70.0 || step * factor >= 10_000.0 {
                return step * factor;
            }
        }
        step *= 10.0;
    }
}

fn format_ms(ms: f64, step: f64) -> String {
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    format!("{ms:.decimals$}")
}

fn set_lines(ent: &mut EntitiesHolderMut, entity: EntityId, points: Vec<Vec2<f32>>, thickness_px: f32) {
    if let Some(line_c) = ent.get_component_mut::<UiLineComponent>(entity) {
        line_c.points = points.into();
        line_c.thickness_px = thickness_px;
    }
}

fn place(ent: &mut EntitiesHolderMut, entity: EntityId, offset: Vec2<f32>, size: Vec2<f32>) {
    if let Some(rect_c) = ent.get_component_mut::<LocalRectComponent>(entity) {
        rect_c.offset = offset.map(UiVal::px);
        rect_c.scale = Vec2::new(UiVal::px(size.x).into(), UiVal::px(size.y).into());
    }
}

fn lane_name(thread: u32, frame_threads: u32) -> String {
    match thread {
        0 => "main".into(),
        n if n <= frame_threads => format!("worker {n}"),
        n => format!("background {}", n - frame_threads),
    }
}

fn kind_name(kind: ProfileSpanKind) -> &'static str {
    match kind {
        ProfileSpanKind::System => "System",
        ProfileSpanKind::RenderCollect => "Render feature (collect)",
        ProfileSpanKind::RenderPass => "Render feature (pass)",
        ProfileSpanKind::BatchTask => "Job (batch task)",
        ProfileSpanKind::ScopeJob => "Job (scope)",
        ProfileSpanKind::BackgroundJob => "Job (background)",
    }
}

fn job_type(kind: ProfileSpanKind) -> &'static str {
    match kind {
        ProfileSpanKind::BatchTask => "batch task",
        ProfileSpanKind::ScopeJob => "scope",
        _ => "background",
    }
}

fn schedule_name(tag: u64) -> &'static str {
    match tag {
        0 => "Start",
        1 => "FixedUpdate",
        2 => "Update",
        _ => "?",
    }
}

fn span_title(span: &ProfileSpan) -> String {
    match span.kind.is_job() {
        false => format!("{} | {}", kind_name(span.kind), span.name.as_str()),
        true => format!("{} | started by {}", kind_name(span.kind), span.name.as_str()),
    }
}

fn span_details(frame: &ProfiledFrame, span: &ProfileSpan) -> String {
    let context = match span.kind {
        ProfileSpanKind::System => format!("schedule {}", schedule_name(span.tag)),
        ProfileSpanKind::RenderCollect => "add_passes".into(),
        ProfileSpanKind::RenderPass => format!("pass order {}", span.tag),
        ProfileSpanKind::BatchTask => format!("task {}", span.tag),
        ProfileSpanKind::ScopeJob | ProfileSpanKind::BackgroundJob => String::new(),
    };
    let start_ms = (span.start_ns as f64 - frame.start_ns as f64) / NS_PER_MS;

    let mut details = format!(
        "{} | frame {} | start {} ms | time {} ms",
        lane_name(span.thread, frame.frame_threads),
        frame.number,
        format_ms(start_ms, 0.001),
        format_ms(span.duration_ns() as f64 / NS_PER_MS, 0.001),
    );
    if !context.is_empty() {
        details += " | ";
        details += &context;
    }
    details
}

// the last path segment of a system's type name, without generic arguments
fn short_name(name: &str) -> &str {
    let name = name.split('<').next().unwrap_or(name);
    name.rsplit("::").next().unwrap_or(name)
}

fn span_label(span: &ProfileSpan) -> String {
    let name = short_name(span.name.as_str());

    match span.kind {
        ProfileSpanKind::System | ProfileSpanKind::RenderPass => name.into(),
        ProfileSpanKind::RenderCollect => format!("{name} (collect)"),
        kind => format!("{name} (job: {})", job_type(kind)),
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.into();
    }

    let mut truncated: String = text.chars().take(max_chars.saturating_sub(2)).collect();
    truncated.push_str("..");
    truncated
}

// a filled pixel rect as 1 px wide lines: the line mesh extends each line by half its thickness, only 0.5 px here
fn push_rect(points: &mut Vec<Vec2<f32>>, view: &LanesView, x0: f32, x1: f32, y0: f32, y1: f32) {
    if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
        return;
    }

    if x1 - x0 < 2.0 {
        points.push(view.to_local(Vec2::new(x0 + 0.5, y0 + 0.5)));
        points.push(view.to_local(Vec2::new(x0 + 0.5, y1 - 0.5)));
        return;
    }

    let mut y = y0 + 0.5;
    while y < y1 {
        points.push(view.to_local(Vec2::new(x0 + 0.5, y)));
        points.push(view.to_local(Vec2::new(x1 - 0.5, y)));
        y += 1.0;
    }
}
