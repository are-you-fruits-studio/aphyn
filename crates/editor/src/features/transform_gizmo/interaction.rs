use std::f32::consts::TAU;

use aphyn::*;

use super::{
    GizmoHandle, GizmoTool, TransformGizmoResource,
    handles::{GizmoFrame, GizmoView, HandlePart, HandleShape, RING_RADIUS, handle_parts, perpendicular_basis},
};
use crate::features::{
    inspector_window::data::{AssetEditedEvent, InspectedEntityResource},
    scene_window::{SceneCameraResource, SceneViewImageComponent, view_size},
    ui_interaction::HoverAreaComponent,
    world_preload::SimulatedWorldResource,
};

const PICK_DISTANCE_PX: f32 = 8.0;
const CENTER_PICK_BONUS_PX: f32 = 3.0;
const RING_BACK_PICK_PENALTY_PX: f32 = 4.0;
const RING_PICK_SAMPLES: usize = 64;
const EDGE_ON_DOT: f32 = 0.15;
const UNIFORM_SCALE_PER_PX: f32 = 0.01;
const MIN_SCALE_FACTOR: f32 = 1e-3;

pub struct GizmoDrag {
    pub handle: GizmoHandle,
    entity: EntityId,
    tool: GizmoTool,
    frame: GizmoFrame,
    start_local: LocalTransform,
    parent: GlobalTransform,
    start_mouse: Vec2<f32>,
    start: DragStart,
}

enum DragStart {
    AxisParam(f32),
    PlaneHit(Vec3<f32>),
    RingPlane(Vec3<f32>),
    RingTangent { tangent_px: Vec2<f32>, radius_px: f32 },
    ViewRing { center_px: Vec2<f32> },
    UniformScale,
}

pub fn transform_gizmo_interaction_system(
    input: Res<InputResource>,
    camera_res: Res<SceneCameraResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut gizmo_res: ResMut<TransformGizmoResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
    image_q: WorldQuery<(&GlobalRectComponent, &HoverAreaComponent), WithFilter<SceneViewImageComponent>>,
) {
    let gizmo_res = &mut *gizmo_res;
    let selected = inspected_entity.selected_entity;

    gizmo_res.hovered = None;

    if gizmo_res
        .drag
        .as_ref()
        .is_some_and(|drag| drag.entity != selected || drag.tool != gizmo_res.tool || !input.mouse.is_pressed(MouseButton::Left))
    {
        gizmo_res.drag = None;
    }

    let (Some(simulated_world), Some((rect_c, hover_c))) = (simulated_world.0.as_mut(), image_q.iter().next()) else {
        gizmo_res.drag = None;
        return;
    };

    let data = &mut simulated_world.data;

    let (Some(global), Some(start_local)) = (
        data.entities().get_component::<GlobalTransform>(selected).copied(),
        data.entities().get_component::<LocalTransform>(selected).copied(),
    ) else {
        gizmo_res.drag = None;
        return;
    };

    return_if_not!(Some(view) = GizmoView::new(&camera_res, view_size(rect_c)));

    let mouse = Vec2::from_array(input.mouse.position.map(|v| v as f32)) - (rect_c.center - rect_c.scale * 0.5);

    if let Some(drag) = &gizmo_res.drag {
        return_if_not!(Some(local) = dragged_local(drag, &view, mouse));

        let mut ent = data.entities_mut();
        if let Some(local_c) = ent.get_component_mut::<LocalTransform>(selected)
            && *local_c != local
        {
            *local_c = local;
            asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
        }

        return;
    }

    return_if!(!hover_c.is_hovered || camera_res.is_controlling);

    let frame = GizmoFrame::new(gizmo_res.tool, gizmo_res.space, &global, &view);
    gizmo_res.hovered = pick_handle(&handle_parts(gizmo_res.tool, &frame, None), &view, mouse);

    return_if!(!input.mouse.is_just_pressed(MouseButton::Left));
    return_if_not!(Some(handle) = gizmo_res.hovered);
    return_if_not!(Some(start) = drag_start(gizmo_res.tool, handle, &frame, &view, mouse));

    let ent = data.entities();
    let parent = ent
        .get_component::<ChildComponent>(selected)
        .and_then(|child_c| ent.get_component::<GlobalTransform>(child_c.parent()))
        .copied()
        .unwrap_or(GlobalTransform::IDENTITY);

    gizmo_res.drag = Some(GizmoDrag {
        handle,
        entity: selected,
        tool: gizmo_res.tool,
        frame,
        start_local,
        parent,
        start_mouse: mouse,
        start,
    });
}

fn pick_handle(parts: &[HandlePart], view: &GizmoView, mouse: Vec2<f32>) -> Option<GizmoHandle> {
    let mut best: Option<(f32, GizmoHandle)> = None;

    for part in parts {
        continue_if_not!(Some(distance) = shape_screen_distance(&part.shape, view, mouse));
        continue_if!(distance > PICK_DISTANCE_PX);

        let bonus = if part.handle == GizmoHandle::Center {
            CENTER_PICK_BONUS_PX
        } else {
            0.0
        };
        let score = distance - bonus;

        if best.is_none_or(|(best_score, _)| score < best_score) {
            best = Some((score, part.handle));
        }
    }

    best.map(|(_, handle)| handle)
}

fn shape_screen_distance(shape: &HandleShape, view: &GizmoView, mouse: Vec2<f32>) -> Option<f32> {
    match *shape {
        HandleShape::Cylinder { start, end, .. } => Some(distance_to_segment(
            mouse,
            view.world_to_view(start)?,
            view.world_to_view(end)?,
        )),
        HandleShape::Cone { base, tip, radius } => {
            let radius_px = radius / view.world_size_of_pixel(base);
            let distance = distance_to_segment(mouse, view.world_to_view(base)?, view.world_to_view(tip)?);
            Some((distance - radius_px).max(0.0))
        }
        HandleShape::Cube { center, half, .. } => {
            let half_px = half / view.world_size_of_pixel(center);
            Some(((mouse - view.world_to_view(center)?).length() as f32 - half_px * 1.2).max(0.0))
        }
        HandleShape::Quad { corner, u, v } => {
            let a = view.world_to_view(corner)?;
            let b = view.world_to_view(corner + u)?;
            let c = view.world_to_view(corner + u + v)?;
            let d = view.world_to_view(corner + v)?;

            if is_in_triangle(mouse, a, b, c) || is_in_triangle(mouse, a, c, d) {
                return Some(0.0);
            }

            [(a, b), (b, c), (c, d), (d, a)]
                .into_iter()
                .map(|(p, q)| distance_to_segment(mouse, p, q))
                .reduce(f32::min)
        }
        HandleShape::Ring {
            center, normal, radius, ..
        } => {
            let (u, v) = perpendicular_basis(normal);
            let point = |i: usize| {
                let angle = i as f32 / RING_PICK_SAMPLES as f32 * TAU;
                center + (u * angle.cos() + v * angle.sin()) * radius
            };

            let mut best: Option<f32> = None;

            for i in 0..RING_PICK_SAMPLES {
                let (p, q) = (point(i), point(i + 1));
                continue_if_not!(Some(p_px) = view.world_to_view(p));
                continue_if_not!(Some(q_px) = view.world_to_view(q));

                let is_back = ((p + q) * 0.5 - center).dot(view.forward) > 1e-4 * radius;
                let penalty = if is_back { RING_BACK_PICK_PENALTY_PX } else { 0.0 };
                let distance = distance_to_segment(mouse, p_px, q_px) + penalty;

                best = Some(best.map_or(distance, |best| best.min(distance)));
            }

            best
        }
    }
}

fn drag_start(tool: GizmoTool, handle: GizmoHandle, frame: &GizmoFrame, view: &GizmoView, mouse: Vec2<f32>) -> Option<DragStart> {
    let (ray_origin, ray_dir) = view.ray(mouse);

    match (tool, handle) {
        (GizmoTool::Move | GizmoTool::Scale, GizmoHandle::Axis(i)) => {
            axis_param(frame.origin, frame.axes[i], ray_origin, ray_dir).map(DragStart::AxisParam)
        }
        (GizmoTool::Move, GizmoHandle::Plane(i)) => {
            ray_plane(ray_origin, ray_dir, frame.origin, frame.axes[i]).map(DragStart::PlaneHit)
        }
        (GizmoTool::Move, GizmoHandle::Center) => {
            ray_plane(ray_origin, ray_dir, frame.origin, frame.forward).map(DragStart::PlaneHit)
        }
        (GizmoTool::Scale, GizmoHandle::Center) => Some(DragStart::UniformScale),
        (GizmoTool::Rotate, GizmoHandle::Axis(i)) => {
            let axis = frame.axes[i];

            if ray_dir.dot(axis).abs() > EDGE_ON_DOT {
                return ray_plane(ray_origin, ray_dir, frame.origin, axis).map(|hit| DragStart::RingPlane(hit - frame.origin));
            }

            // the ring is seen edge-on, so the drag follows the ring's screen-space tangent at the grabbed point
            let radius = RING_RADIUS * frame.size;
            let (u, v) = perpendicular_basis(axis);

            let grabbed = (0..RING_PICK_SAMPLES)
                .map(|i| {
                    let angle = i as f32 / RING_PICK_SAMPLES as f32 * TAU;
                    frame.origin + (u * angle.cos() + v * angle.sin()) * radius
                })
                .filter_map(|p| Some((p, (view.world_to_view(p)? - mouse).length())))
                .min_by(|(_, a), (_, b)| a.total_cmp(b))?
                .0;

            let tangent = axis.cross(grabbed - frame.origin).normalized() * (radius * 0.1);
            let tangent_px = view.world_to_view(grabbed + tangent)? - view.world_to_view(grabbed)?;
            if tangent_px.length_sq() < 1e-6 {
                return None;
            }

            Some(DragStart::RingTangent {
                tangent_px: tangent_px.normalized(),
                radius_px: radius / view.world_size_of_pixel(frame.origin),
            })
        }
        (GizmoTool::Rotate, GizmoHandle::ViewRing) => Some(DragStart::ViewRing {
            center_px: view.world_to_view(frame.origin)?,
        }),
        _ => None,
    }
}

fn dragged_local(drag: &GizmoDrag, view: &GizmoView, mouse: Vec2<f32>) -> Option<LocalTransform> {
    let (ray_origin, ray_dir) = view.ray(mouse);
    let frame = &drag.frame;
    let start = &drag.start_local;
    let mut local = *start;

    match (drag.tool, drag.handle, &drag.start) {
        (GizmoTool::Move, GizmoHandle::Axis(i), DragStart::AxisParam(start_t)) => {
            let t = axis_param(frame.origin, frame.axes[i], ray_origin, ray_dir)?;
            local.position = local_position(&drag.parent, frame.origin + frame.axes[i] * (t - start_t))?;
        }
        (GizmoTool::Move, handle, DragStart::PlaneHit(start_hit)) => {
            let normal = match handle {
                GizmoHandle::Plane(i) => frame.axes[i],
                _ => frame.forward,
            };
            let hit = ray_plane(ray_origin, ray_dir, frame.origin, normal)?;
            local.position = local_position(&drag.parent, frame.origin + (hit - *start_hit))?;
        }
        (GizmoTool::Scale, GizmoHandle::Axis(i), DragStart::AxisParam(start_t)) => {
            if start_t.abs() < 1e-6 {
                return None;
            }
            let t = axis_param(frame.origin, frame.axes[i], ray_origin, ray_dir)?;
            local.scale[i] = start.scale[i] * non_zero_factor(t / start_t);
        }
        (GizmoTool::Scale, GizmoHandle::Center, DragStart::UniformScale) => {
            let delta = mouse - drag.start_mouse;
            let factor = (1.0 + (delta.x - delta.y) * UNIFORM_SCALE_PER_PX).max(MIN_SCALE_FACTOR);
            local.scale = start.scale * factor;
        }
        (GizmoTool::Rotate, GizmoHandle::Axis(i), DragStart::RingPlane(start_vec)) => {
            let axis = frame.axes[i];
            let hit_vec = ray_plane(ray_origin, ray_dir, frame.origin, axis)? - frame.origin;
            let angle = start_vec.cross(hit_vec).dot(axis).atan2(start_vec.dot(hit_vec));
            local.rotation = rotated_local(start, &drag.parent, axis, angle);
        }
        (GizmoTool::Rotate, GizmoHandle::Axis(i), DragStart::RingTangent { tangent_px, radius_px }) => {
            let angle = (mouse - drag.start_mouse).dot(*tangent_px) / radius_px.max(1.0);
            local.rotation = rotated_local(start, &drag.parent, frame.axes[i], angle);
        }
        (GizmoTool::Rotate, GizmoHandle::ViewRing, DragStart::ViewRing { center_px }) => {
            let (a, b) = (drag.start_mouse - *center_px, mouse - *center_px);
            // view y points down, so a counter-clockwise screen rotation is a negative screen angle
            let screen_angle = (a.x * b.y - a.y * b.x).atan2(a.dot(b));
            local.rotation = rotated_local(start, &drag.parent, frame.forward, -screen_angle);
        }
        _ => return None,
    }

    Some(local)
}

fn local_position(parent: &GlobalTransform, global_position: Vec3<f32>) -> Option<Vec3<f32>> {
    Some(parent.scale_rotation.inverse()? * (global_position - parent.position))
}

fn rotated_local(start: &LocalTransform, parent: &GlobalTransform, world_axis: Vec3<f32>, angle: f32) -> Quat<f32> {
    let (_, parent_rotation) = parent.scale_rotation.to_lossy_scale_rotation();
    let mut parent_inverse = parent_rotation.normalized().to_matrix();
    parent_inverse.transpose();

    let delta = Quat::rotation_axis_angle(parent_inverse * world_axis, angle as f64);

    Quat::from_matrix(delta.to_matrix() * start.rotation.normalized().to_matrix()).normalized()
}

fn non_zero_factor(factor: f32) -> f32 {
    if factor.abs() < MIN_SCALE_FACTOR {
        MIN_SCALE_FACTOR.copysign(factor)
    } else {
        factor
    }
}

fn axis_param(origin: Vec3<f32>, axis: Vec3<f32>, ray_origin: Vec3<f32>, ray_dir: Vec3<f32>) -> Option<f32> {
    let offset = origin - ray_origin;
    let b = axis.dot(ray_dir);
    let denom = 1.0 - b * b;

    if denom < 1e-4 {
        return None;
    }

    Some((b * ray_dir.dot(offset) - axis.dot(offset)) / denom)
}

fn ray_plane(ray_origin: Vec3<f32>, ray_dir: Vec3<f32>, point: Vec3<f32>, normal: Vec3<f32>) -> Option<Vec3<f32>> {
    let denom = ray_dir.dot(normal);

    if denom.abs() < 1e-6 {
        return None;
    }

    let t = (point - ray_origin).dot(normal) / denom;

    (t >= 0.0).then(|| ray_origin + ray_dir * t)
}

fn distance_to_segment(p: Vec2<f32>, a: Vec2<f32>, b: Vec2<f32>) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    let t = if len_sq > 1e-9 {
        ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };

    (p - (a + ab * t)).length() as f32
}

fn is_in_triangle(p: Vec2<f32>, a: Vec2<f32>, b: Vec2<f32>, c: Vec2<f32>) -> bool {
    let side = |p1: Vec2<f32>, p2: Vec2<f32>| (p2.x - p1.x) * (p.y - p1.y) - (p2.y - p1.y) * (p.x - p1.x);
    let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));

    let has_negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;

    !(has_negative && has_positive)
}
