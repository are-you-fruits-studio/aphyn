use fruits_math::{Vec3, Vec4};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GizmoLine {
    pub start: Vec3<f32>,
    pub end: Vec3<f32>,
    // todo: transparency.
    pub color: Vec4<f32>,
}
