use fruits_asset_storage::AssetHandle;
use fruits_ffi::{FfiDroppable, FfiOption};
use fruits_math::{Mat4, Vec3, Vec4};
use fruits_serialization::*;
use fruits_utils::mem::{AllBitVariationsValid, AllBitsInit};
use wgpu::{BindGroup, Buffer, BufferUsages, Sampler, TextureView, util::{BufferInitDescriptor, DeviceExt}};

use crate::{CreateBindGroupEntry, RenderState, StandardTexture};

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serializable)]
pub enum RenderSpace {
    Clip,
    Window,
    #[default]
    World,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StandardGenericLight {
    pub color: Vec3<f32>,
    pub light_type: u32,
    pub center: Vec3<f32>,
    pub range: f32,
    pub direction_dst: Vec3<f32>,
    pub fov: f32,
}

unsafe impl AllBitVariationsValid for StandardGenericLight {}
unsafe impl AllBitsInit for StandardGenericLight {}

impl Default for StandardGenericLight {
    fn default() -> Self {
        Self {
            color: Vec3::splat(0.0),
            light_type: 0,
            center: Vec3::splat(0.0),
            range: 0.0,
            direction_dst: Vec3::new(0.0, -1.0, 0.0),
            fov: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StandardUniformGlobal {
    pub camera_position_world: Vec3<f32>,
    pub lights_count: u32,
}

unsafe impl AllBitVariationsValid for StandardUniformGlobal {}
unsafe impl AllBitsInit for StandardUniformGlobal {}

impl Default for StandardUniformGlobal {
    fn default() -> Self {
        Self {
            camera_position_world: Vec3::splat(0.0),
            lights_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StandardUniformMaterial {
    pub world_to_clip: Mat4<f32>,
    pub color: Vec4<f32>,
    pub emission_color: Vec4<f32>,
    pub metallic: f32,
    pub roughness: f32,
    pub alpha_threshold: f32,
    pub _padding: [f32; 1],
}

unsafe impl AllBitVariationsValid for StandardUniformMaterial {}
unsafe impl AllBitsInit for StandardUniformMaterial {}

impl Default for StandardUniformMaterial {
    fn default() -> Self {
        Self {
            world_to_clip: Mat4::IDENTITY,
            color: Vec4::splat(0.5),
            emission_color: Vec4::splat(0.0),
            metallic: 0.5,
            roughness: 0.5,
            alpha_threshold: 0.5,
            _padding: Default::default(),
        }
    }
}

impl From<StandardLight> for StandardGenericLight {
    fn from(value: StandardLight) -> Self {
        match value {
            StandardLight::Point { color, center, range } => Self {
                light_type: 0,
                color,
                center,
                range,
                ..Default::default()
            },
            StandardLight::Spot { color, center, range, direction_dst, fov } => Self {
                light_type: 1,
                color,
                center,
                range,
                direction_dst,
                fov,
            },
            StandardLight::Directional { color, direction_dst } => Self {
                light_type: 2,
                color,
                direction_dst,
                ..Default::default()
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serializable)]
pub enum StandardLight {
    Point {
        color: Vec3<f32>,
        center: Vec3<f32>,
        range: f32,
    },
    Spot {
        color: Vec3<f32>,
        center: Vec3<f32>,
        range: f32,
        direction_dst: Vec3<f32>,
        fov: f32,
    },
    Directional {
        color: Vec3<f32>,
        direction_dst: Vec3<f32>,
    },
}

impl Default for StandardLight {
    fn default() -> Self {
        Self::Point {
            color: Vec3::splat(0.0),
            center: Vec3::splat(0.0),
            range: 0.0,
        }
    }
}

pub struct StandardMaterialNative {
    pub buffer_uniform: Buffer,
    pub bind_group: BindGroup,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, PartialOrd, Serializable)]
pub struct StandardMaterialAssetMetadata {
    pub is_lit: bool,
    pub space: RenderSpace,
    pub color: Vec4<f32>,
    pub emission_color: Vec4<f32>,
    pub metallic: f32,
    pub roughness: f32,
    pub alpha_threshold: FfiOption<f32>,
    pub color_tex: AssetHandle<StandardTexture>,
    pub roughness_tex: AssetHandle<StandardTexture>,
    pub metallic_tex: AssetHandle<StandardTexture>,
    pub normal_tex: AssetHandle<StandardTexture>,
    pub emission_tex: AssetHandle<StandardTexture>,
}

impl Default for StandardMaterialAssetMetadata {
    fn default() -> Self {
        Self {
            is_lit: false,
            space: RenderSpace::World,
            color: Vec4::splat(0.5),
            emission_color: Vec4::splat(0.0),
            metallic: 0.0,
            roughness: 0.5,
            alpha_threshold: Some(0.5).into(),
            color_tex: AssetHandle::EMPTY,
            roughness_tex: AssetHandle::EMPTY,
            metallic_tex: AssetHandle::EMPTY,
            normal_tex: AssetHandle::EMPTY,
            emission_tex: AssetHandle::EMPTY,
        }
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct StandardMaterialAssets<T> {
    pub color_texture: T,
    pub roughness_texture: T,
    pub metallic_texture: T,
    pub normal_texture: T,
    pub emission_texture: T,
}

impl<T> StandardMaterialAssets<T> {
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> StandardMaterialAssets<U> {
        StandardMaterialAssets {
            color_texture: f(self.color_texture),
            roughness_texture: f(self.roughness_texture),
            metallic_texture: f(self.metallic_texture),
            normal_texture: f(self.normal_texture),
            emission_texture: f(self.emission_texture),
        }
    }
}

#[repr(C)]
pub struct StandardMaterial {
    meta: StandardMaterialAssetMetadata,
    native: FfiDroppable,
}

impl StandardMaterial {
    pub(crate) fn new(
        render_state: &RenderState,
        assets: StandardMaterialAssets<&StandardTexture>,
        meta: StandardMaterialAssetMetadata,
    ) -> Self {
        let buffer_uniform = render_state.device().create_buffer_init(&BufferInitDescriptor {
            label: Some("Standard Material Uniform Buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            contents: fruits_utils::mem::as_bytes(&[StandardUniformMaterial::default()]),
        });

        let tex_color = unsafe { texture_to_view_and_sampler(assets.color_texture) };
        let tex_roughness = unsafe { texture_to_view_and_sampler(assets.roughness_texture) };
        let tex_metallic = unsafe { texture_to_view_and_sampler(assets.metallic_texture) };
        let tex_normal = unsafe { texture_to_view_and_sampler(assets.normal_texture) };
        let tex_emission = unsafe { texture_to_view_and_sampler(assets.emission_texture) };

        let bind_group = crate::utils::create_bind_group(
            render_state.device(),
            Some("Standard Material Bind Group"),
            &render_state.render_data().bind_group_layout_material,
            &[
                CreateBindGroupEntry::Buffer(&buffer_uniform),
                CreateBindGroupEntry::Texture(&tex_color.0),
                CreateBindGroupEntry::Sampler(tex_color.1),
                CreateBindGroupEntry::Texture(&tex_roughness.0),
                CreateBindGroupEntry::Sampler(tex_roughness.1),
                CreateBindGroupEntry::Texture(&tex_metallic.0),
                CreateBindGroupEntry::Sampler(tex_metallic.1),
                CreateBindGroupEntry::Texture(&tex_normal.0),
                CreateBindGroupEntry::Sampler(tex_normal.1),
                CreateBindGroupEntry::Texture(&tex_emission.0),
                CreateBindGroupEntry::Sampler(tex_emission.1),
            ],
        );

        Self {
            native: FfiDroppable::new(StandardMaterialNative {
                buffer_uniform,
                bind_group,
            }),
            meta: meta.into(),
        }
    }

    pub fn meta(&self) -> &StandardMaterialAssetMetadata {
        &self.meta
    }

    pub unsafe fn native(&self) -> &StandardMaterialNative {
        unsafe { &*(self.native.get() as *const StandardMaterialNative) }
    }
}

unsafe impl Send for StandardMaterial where StandardMaterialNative: Send {}
unsafe impl Sync for StandardMaterial where StandardMaterialNative: Sync {}

unsafe fn texture_to_view_and_sampler(texture: &StandardTexture) -> (TextureView, &Sampler) {
    let texture_view = unsafe { texture.native().texture.create_view(&Default::default()) };
    let sampler = unsafe { &texture.native().sampler };

    (texture_view, sampler)
}