use aphyn::*;

use super::{GizmoHandle, GizmoSpace, GizmoTool};
use crate::{features::scene_window::SceneCameraResource, theme::PAL};

pub const GIZMO_SCREEN_SIZE_PX: f32 = 110.0;

const SHAFT_RADIUS: f32 = 0.015;
const MOVE_SHAFT_END: f32 = 0.78;
const CONE_RADIUS: f32 = 0.07;
const PLANE_MIN: f32 = 0.2;
const PLANE_MAX: f32 = 0.45;
const MOVE_CENTER_HALF: f32 = 0.06;
pub const RING_RADIUS: f32 = 1.0;
const VIEW_RING_RADIUS: f32 = 1.18;
const RING_TUBE: f32 = 0.018;
const SCALE_SHAFT_END: f32 = 0.85;
const SCALE_CUBE_HALF: f32 = 0.06;
const SCALE_CENTER_HALF: f32 = 0.09;

pub struct GizmoView {
    pub camera_position: Vec3<f32>,
    pub forward: Vec3<f32>,
    pub camera: Camera,
    pub size: Vec2<f32>,
    world_to_clip: Mat4<f32>,
    clip_to_world: Mat4<f32>,
}

impl GizmoView {
    pub fn new(camera_res: &SceneCameraResource, size: [u32; 2]) -> Option<Self> {
        let size = Vec2::new(size[0] as f32, size[1] as f32);
        let rotation = camera_res.scale_rotation();

        let view_matrix = rotation.into_4x4_with_offset(camera_res.position).inverse()?;
        let world_to_clip = camera_res.camera.projection_matrix(size.x / size.y) * view_matrix;

        Some(Self {
            camera_position: camera_res.position,
            forward: (rotation * Vec3::new(0.0, 0.0, 1.0)).normalized(),
            camera: camera_res.camera,
            size,
            world_to_clip,
            clip_to_world: world_to_clip.inverse()?,
        })
    }

    pub fn world_to_view(&self, point: Vec3<f32>) -> Option<Vec2<f32>> {
        let clip = self.world_to_clip * Vec4::new(point.x, point.y, point.z, 1.0);
        if clip.w <= 1e-5 {
            return None;
        }

        let ndc = Vec2::new(clip.x, clip.y) / clip.w;

        Some(Vec2::new(
            (ndc.x + 1.0) * 0.5 * self.size.x,
            (1.0 - ndc.y) * 0.5 * self.size.y,
        ))
    }

    pub fn ray(&self, view_pos: Vec2<f32>) -> (Vec3<f32>, Vec3<f32>) {
        let x = view_pos.x / self.size.x * 2.0 - 1.0;
        let y = 1.0 - view_pos.y / self.size.y * 2.0;

        let near = self.clip_to_world.mul_with_projection(Vec3::new(x, y, 1.0));
        let far = self.clip_to_world.mul_with_projection(Vec3::new(x, y, 0.0));

        (near, (far - near).normalized())
    }

    pub fn world_size_of_pixel(&self, point: Vec3<f32>) -> f32 {
        match self.camera.projection {
            CameraProjection::Perspective { fov } => {
                let depth = (point - self.camera_position).dot(self.forward).max(self.camera.near);
                2.0 * depth * (fov * 0.5).tan() / self.size.y
            }
            CameraProjection::Orthographic { height } => height / self.size.y,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GizmoFrame {
    pub origin: Vec3<f32>,
    pub axes: [Vec3<f32>; 3],
    pub size: f32,
    pub forward: Vec3<f32>,
}

impl GizmoFrame {
    pub fn new(tool: GizmoTool, space: GizmoSpace, global: &GlobalTransform, view: &GizmoView) -> Self {
        let world_axes = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];

        let axes = match (tool, space) {
            (GizmoTool::Scale, _) | (_, GizmoSpace::Local) => std::array::from_fn(|i| {
                let column = Vec3::from_array(global.scale_rotation[i]);
                if column.length_sq() > 1e-12 {
                    column.normalized()
                } else {
                    world_axes[i]
                }
            }),
            _ => world_axes,
        };

        Self {
            origin: global.position,
            axes,
            size: view.world_size_of_pixel(global.position) * GIZMO_SCREEN_SIZE_PX,
            forward: view.forward,
        }
    }

    fn at(&self, axis: usize, t: f32) -> Vec3<f32> {
        self.origin + self.axes[axis] * (t * self.size)
    }
}

pub enum HandleShape {
    Cylinder {
        start: Vec3<f32>,
        end: Vec3<f32>,
        radius: f32,
    },
    Cone {
        base: Vec3<f32>,
        tip: Vec3<f32>,
        radius: f32,
    },
    Cube {
        center: Vec3<f32>,
        axes: [Vec3<f32>; 3],
        half: f32,
    },
    Quad {
        corner: Vec3<f32>,
        u: Vec3<f32>,
        v: Vec3<f32>,
    },
    Ring {
        center: Vec3<f32>,
        normal: Vec3<f32>,
        radius: f32,
        tube: f32,
    },
}

pub struct HandlePart {
    pub handle: GizmoHandle,
    pub shape: HandleShape,
    pub color: Vec4<f32>,
}

pub fn handle_parts(tool: GizmoTool, frame: &GizmoFrame, active: Option<GizmoHandle>) -> Vec<HandlePart> {
    let s = frame.size;
    let mut parts = Vec::new();

    let mut push = |handle: GizmoHandle, base_color: Vec4<f32>, shape: HandleShape| {
        let color = if active == Some(handle) { PAL.resin_300 } else { base_color };
        parts.push(HandlePart { handle, shape, color });
    };

    match tool {
        GizmoTool::None => {}
        GizmoTool::Move => {
            for i in 0..3 {
                let handle = GizmoHandle::Axis(i);
                push(
                    handle,
                    [PAL.axis_x, PAL.axis_y, PAL.axis_z][i],
                    HandleShape::Cylinder {
                        start: frame.origin,
                        end: frame.at(i, MOVE_SHAFT_END),
                        radius: SHAFT_RADIUS * s,
                    },
                );
                push(
                    handle,
                    [PAL.axis_x, PAL.axis_y, PAL.axis_z][i],
                    HandleShape::Cone {
                        base: frame.at(i, MOVE_SHAFT_END),
                        tip: frame.at(i, 1.0),
                        radius: CONE_RADIUS * s,
                    },
                );
            }

            for i in 0..3 {
                let (u, v) = ((i + 1) % 3, (i + 2) % 3);
                push(
                    GizmoHandle::Plane(i),
                    [PAL.axis_x, PAL.axis_y, PAL.axis_z][i],
                    HandleShape::Quad {
                        corner: frame.origin + (frame.axes[u] + frame.axes[v]) * (PLANE_MIN * s),
                        u: frame.axes[u] * ((PLANE_MAX - PLANE_MIN) * s),
                        v: frame.axes[v] * ((PLANE_MAX - PLANE_MIN) * s),
                    },
                );
            }

            push(
                GizmoHandle::Center,
                PAL.text_2,
                HandleShape::Cube {
                    center: frame.origin,
                    axes: frame.axes,
                    half: MOVE_CENTER_HALF * s,
                },
            );
        }
        GizmoTool::Rotate => {
            for i in 0..3 {
                push(
                    GizmoHandle::Axis(i),
                    [PAL.axis_x, PAL.axis_y, PAL.axis_z][i],
                    HandleShape::Ring {
                        center: frame.origin,
                        normal: frame.axes[i],
                        radius: RING_RADIUS * s,
                        tube: RING_TUBE * s,
                    },
                );
            }

            push(
                GizmoHandle::ViewRing,
                PAL.text_2,
                HandleShape::Ring {
                    center: frame.origin,
                    normal: frame.forward,
                    radius: VIEW_RING_RADIUS * s,
                    tube: RING_TUBE * s,
                },
            );
        }
        GizmoTool::Scale => {
            for i in 0..3 {
                let handle = GizmoHandle::Axis(i);
                push(
                    handle,
                    [PAL.axis_x, PAL.axis_y, PAL.axis_z][i],
                    HandleShape::Cylinder {
                        start: frame.origin,
                        end: frame.at(i, SCALE_SHAFT_END),
                        radius: SHAFT_RADIUS * s,
                    },
                );
                push(
                    handle,
                    [PAL.axis_x, PAL.axis_y, PAL.axis_z][i],
                    HandleShape::Cube {
                        center: frame.at(i, SCALE_SHAFT_END + SCALE_CUBE_HALF),
                        axes: frame.axes,
                        half: SCALE_CUBE_HALF * s,
                    },
                );
            }

            push(
                GizmoHandle::Center,
                PAL.text_2,
                HandleShape::Cube {
                    center: frame.origin,
                    axes: frame.axes,
                    half: SCALE_CENTER_HALF * s,
                },
            );
        }
    }

    parts
}

pub fn perpendicular_basis(normal: Vec3<f32>) -> (Vec3<f32>, Vec3<f32>) {
    let helper = if normal.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = normal.cross(helper).normalized();
    let v = normal.cross(u).normalized();

    (u, v)
}
