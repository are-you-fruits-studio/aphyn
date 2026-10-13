use fruits_ecs::Resource;
use fruits_math::{Mat3, Mat4, Vec3};
use wgpu::{Texture, TextureFormat, TextureView};

use crate::StandardLight;

pub const RENDER_TARGET_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    // todo: orthographic projection
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

// todo: support ffi
#[derive(Resource)]
pub struct CurrentRenderingResource {
    pub camera_position: Vec3<f32>,
    pub camera_scale_rotation: Mat3<f32>,
    pub camera: Camera,
    pub light: Option<StandardLight>,
    pub target_texture: Texture,
    pub target_view: TextureView,
}

impl CurrentRenderingResource {
    pub fn target_size(&self) -> [u32; 2] {
        let size = self.target_texture.size();
        [size.width, size.height]
    }

    pub fn world_to_clip(&self) -> Mat4<f32> {
        let [width, height] = self.target_size();
        let aspect = width as f32 / height as f32;

        let projection_matrix = fruits_math::perspective_proj_matrix(self.camera.fov, self.camera.near, self.camera.far, aspect);

        let view_matrix = self
            .camera_scale_rotation
            .into_4x4_with_offset(self.camera_position)
            .inverse()
            .unwrap();

        projection_matrix * view_matrix
    }
}
