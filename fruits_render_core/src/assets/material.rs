use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_ffi::FfiDroppable;
use fruits_math::{Mat4, Vec3, Vec4};
use fruits_serialization::*;
use fruits_utils::mem::{AllBitVariationsValid, AllBitsInit};
use wgpu::{BindGroup, Buffer, BufferUsages, Sampler, TextureView, util::{BufferInitDescriptor, DeviceExt}};

use crate::{CreateBindGroupEntry, RenderState, StandardShader, StandardTexture};

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
    pub world_to_clip: Mat4<f32>,
    pub window_to_clip: Mat4<f32>,
    pub camera_position_world: Vec3<f32>,
    pub lights_count: u32,
}

unsafe impl AllBitVariationsValid for StandardUniformGlobal {}
unsafe impl AllBitsInit for StandardUniformGlobal {}

impl Default for StandardUniformGlobal {
    fn default() -> Self {
        Self {
            world_to_clip: Mat4::IDENTITY,
            window_to_clip: Mat4::IDENTITY,
            camera_position_world: Vec3::splat(0.0),
            lights_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StandardUniformMaterial {
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
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serializable)]
pub enum StandardEngineShader {
    #[default]
    OpaqueUnlit,
    OpaqueLit,
    TransparentUnlit,
    TransparentLit,
    UiOpaqueUnlit,
    UiTransparentUnlit,
}

impl From<StandardEngineShader> for StandardMaterialShader {
    fn from(value: StandardEngineShader) -> Self {
        Self::Engine(value)
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, PartialOrd, Serializable)]
pub enum StandardMaterialShader {
    Engine(StandardEngineShader),
    Custom(AssetHandle<StandardShader>),
}

impl Default for StandardMaterialShader {
    fn default() -> Self {
        Self::Engine(StandardEngineShader::default())
    }
}

impl StandardMaterialShader {
    pub fn is_transparent(&self) -> bool {
        match self {
            Self::Engine(
                StandardEngineShader::TransparentUnlit
                | StandardEngineShader::TransparentLit
                | StandardEngineShader::UiTransparentUnlit,
            ) => true,
            Self::Engine(
                StandardEngineShader::OpaqueUnlit | StandardEngineShader::OpaqueLit | StandardEngineShader::UiOpaqueUnlit,
            ) => false,
            // todo: transparent custom shaders
            Self::Custom(_) => false,
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, PartialOrd, Serializable)]
pub struct StandardMaterialAssetMetadata {
    pub shader: StandardMaterialShader,
    pub color: Vec4<f32>,
    pub emission_color: Vec4<f32>,
    pub metallic: f32,
    pub roughness: f32,
    pub alpha_threshold: f32,
    pub color_tex: AssetHandle<StandardTexture>,
    pub roughness_tex: AssetHandle<StandardTexture>,
    pub metallic_tex: AssetHandle<StandardTexture>,
    pub normal_tex: AssetHandle<StandardTexture>,
    pub emission_tex: AssetHandle<StandardTexture>,
}

impl Default for StandardMaterialAssetMetadata {
    fn default() -> Self {
        Self {
            shader: StandardMaterialShader::default(),
            color: Vec4::splat(1.0),
            emission_color: Vec4::splat(0.0),
            metallic: 0.0,
            roughness: 0.5,
            alpha_threshold: 0.5,
            color_tex: AssetHandle::EMPTY,
            roughness_tex: AssetHandle::EMPTY,
            metallic_tex: AssetHandle::EMPTY,
            normal_tex: AssetHandle::EMPTY,
            emission_tex: AssetHandle::EMPTY,
        }
    }
}

#[repr(C)]
pub struct StandardMaterial {
    meta: StandardMaterialAssetMetadata,
    native: FfiDroppable,
    texture_ids: [u64; 5],
    is_dirty: bool,
}

impl StandardMaterial {
    pub(crate) fn new(render_state: &RenderState, meta: StandardMaterialAssetMetadata) -> Self {
        let buffer_uniform = render_state.device().create_buffer_init(&BufferInitDescriptor {
            label: Some("Standard Material Uniform Buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            contents: fruits_utils::mem::as_bytes(&[StandardUniformMaterial::default()]),
        });

        let render_assets = render_state.render_assets();
        let white = &render_assets.texture_white;
        let textures = [white, white, white, &render_assets.texture_normal_default, white];

        let bind_group = create_material_bind_group(render_state, &buffer_uniform, textures);

        Self {
            native: FfiDroppable::new(StandardMaterialNative {
                buffer_uniform,
                bind_group,
            }),
            meta,
            texture_ids: [0; 5],
            is_dirty: true,
        }
    }

    pub fn meta(&self) -> &StandardMaterialAssetMetadata {
        &self.meta
    }

    pub fn meta_mut(&mut self) -> &mut StandardMaterialAssetMetadata {
        self.is_dirty = true;
        &mut self.meta
    }

    pub fn update_gpu_data_if_dirty(&mut self, render_state: &RenderState, textures: &AssetStorageResource<StandardTexture>) {
        let render_assets = render_state.render_assets();
        let white = &render_assets.texture_white;
        let resolve = |handle, fallback| textures.get(handle).unwrap_or(fallback);

        let resolved = [
            resolve(&self.meta.color_tex, white),
            resolve(&self.meta.roughness_tex, white),
            resolve(&self.meta.metallic_tex, white),
            resolve(&self.meta.normal_tex, &render_assets.texture_normal_default),
            resolve(&self.meta.emission_tex, white),
        ];

        let texture_ids = resolved.map(|t| t.id());

        if texture_ids != self.texture_ids {
            let native = unsafe { self.native_mut() };
            native.bind_group = create_material_bind_group(render_state, &native.buffer_uniform, resolved);
            self.texture_ids = texture_ids;
        }

        if self.is_dirty {
            let uniform = StandardUniformMaterial {
                color: srgb_to_linear(self.meta.color),
                emission_color: srgb_to_linear(self.meta.emission_color),
                metallic: self.meta.metallic,
                roughness: self.meta.roughness,
                alpha_threshold: self.meta.alpha_threshold,
                _padding: Default::default(),
            };

            let native = unsafe { self.native() };
            render_state.queue().write_buffer(&native.buffer_uniform, 0, fruits_utils::mem::as_bytes(&[uniform]));

            self.is_dirty = false;
        }
    }

    pub unsafe fn native(&self) -> &StandardMaterialNative {
        unsafe { &*(self.native.get() as *const StandardMaterialNative) }
    }

    unsafe fn native_mut(&mut self) -> &mut StandardMaterialNative {
        unsafe { &mut *(self.native.get() as *mut StandardMaterialNative) }
    }
}

unsafe impl Send for StandardMaterial where StandardMaterialNative: Send {}
unsafe impl Sync for StandardMaterial where StandardMaterialNative: Sync {}

fn create_material_bind_group(render_state: &RenderState, buffer_uniform: &Buffer, textures: [&StandardTexture; 5]) -> BindGroup {
    let [color, roughness, metallic, normal, emission] = textures.map(|t| unsafe { texture_to_view_and_sampler(t) });

    crate::utils::create_bind_group(
        render_state.device(),
        Some("Standard Material Bind Group"),
        &render_state.render_data().bind_group_layout_material,
        &[
            CreateBindGroupEntry::Buffer(buffer_uniform),
            CreateBindGroupEntry::Texture(&color.0),
            CreateBindGroupEntry::Sampler(color.1),
            CreateBindGroupEntry::Texture(&roughness.0),
            CreateBindGroupEntry::Sampler(roughness.1),
            CreateBindGroupEntry::Texture(&metallic.0),
            CreateBindGroupEntry::Sampler(metallic.1),
            CreateBindGroupEntry::Texture(&normal.0),
            CreateBindGroupEntry::Sampler(normal.1),
            CreateBindGroupEntry::Texture(&emission.0),
            CreateBindGroupEntry::Sampler(emission.1),
        ],
    )
}

unsafe fn texture_to_view_and_sampler(texture: &StandardTexture) -> (TextureView, &Sampler) {
    let texture_view = unsafe { texture.native().texture.create_view(&Default::default()) };
    let sampler = unsafe { &texture.native().sampler };

    (texture_view, sampler)
}

fn srgb_to_linear(color: Vec4<f32>) -> Vec4<f32> {
    Vec4::new(color.x.powf(2.2), color.y.powf(2.2), color.z.powf(2.2), color.w)
}