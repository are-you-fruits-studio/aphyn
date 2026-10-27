use fruits_engine::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        inspector_window::utils::find_in_parents,
        ui_interaction::{ButtonComponent, DragAreaComponent, HoverAreaComponent, UiPressResource, check_ui_interaction_system},
    },
    prefabs::WindowComponent,
};

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(WindowsOrderResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(drag_window_system)
        .insert_child_system(close_window_system)
        .insert_child_system(highlit_window_scale_handles_system)
        .insert_child_system(rescale_window_system)
        .insert_child_system(keep_windows_inside_dock_system)
        .insert_child_system(order_windows_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(close_window_system)
        .before_system(drag_window_system)
        .before_system(highlit_window_scale_handles_system)
        .before_system(rescale_window_system);

    update.order_system(drag_window_system).before_system(keep_windows_inside_dock_system);
    update.order_system(rescale_window_system).before_system(keep_windows_inside_dock_system);
    update.order_system(check_ui_interaction_system).before_system(order_windows_system);
    update.order_system(close_window_system).before_system(order_windows_system);
}

const MIN_WINDOW_SIZE: Vec2<f32> = Vec2::new(100.0, 60.0);

pub const WINDOWS_FRONT_Z: f32 = -8_000.0;
pub const WINDOWS_Z_BAND: f32 = 1_000.0;

#[derive(Component, Default, Debug)]
pub struct DockComponent;

#[derive(Resource, Default)]
pub struct WindowsOrderResource {
    pub front_to_back: Vec<EntityId>,
}

pub fn order_windows_system(
    press_res: Res<UiPressResource>,
    mut order_res: ResMut<WindowsOrderResource>,
    mut window_rect_q: WorldQuery<(EntityId, &mut LocalRectComponent), WithFilter<WindowComponent>>,
    window_q: WorldQuery<&WindowComponent>,
    child_q: WorldQuery<&ChildComponent>,
) {
    let order = &mut order_res.front_to_back;

    order.retain(|&ent| window_q.get(ent).is_some());

    for (ent, _) in window_rect_q.iter_mut() {
        if !order.contains(&ent) {
            order.insert(0, ent);
        }
    }

    if let Some(ent_pressed) = press_res.pressed_this_frame {
        let is_window = |e: EntityId| window_q.get(e).is_some().then_some(e);
        let ent_window = is_window(ent_pressed).or_else(|| find_in_parents(child_q, ent_pressed, is_window));

        if let Some(ent_window) = ent_window
            && let Some(idx) = order.iter().position(|&e| e == ent_window)
        {
            let ent_window = order.remove(idx);
            order.insert(0, ent_window);
        }
    }

    for (rank, &ent) in order.iter().enumerate() {
        if let Some((_, rect_c)) = window_rect_q.get_mut(ent) {
            rect_c.z = WINDOWS_FRONT_Z + rank as f32 * WINDOWS_Z_BAND;
        }
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    min: Vec2<f32>,
    max: Vec2<f32>,
}

impl Bounds {
    const UNBOUNDED: Self = Self {
        min: Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        max: Vec2::new(f32::INFINITY, f32::INFINITY),
    };
}

fn dock_bounds(dock_q: &WorldQuery<&GlobalRectComponent, WithFilter<DockComponent>>) -> Option<Bounds> {
    let dock_rect = dock_q.iter().next()?;

    if dock_rect.scale.x <= 0.0 || dock_rect.scale.y <= 0.0 {
        return None;
    }

    Some(Bounds {
        min: dock_rect.center - dock_rect.scale * 0.5,
        max: dock_rect.center + dock_rect.scale * 0.5,
    })
}

fn px_offset(rect: &LocalRectComponent, axis: usize) -> f32 {
    match rect.offset[axis] {
        UiVal { unit: UiUnit::Px, val } => val,
        _ => 0.0,
    }
}

pub fn drag_window_system(
    input_res: Res<InputResource>,
    mut window_q: WorldQuery<(&mut LocalRectComponent, &WindowComponent)>,
    drag_area_q: WorldQuery<(&DragAreaComponent, &GlobalRectComponent)>,
    dock_q: WorldQuery<&GlobalRectComponent, WithFilter<DockComponent>>,
) {
    let bounds = dock_bounds(&dock_q).unwrap_or(Bounds::UNBOUNDED);

    for (window_local_rect, window_c) in window_q.iter_mut() {
        let Some((drag_area_c, drag_area_rect)) = drag_area_q.get(window_c.drag_header) else {
            continue;
        };

        let Some(press_global_offset) = drag_area_c.press_global_offset else {
            continue;
        };

        let expected_pos = Vec2::from_array(input_res.mouse.position.map(|v| v as f32)) - press_global_offset;
        let real_pos = drag_area_rect.center;
        let old_offset = window_local_rect.offset.map(|u| match u.unit {
            UiUnit::Px => u.val,
            _ => 0.0,
        });
        let new_offset = old_offset + expected_pos - real_pos;
        window_local_rect.offset = Vec2::from_fn(|axis| {
            let size = px_scale(window_local_rect, axis);
            UiVal::px(new_offset[axis].min(bounds.max[axis] - size).max(bounds.min[axis]))
        });
    }
}

pub fn close_window_system(mut world: WorldDataMut) {
    let mut entities_to_delete = Vec::new();

    for (ent_window, window_c) in world.entities().query::<(EntityId, &WindowComponent)>().iter() {
        let Some(button_c) = world.entities().get_component::<ButtonComponent>(window_c.close_button) else {
            continue;
        };

        if button_c.was_clicked_this_frame {
            entities_to_delete.push(ent_window);
        }
    }

    for ent_to_delete in entities_to_delete {
        destroy_entity_and_children(world.entities_mut(), ent_to_delete);
    }
}

pub fn highlit_window_scale_handles_system(
    mut handle_q: WorldQuery<(&HoverAreaComponent, &mut ImageComponent)>,
    window_q: WorldQuery<&WindowComponent>,
) {
    for window_c in window_q.iter() {
        let ent_scale_borders = [
            window_c.scale_border_left,
            window_c.scale_border_right,
            window_c.scale_border_top,
            window_c.scale_border_bottom,
            window_c.scale_corner_top_left,
            window_c.scale_corner_top_right,
            window_c.scale_corner_bottom_left,
            window_c.scale_corner_bottom_right,
        ];

        for ent_scale_border in ent_scale_borders {
            let Some((hover_c, image_c)) = handle_q.get_mut(ent_scale_border) else {
                continue;
            };
            
            image_c.color.w = match hover_c.is_hovered {
                true => 1.0,
                false => 0.0,
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ScaleEdge {
    Min,
    Max,
}

pub fn rescale_window_system(
    input_res: Res<InputResource>,
    handle_q: WorldQuery<(&DragAreaComponent, &GlobalRectComponent)>,
    mut window_q: WorldQuery<(&mut LocalRectComponent, &WindowComponent)>,
    dock_q: WorldQuery<&GlobalRectComponent, WithFilter<DockComponent>>,
) {
    let bounds = dock_bounds(&dock_q).unwrap_or(Bounds::UNBOUNDED);
    let mouse_pos = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));

    for (window_rect, window_c) in window_q.iter_mut() {
        let handles = [
            (window_c.scale_border_left, [Some(ScaleEdge::Min), None]),
            (window_c.scale_border_right, [Some(ScaleEdge::Max), None]),
            (window_c.scale_border_top, [None, Some(ScaleEdge::Min)]),
            (window_c.scale_border_bottom, [None, Some(ScaleEdge::Max)]),
            (window_c.scale_corner_top_left, [Some(ScaleEdge::Min), Some(ScaleEdge::Min)]),
            (window_c.scale_corner_top_right, [Some(ScaleEdge::Max), Some(ScaleEdge::Min)]),
            (window_c.scale_corner_bottom_left, [Some(ScaleEdge::Min), Some(ScaleEdge::Max)]),
            (window_c.scale_corner_bottom_right, [Some(ScaleEdge::Max), Some(ScaleEdge::Max)]),
        ];

        for (handle, edges) in handles {
            let Some((drag_area_c, handle_rect)) = handle_q.get(handle) else {
                continue;
            };

            let Some(press_global_offset) = drag_area_c.press_global_offset else {
                continue;
            };

            let diff = mouse_pos - press_global_offset - handle_rect.center;

            for (axis, edge) in edges.into_iter().enumerate() {
                match edge {
                    Some(ScaleEdge::Min) => drag_min_edge(window_rect, axis, diff[axis], bounds),
                    Some(ScaleEdge::Max) => drag_max_edge(window_rect, axis, diff[axis], bounds),
                    None => {}
                }
            }
        }
    }
}

fn px_scale(rect: &LocalRectComponent, axis: usize) -> f32 {
    match rect.scale[axis].into_option() {
        Some(UiVal { unit: UiUnit::Px, val }) => val,
        _ => 0.0,
    }
}

fn drag_max_edge(rect: &mut LocalRectComponent, axis: usize, diff: f32, bounds: Bounds) {
    let offset = px_offset(rect, axis);
    let size = (px_scale(rect, axis) + diff).min(bounds.max[axis] - offset).max(MIN_WINDOW_SIZE[axis]);
    rect.scale[axis] = UiVal::px(size).into();
}

fn drag_min_edge(rect: &mut LocalRectComponent, axis: usize, diff: f32, bounds: Bounds) {
    let old_offset = px_offset(rect, axis);
    let max_edge = old_offset + px_scale(rect, axis);
    let offset = (old_offset + diff).max(bounds.min[axis]).min(max_edge - MIN_WINDOW_SIZE[axis]);
    rect.scale[axis] = UiVal::px(max_edge - offset).into();
    rect.offset[axis] = UiVal::px(offset);
}

pub fn keep_windows_inside_dock_system(
    mut window_q: WorldQuery<&mut LocalRectComponent, WithFilter<WindowComponent>>,
    dock_q: WorldQuery<&GlobalRectComponent, WithFilter<DockComponent>>,
) {
    let Some(bounds) = dock_bounds(&dock_q) else {
        return;
    };

    for rect in window_q.iter_mut() {
        for axis in 0..2 {
            let old_size = px_scale(rect, axis);
            let old_offset = px_offset(rect, axis);

            let size = old_size.min(bounds.max[axis] - bounds.min[axis]).max(MIN_WINDOW_SIZE[axis]);
            let offset = old_offset.min(bounds.max[axis] - size).max(bounds.min[axis]);

            if size != old_size {
                rect.scale[axis] = UiVal::px(size).into();
            }
            if offset != old_offset {
                rect.offset[axis] = UiVal::px(offset);
            }
        }
    }
}
