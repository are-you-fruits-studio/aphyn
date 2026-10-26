use fruits_ecs::Resource;
use fruits_math::{Mat3, Mat4, Vec3};
use fruits_serialization::*;
use wgpu::{Texture, TextureFormat, TextureView};

use crate::StandardLight;

pub const RENDER_TARGET_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub const DEPTH_TEXTURE_FORMAT: TextureFormat = TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Debug, Serializable)]
pub struct Camera {
    pub projection: CameraProjection,
    pub near: f32,
    pub far: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            projection: CameraProjection::default(),
            near: 0.1,
            far: 1000.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Serializable)]
pub enum CameraProjection {
    Perspective { fov: f32 },
    Orthographic { height: f32 },
}
impl Default for CameraProjection {
    fn default() -> Self {
        Self::Perspective { fov: 90_f32.to_radians() }
    }
}

impl Camera {
    pub fn projection_matrix(&self, aspect: f32) -> Mat4<f32> {
        match self.projection {
            CameraProjection::Perspective { fov } => fruits_math::perspective_proj_matrix(fov, self.near, self.far, aspect),
            CameraProjection::Orthographic { height } => fruits_math::orthographic_proj_matrix(height, self.near, self.far, aspect),
        }
    }
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

        let projection_matrix = self.camera.projection_matrix(aspect);

        let view_matrix = self
            .camera_scale_rotation
            .into_4x4_with_offset(self.camera_position)
            .inverse()
            .unwrap();

        projection_matrix * view_matrix
    }
}
