use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ffi::{FfiIndexMap, FfiString};
use aphyn_math::{Mat4, Vec3, Vec4};
use aphyn_render_core::{GpuUniformStorageBuffer, RenderApi};
use aphyn_serialization::*;
use aphyn_utils::mem::{AllBitVariationsValid, AllBitsInit};

use crate::{StandardShader, StandardTexture};

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
            StandardLight::Spot {
                color,
                center,
                range,
                direction_dst,
                fov,
            } => Self {
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

#[repr(C, u8)]
#[derive(Clone, Debug, PartialEq, Serializable)]
pub enum MaterialDataValue {
    F32(f32),
    Vec4(Vec4<f32>),
    Texture(AssetHandle<StandardTexture>),
}

impl Default for MaterialDataValue {
    fn default() -> Self {
        Self::F32(0.0)
    }
}

fn next_snapshot(snapshot: u64) -> u64 {
    match snapshot.wrapping_add(1) {
        0 => 1,
        next => next,
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MaterialDataEntry {
    value: MaterialDataValue,
    snapshot: u64,
}

impl MaterialDataEntry {
    pub fn new(value: MaterialDataValue) -> Self {
        Self { value, snapshot: 1 }
    }

    pub fn get(&self) -> &MaterialDataValue {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut MaterialDataValue {
        self.snapshot = next_snapshot(self.snapshot);
        &mut self.value
    }

    pub fn snapshot(&self) -> u64 {
        self.snapshot
    }
}

impl Default for MaterialDataEntry {
    fn default() -> Self {
        Self::new(MaterialDataValue::default())
    }
}

impl<S: SerializerCtxState<MaterialDataValue>> Serializable<S> for MaterialDataEntry {
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        ctx.serialize(&self.value, path)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        ctx.deserialize(&mut self.value, path, serialized);
        self.snapshot = next_snapshot(self.snapshot);
    }
}

#[repr(transparent)]
#[derive(Clone, Debug, Default)]
pub struct MaterialData(FfiIndexMap<FfiString, MaterialDataEntry>);

macro_rules! material_data_get_impl {
    ($variant: ident, $ty: ty, $name_ref: ident, $name_mut: ident, $name_or: ident, $fallback: expr) => {
        pub fn $name_ref(&self, name: &str) -> Option<&$ty> {
            match &self.0.get(name)?.value {
                MaterialDataValue::$variant(value) => Some(value),
                _ => None,
            }
        }

        pub fn $name_mut(&mut self, name: &str) -> Option<&mut $ty> {
            match self.0.get_mut(name)?.get_mut() {
                MaterialDataValue::$variant(value) => Some(value),
                _ => None,
            }
        }

        pub fn $name_or(&self, name: &str, default: &MaterialData) -> $ty {
            self.$name_ref(name)
                .or_else(|| default.$name_ref(name))
                .cloned()
                .unwrap_or($fallback)
        }
    };
}

impl MaterialData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: &str, value: MaterialDataValue) {
        self.0.insert(name.into(), MaterialDataEntry::new(value));
    }

    pub fn entry(&self, name: &str) -> Option<&MaterialDataEntry> {
        self.0.get(name)
    }

    material_data_get_impl!(F32, f32, get_f32, get_f32_mut, get_f32_or, 0.0);
    material_data_get_impl!(Vec4, Vec4<f32>, get_vec4, get_vec4_mut, get_vec4_or, Vec4::splat(0.0));
    material_data_get_impl!(
        Texture,
        AssetHandle<StandardTexture>,
        get_texture,
        get_texture_mut,
        get_texture_or,
        AssetHandle::EMPTY
    );
}

impl<S: SerializerCtxState<FfiString> + SerializerCtxState<MaterialDataEntry>> Serializable<S> for MaterialData {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        self.0.serialize(ctx, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        let existing = self.0.len() as usize;
        self.0.deserialize(ctx, path, serialized);
        for entry in self.0.values_mut().skip(existing) {
            entry.snapshot = 1;
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, Serializable)]
pub struct Material {
    pub template: FfiString,
    pub data: MaterialData,
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
    uniform_buffer: GpuUniformStorageBuffer,
    is_dirty: bool,
}

impl StandardMaterial {
    pub(crate) fn new(core: &RenderApi, meta: StandardMaterialAssetMetadata) -> Self {
        let uniform_buffer =
            core.create_uniform_storage_buffer(Some("standard material uniform"), size_of::<StandardUniformMaterial>());
        crate::write_buffer_bytes(
            core,
            &uniform_buffer,
            0,
            aphyn_utils::mem::as_bytes(&StandardUniformMaterial::default()),
        );

        Self {
            meta,
            uniform_buffer,
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

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn uniform_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.uniform_buffer
    }

    pub fn update_gpu_data_if_dirty(&mut self, render_api: &crate::RenderApiResource) {
        if !self.is_dirty {
            return;
        }

        let uniform = StandardUniformMaterial {
            color: srgb_to_linear(self.meta.color),
            emission_color: srgb_to_linear(self.meta.emission_color),
            metallic: self.meta.metallic,
            roughness: self.meta.roughness,
            alpha_threshold: self.meta.alpha_threshold,
            _padding: Default::default(),
        };

        crate::write_buffer_bytes(
            render_api.core(),
            &self.uniform_buffer,
            0,
            aphyn_utils::mem::as_bytes(&uniform),
        );

        self.is_dirty = false;
    }

    pub fn resolve_textures<'a>(
        &self,
        textures: &'a AssetStorageResource<StandardTexture>,
        white: &'a StandardTexture,
        normal_default: &'a StandardTexture,
    ) -> [&'a StandardTexture; 5] {
        let resolve = |handle, fallback| textures.get(handle).unwrap_or(fallback);

        [
            resolve(&self.meta.color_tex, white),
            resolve(&self.meta.roughness_tex, white),
            resolve(&self.meta.metallic_tex, white),
            resolve(&self.meta.normal_tex, normal_default),
            resolve(&self.meta.emission_tex, white),
        ]
    }
}

fn srgb_to_linear(color: Vec4<f32>) -> Vec4<f32> {
    Vec4::new(color.x.powf(2.2), color.y.powf(2.2), color.z.powf(2.2), color.w)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> TransSerializerRegistry<'static> {
        let mut registry = TransSerializerRegistry::new();
        registry.register(StandardSerializer::<f32>::default());
        registry.register(StandardSerializer::<Vec4<f32>>::default());
        registry.register(StandardSerializer::<FfiString>::default());
        registry.register(StandardSerializer::<MaterialDataValue>::default());
        registry.register(StandardSerializer::<MaterialDataEntry>::default());
        registry.register(StandardSerializer::<MaterialData>::default());
        registry.register(StandardSerializer::<Material>::default());
        registry
    }

    fn sample_data() -> MaterialData {
        let mut data = MaterialData::new();
        data.insert("speed", MaterialDataValue::F32(0.3));
        data.insert("color", MaterialDataValue::Vec4(Vec4::new(1.0, 0.0, 0.0, 1.0)));
        data
    }

    fn entry_names(data: &MaterialData) -> Vec<String> {
        data.0.keys().map(|k| k.to_string()).collect()
    }

    #[test]
    fn roundtrip_keeps_values() {
        let mut registry = registry();
        let mut on_err = |err: SerializationError| panic!("{err}");
        let data = sample_data();

        let serialized = registry.serialize(&data, "", &mut on_err);
        let mut restored = MaterialData::default();
        registry.deserialize(&mut restored, "", &serialized, &mut on_err);

        assert_eq!(entry_names(&restored), ["speed", "color"]);
        assert_eq!(restored.get_f32("speed"), Some(&0.3));
        assert_eq!(restored.get_vec4("color"), Some(&Vec4::new(1.0, 0.0, 0.0, 1.0)));

        let json = serialized.to_json();
        let value_json = registry.serialize(&MaterialDataValue::F32(0.3), "", &mut on_err).to_json();
        assert_eq!(json["speed"], value_json);
        assert!(json["speed"].get("snapshot").is_none());
    }

    #[test]
    fn get_mut_bumps_snapshot() {
        let mut data = sample_data();
        assert_eq!(data.entry("speed").unwrap().snapshot(), 1);

        *data.get_f32_mut("speed").unwrap() = 0.5;
        assert_eq!(data.entry("speed").unwrap().snapshot(), 2);

        data.get_f32("speed");
        assert_eq!(data.entry("speed").unwrap().snapshot(), 2);
        assert_eq!(data.entry("color").unwrap().snapshot(), 1);
    }

    #[test]
    fn deserialize_bumps_snapshot() {
        let mut registry = registry();
        let mut on_err = |err: SerializationError| panic!("{err}");
        let mut data = sample_data();

        let serialized = registry.serialize(&MaterialDataValue::F32(0.9), "", &mut on_err);
        registry.deserialize(&mut data, "speed", &serialized, &mut on_err);

        assert_eq!(data.get_f32("speed"), Some(&0.9));
        assert_eq!(data.entry("speed").unwrap().snapshot(), 2);
        assert_eq!(data.entry("color").unwrap().snapshot(), 1);
    }

    #[test]
    fn deserialize_new_entries_start_at_one() {
        let mut registry = registry();
        let mut on_err = |err: SerializationError| panic!("{err}");

        let serialized = registry.serialize(&sample_data(), "", &mut on_err);
        let mut restored = MaterialData::default();
        registry.deserialize(&mut restored, "", &serialized, &mut on_err);

        assert_eq!(restored.entry("speed").unwrap().snapshot(), 1);
        assert_eq!(restored.entry("color").unwrap().snapshot(), 1);
    }

    #[test]
    fn next_snapshot_skips_zero() {
        assert_eq!(next_snapshot(u64::MAX), 1);
        assert_eq!(next_snapshot(1), 2);
    }

    #[test]
    fn typed_get_or_default_falls_back() {
        let mut data = MaterialData::new();
        data.insert("color", MaterialDataValue::F32(2.0));

        let mut defaults = MaterialData::new();
        defaults.insert("speed", MaterialDataValue::F32(0.3));
        defaults.insert("color", MaterialDataValue::Vec4(Vec4::new(0.0, 0.0, 1.0, 1.0)));

        assert_eq!(data.get_f32_or("speed", &defaults), 0.3);
        assert_eq!(data.get_vec4_or("color", &defaults), Vec4::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(data.get_f32_or("missing", &defaults), 0.0);
        assert_eq!(data.get_vec4_or("missing", &defaults), Vec4::splat(0.0));
        assert_eq!(data.get_texture_or("missing", &defaults), AssetHandle::EMPTY);
    }

    #[test]
    fn material_roundtrip() {
        let mut registry = registry();
        let mut on_err = |err: SerializationError| panic!("{err}");
        let material = Material {
            template: "water".into(),
            data: sample_data(),
        };

        let serialized = registry.serialize(&material, "", &mut on_err);
        let json = serialized.to_json();
        assert!(json.get("template").is_some());
        assert!(json.get("data").is_some());

        let mut restored = Material::default();
        registry.deserialize(&mut restored, "", &serialized, &mut on_err);

        assert_eq!(restored.template.as_str(), "water");
        assert_eq!(entry_names(&restored.data), ["speed", "color"]);
        assert_eq!(restored.data.get_f32("speed"), Some(&0.3));
    }
}
