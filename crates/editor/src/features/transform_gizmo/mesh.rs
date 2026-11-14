use std::f32::consts::TAU;

use aphyn::*;

use super::{
    GizmoTool, TransformGizmoMeshResource, TransformGizmoResource,
    handles::{GizmoFrame, GizmoView, HandleShape, handle_parts, perpendicular_basis},
};
use crate::features::scene_window::SceneCameraResource;

const ROUND_SEGMENTS: usize = 16;
const RING_MAJOR_SEGMENTS: usize = 64;
const RING_MINOR_SEGMENTS: usize = 8;

pub fn build_transform_gizmo_mesh(
    gizmo_res: &TransformGizmoResource,
    selected_entity: EntityId,
    camera_res: &SceneCameraResource,
    sim_data: WorldDataRef,
    view_size: [u32; 2],
) -> Option<TransformGizmoMeshResource> {
    if gizmo_res.tool == GizmoTool::None {
        return None;
    }

    let ent = sim_data.entities();

    ent.get_component::<LocalTransform>(selected_entity)?;
    let global = ent.get_component::<GlobalTransform>(selected_entity)?;

    let view = GizmoView::new(camera_res, view_size)?;
    let frame = GizmoFrame::new(gizmo_res.tool, gizmo_res.space, global, &view);

    let mut builder = MeshBuilder::default();

    for part in handle_parts(gizmo_res.tool, &frame, gizmo_res.active_handle()) {
        builder.push_shape(&part.shape, part.color);
    }

    Some(TransformGizmoMeshResource {
        vertices: builder.vertices.into(),
        indices: builder.indices.into(),
    })
}

#[derive(Default)]
struct MeshBuilder {
    vertices: Vec<[f32; 10]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    fn push_shape(&mut self, shape: &HandleShape, color: Vec4<f32>) {
        match *shape {
            HandleShape::Cylinder { start, end, radius } => self.push_cylinder(start, end, radius, color),
            HandleShape::Cone { base, tip, radius } => self.push_cone(base, tip, radius, color),
            HandleShape::Cube { center, axes, half } => self.push_cube(center, axes, half, color),
            HandleShape::Quad { corner, u, v } => self.push_quad(corner, u, v, color),
            HandleShape::Ring {
                center,
                normal,
                radius,
                tube,
            } => self.push_torus(center, normal, radius, tube, color),
        }
    }

    fn push_vertex(&mut self, position: Vec3<f32>, normal: Vec3<f32>, color: Vec4<f32>) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push([
            position.x, position.y, position.z, normal.x, normal.y, normal.z, color.x, color.y, color.z, color.w,
        ]);
        index
    }

    fn push_quad_indices(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.indices.extend_from_slice(&[a, b, c, a, c, d]);
    }

    fn push_cylinder(&mut self, start: Vec3<f32>, end: Vec3<f32>, radius: f32, color: Vec4<f32>) {
        let (u, v) = perpendicular_basis((end - start).normalized());
        let first = self.vertices.len() as u32;

        for i in 0..ROUND_SEGMENTS {
            let angle = i as f32 / ROUND_SEGMENTS as f32 * TAU;
            let normal = u * angle.cos() + v * angle.sin();
            self.push_vertex(start + normal * radius, normal, color);
            self.push_vertex(end + normal * radius, normal, color);
        }

        for i in 0..ROUND_SEGMENTS as u32 {
            let next = (i + 1) % ROUND_SEGMENTS as u32;
            self.push_quad_indices(first + i * 2, first + next * 2, first + next * 2 + 1, first + i * 2 + 1);
        }
    }

    fn push_cone(&mut self, base: Vec3<f32>, tip: Vec3<f32>, radius: f32, color: Vec4<f32>) {
        let axis = (tip - base).normalized();
        let (u, v) = perpendicular_basis(axis);
        let base_center = self.push_vertex(base, -axis, color);
        let first = self.vertices.len() as u32;

        for i in 0..ROUND_SEGMENTS {
            let angle = i as f32 / ROUND_SEGMENTS as f32 * TAU;
            let radial = u * angle.cos() + v * angle.sin();
            let point = base + radial * radius;
            self.push_vertex(point, (radial + axis * 0.5).normalized(), color);
            self.push_vertex(tip, (radial + axis * 0.5).normalized(), color);
            self.push_vertex(point, -axis, color);
        }

        for i in 0..ROUND_SEGMENTS as u32 {
            let next = (i + 1) % ROUND_SEGMENTS as u32;
            self.indices
                .extend_from_slice(&[first + i * 3, first + next * 3, first + i * 3 + 1]);
            self.indices
                .extend_from_slice(&[base_center, first + next * 3 + 2, first + i * 3 + 2]);
        }
    }

    fn push_cube(&mut self, center: Vec3<f32>, axes: [Vec3<f32>; 3], half: f32, color: Vec4<f32>) {
        for face in 0..3 {
            for sign in [-1.0_f32, 1.0] {
                let normal = axes[face] * sign;
                let u = axes[(face + 1) % 3] * half;
                let v = axes[(face + 2) % 3] * half;
                let face_center = center + normal * half;

                let a = self.push_vertex(face_center - u - v, normal, color);
                let b = self.push_vertex(face_center + u - v, normal, color);
                let c = self.push_vertex(face_center + u + v, normal, color);
                let d = self.push_vertex(face_center - u + v, normal, color);
                self.push_quad_indices(a, b, c, d);
            }
        }
    }

    fn push_quad(&mut self, corner: Vec3<f32>, u: Vec3<f32>, v: Vec3<f32>, color: Vec4<f32>) {
        let normal = u.cross(v).normalized();

        let a = self.push_vertex(corner, normal, color);
        let b = self.push_vertex(corner + u, normal, color);
        let c = self.push_vertex(corner + u + v, normal, color);
        let d = self.push_vertex(corner + v, normal, color);
        self.push_quad_indices(a, b, c, d);
    }

    fn push_torus(&mut self, center: Vec3<f32>, normal: Vec3<f32>, radius: f32, tube: f32, color: Vec4<f32>) {
        let (u, v) = perpendicular_basis(normal);
        let first = self.vertices.len() as u32;

        for i in 0..RING_MAJOR_SEGMENTS {
            let major = i as f32 / RING_MAJOR_SEGMENTS as f32 * TAU;
            let radial = u * major.cos() + v * major.sin();
            let ring_center = center + radial * radius;

            for j in 0..RING_MINOR_SEGMENTS {
                let minor = j as f32 / RING_MINOR_SEGMENTS as f32 * TAU;
                let tube_normal = radial * minor.cos() + normal * minor.sin();
                self.push_vertex(ring_center + tube_normal * tube, tube_normal, color);
            }
        }

        let minor_count = RING_MINOR_SEGMENTS as u32;
        let index = |i: u32, j: u32| first + (i % RING_MAJOR_SEGMENTS as u32) * minor_count + j % minor_count;

        for i in 0..RING_MAJOR_SEGMENTS as u32 {
            for j in 0..minor_count {
                self.push_quad_indices(index(i, j), index(i + 1, j), index(i + 1, j + 1), index(i, j + 1));
            }
        }
    }
}
