use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ffi::{FfiIndexMap, FfiOption, FfiString, FfiVec};
use aphyn_math::Vec4;
use aphyn_render_core::{GpuBindingTy, GpuBufferMemberTy, GpuShader, GpuTextureSampleKind, GpuUniformStorageBuffer, RenderApi};
use aphyn_serialization::*;

use crate::{MaterialTemplate, MaterialTemplatePass, Shader, StandardTexture};

// todo: more input types (integers, vec2/vec3, matrices)
// todo: a convenient way to author colors in sRGB; Vec4 inputs are always uploaded as is (linear)
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

impl MaterialDataValue {
    pub fn is_same_kind(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    fn buffer_member_ty(&self) -> Option<GpuBufferMemberTy> {
        match self {
            Self::F32(_) => Some(GpuBufferMemberTy::Float),
            Self::Vec4(_) => Some(GpuBufferMemberTy::Float4),
            Self::Texture(_) => None,
        }
    }

    fn write_bytes(&self, out: &mut [u8]) {
        match self {
            Self::F32(value) => out[..4].copy_from_slice(&value.to_le_bytes()),
            Self::Vec4(value) => {
                for (i, component) in value.into_array().into_iter().enumerate() {
                    out[i * 4..i * 4 + 4].copy_from_slice(&component.to_le_bytes());
                }
            }
            Self::Texture(_) => {}
        }
    }

    fn byte_size(&self) -> usize {
        match self {
            Self::F32(_) => 4,
            Self::Vec4(_) => 16,
            Self::Texture(_) => 0,
        }
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
    ($variant: ident, $ty: ty, $name_ref: ident, $name_mut: ident) => {
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

    pub fn entry_mut(&mut self, name: &str) -> Option<&mut MaterialDataEntry> {
        self.0.get_mut(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &MaterialDataEntry)> {
        self.0.iter().map(|(name, entry)| (name.as_str(), entry))
    }

    pub fn len(&self) -> usize {
        self.0.len() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    // todo: use a more convenient/reliable change-detection approach
    pub fn carry_snapshots_from(&mut self, old: &MaterialData) {
        for (name, entry) in self.0.iter_mut() {
            if let Some(old_entry) = old.0.get(name.as_str()) {
                entry.snapshot = next_snapshot(old_entry.snapshot);
            }
        }
    }

    pub fn reconcile(&mut self, template: &MaterialTemplate) {
        let mut old = std::mem::take(&mut self.0);

        for (name, input) in template.inputs.iter() {
            let entry = match old.remove_swap(name.as_str()) {
                Some(entry) if entry.value.is_same_kind(&input.default) => entry,
                _ => MaterialDataEntry::new(input.default.clone()),
            };

            self.0.insert(name.clone(), entry);
        }
    }

    material_data_get_impl!(F32, f32, get_f32, get_f32_mut);
    material_data_get_impl!(Vec4, Vec4<f32>, get_vec4, get_vec4_mut);
    material_data_get_impl!(Texture, AssetHandle<StandardTexture>, get_texture, get_texture_mut);
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
pub struct MaterialUniform {
    group: usize,
    binding: usize,
    buffer: GpuUniformStorageBuffer,
    cache: FfiVec<u8>,
    is_dirty: bool,
}

impl MaterialUniform {
    pub fn group(&self) -> usize {
        self.group
    }

    pub fn binding(&self) -> usize {
        self.binding
    }

    pub fn buffer(&self) -> &GpuUniformStorageBuffer {
        &self.buffer
    }
}

#[repr(C)]
struct MaterialPassValue {
    input: FfiString,
    uniform: usize,
    offset: usize,
    uploaded_snapshot: u64,
}

#[repr(C)]
pub struct MaterialPassGpu {
    shader: GpuShader,
    uniforms: FfiVec<MaterialUniform>,
    values: FfiVec<MaterialPassValue>,
}

impl MaterialPassGpu {
    pub fn shader(&self) -> &GpuShader {
        &self.shader
    }

    pub fn uniforms(&self) -> &[MaterialUniform] {
        &self.uniforms
    }
}

#[repr(C)]
#[derive(Default)]
pub struct MaterialGpuData {
    is_built: bool,
    passes: FfiVec<FfiOption<MaterialPassGpu>>,
}

impl MaterialGpuData {
    pub fn is_built(&self) -> bool {
        self.is_built
    }

    pub fn pass(&self, index: usize) -> Option<&MaterialPassGpu> {
        self.passes.get(index as u64)?.as_ref()
    }

    pub fn pass_count(&self) -> usize {
        self.passes.len() as usize
    }

    pub(crate) fn build(
        core: &RenderApi,
        template: &MaterialTemplate,
        shaders: &AssetStorageResource<Shader>,
        data: &MaterialData,
    ) -> Self {
        let passes = template
            .passes
            .iter()
            .enumerate()
            .map(|(i, pass)| match build_pass(core, template, shaders, pass) {
                Ok(pass) => FfiOption::Some(pass),
                Err(error) => {
                    eprintln!("material template pass {i} is disabled: {error}");
                    FfiOption::None
                }
            })
            .collect();

        let mut gpu = Self { is_built: true, passes };
        gpu.upload_changes(core, data);

        gpu
    }

    pub(crate) fn upload_changes(&mut self, core: &RenderApi, data: &MaterialData) {
        for pass in self.passes.iter_mut() {
            let FfiOption::Some(pass) = pass else {
                continue;
            };

            for value in pass.values.iter_mut() {
                let Some(entry) = data.entry(value.input.as_str()) else {
                    continue;
                };

                if entry.snapshot == value.uploaded_snapshot {
                    continue;
                }

                let Some(uniform) = pass.uniforms.get_mut(value.uniform as u64) else {
                    continue;
                };
                let cache: &mut [u8] = &mut uniform.cache;
                let size = entry.value.byte_size();

                if let Some(bytes) = cache.get_mut(value.offset..value.offset + size) {
                    entry.value.write_bytes(bytes);
                    uniform.is_dirty = true;
                }

                value.uploaded_snapshot = entry.snapshot;
            }

            for uniform in pass.uniforms.iter_mut().filter(|uniform| uniform.is_dirty) {
                crate::write_buffer_bytes(core, &uniform.buffer, 0, &uniform.cache);
                uniform.is_dirty = false;
            }
        }
    }
}

impl std::fmt::Debug for MaterialGpuData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialGpuData")
            .field("is_built", &self.is_built)
            .field("passes", &self.passes.len())
            .finish()
    }
}

fn build_pass(
    core: &RenderApi,
    template: &MaterialTemplate,
    shaders: &AssetStorageResource<Shader>,
    pass: &MaterialTemplatePass,
) -> Result<MaterialPassGpu, String> {
    let shader = shaders.get(&pass.shader).ok_or("its shader is not loaded")?.shader();

    let mut uniforms = Vec::<MaterialUniform>::new();
    let mut values = Vec::new();

    for value in pass.values.iter() {
        let input = template
            .input(value.input.as_str())
            .ok_or_else(|| format!("unknown input '{}'", value.input))?;
        let expected_ty = input
            .default
            .buffer_member_ty()
            .ok_or_else(|| format!("texture input '{}' is mapped to a buffer member", value.input))?;

        if value.group == 0 {
            return Err(format!(
                "input '{}' is mapped to group 0, which is reserved for the engine",
                value.input
            ));
        }

        let entry = shader.bind_group_layout_entry(value.group, value.binding).ok_or_else(|| {
            format!(
                "the shader has no binding @group({}) @binding({})",
                value.group, value.binding
            )
        })?;

        if !matches!(
            entry.ty,
            GpuBindingTy::UniformBuffer | GpuBindingTy::StorageBuffer { read_only: true }
        ) {
            return Err(format!(
                "@group({}) @binding({}) is not a uniform or read-only storage buffer",
                value.group, value.binding
            ));
        }

        let member = entry.member(value.member.as_str()).ok_or_else(|| {
            format!(
                "@group({}) @binding({}) has no member '{}'",
                value.group, value.binding, value.member
            )
        })?;

        if member.ty != expected_ty {
            return Err(format!(
                "member '{}' is {:?}, but input '{}' needs {:?}",
                value.member, member.ty, value.input, expected_ty
            ));
        }

        let uniform = match uniforms
            .iter()
            .position(|uniform| uniform.group == value.group && uniform.binding == value.binding)
        {
            Some(uniform) => uniform,
            None => {
                uniforms.push(MaterialUniform {
                    group: value.group,
                    binding: value.binding,
                    buffer: core.create_uniform_storage_buffer(Some("material uniform"), entry.size),
                    cache: vec![0_u8; entry.size.next_multiple_of(4)].into(),
                    is_dirty: true,
                });
                uniforms.len() - 1
            }
        };

        values.push(MaterialPassValue {
            input: value.input.clone(),
            uniform,
            offset: member.offset,
            uploaded_snapshot: 0,
        });
    }

    for texture in pass.textures.iter() {
        let input = template
            .input(texture.input.as_str())
            .ok_or_else(|| format!("unknown input '{}'", texture.input))?;

        if !matches!(input.default, MaterialDataValue::Texture(_)) {
            return Err(format!(
                "input '{}' is mapped to a texture, but it is not a texture",
                texture.input
            ));
        }

        if texture.group == 0 {
            return Err(format!(
                "input '{}' is mapped to group 0, which is reserved for the engine",
                texture.input
            ));
        }

        let texture_entry = shader.bind_group_layout_entry(texture.group, texture.texture_binding);

        if !texture_entry.is_some_and(|entry| entry.ty == GpuBindingTy::Texture(GpuTextureSampleKind::Float)) {
            return Err(format!(
                "@group({}) @binding({}) is not a float texture",
                texture.group, texture.texture_binding
            ));
        }

        if let FfiOption::Some(sampler_binding) = texture.sampler_binding {
            let sampler_entry = shader.bind_group_layout_entry(texture.group, sampler_binding);

            if !sampler_entry.is_some_and(|entry| entry.ty == GpuBindingTy::Sampler) {
                return Err(format!(
                    "@group({}) @binding({}) is not a filtering sampler",
                    texture.group, sampler_binding
                ));
            }
        }
    }

    for (group, entries) in shader.bind_group_layouts().iter().enumerate().skip(1) {
        for entry in entries.iter() {
            let is_mapped = uniforms
                .iter()
                .any(|uniform| uniform.group() == group && uniform.binding() == entry.binding)
                || pass.textures.iter().any(|texture| {
                    texture.group == group
                        && (texture.texture_binding == entry.binding
                            || texture
                                .sampler_binding
                                .as_ref()
                                .is_some_and(|&binding| binding == entry.binding))
                });

            if !is_mapped {
                return Err(format!(
                    "@group({group}) @binding({}) is not mapped to any input",
                    entry.binding
                ));
            }
        }
    }

    Ok(MaterialPassGpu {
        shader: shader.clone(),
        uniforms: uniforms.into(),
        values: values.into(),
    })
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Material {
    pub template: AssetHandle<MaterialTemplate>,
    pub data: MaterialData,
    pub(crate) gpu: MaterialGpuData,
}

impl Material {
    pub fn new(template: AssetHandle<MaterialTemplate>, data: MaterialData) -> Self {
        Self {
            template,
            data,
            gpu: MaterialGpuData::default(),
        }
    }

    pub fn gpu(&self) -> &MaterialGpuData {
        &self.gpu
    }
}

impl Clone for Material {
    fn clone(&self) -> Self {
        Self::new(self.template.clone(), self.data.clone())
    }
}

impl<S: Copy + SerializerCtxState<AssetHandle<MaterialTemplate>> + SerializerCtxState<MaterialData>> Serializable<S> for Material {
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        ctx.serialize_map(path)
            .with_field("template", &self.template)
            .with_field("data", &self.data)
            .finish_as_map(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        ctx.deserialize_map(self, path, value)
            .with_field("template", &mut self.template)
            .with_field("data", &mut self.data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MaterialInput;

    fn registry() -> TransSerializerRegistry<'static> {
        let mut registry = TransSerializerRegistry::new();
        registry.register(StandardSerializer::<f32>::default());
        registry.register(StandardSerializer::<Vec4<f32>>::default());
        registry.register(StandardSerializer::<FfiString>::default());
        registry.register(StandardSerializer::<MaterialDataValue>::default());
        registry.register(StandardSerializer::<MaterialDataEntry>::default());
        registry.register(StandardSerializer::<MaterialData>::default());
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
    fn carry_snapshots_from_bumps_existing_keys_only() {
        let mut old = sample_data();
        *old.get_f32_mut("speed").unwrap() = 0.4;
        *old.get_f32_mut("speed").unwrap() = 0.5;

        let mut replacement = sample_data();
        replacement.insert("glow", MaterialDataValue::F32(1.0));
        replacement.carry_snapshots_from(&old);

        assert_eq!(replacement.entry("speed").unwrap().snapshot(), 4);
        assert_eq!(replacement.entry("color").unwrap().snapshot(), 2);
        assert_eq!(replacement.entry("glow").unwrap().snapshot(), 1);
        assert_eq!(replacement.get_f32("speed"), Some(&0.3));
    }

    #[test]
    fn next_snapshot_skips_zero() {
        assert_eq!(next_snapshot(u64::MAX), 1);
        assert_eq!(next_snapshot(1), 2);
    }

    #[test]
    fn reconcile_follows_template_inputs() {
        let mut template = MaterialTemplate::default();
        template
            .inputs
            .insert("color".into(), MaterialInput::new(MaterialDataValue::Vec4(Vec4::splat(1.0))));
        template
            .inputs
            .insert("speed".into(), MaterialInput::new(MaterialDataValue::F32(2.0)));
        template
            .inputs
            .insert("glow".into(), MaterialInput::new(MaterialDataValue::F32(0.5)));

        let mut data = MaterialData::new();
        data.insert("speed", MaterialDataValue::F32(0.3));
        data.insert("color", MaterialDataValue::F32(9.0));
        data.insert("unknown", MaterialDataValue::F32(1.0));
        *data.get_f32_mut("speed").unwrap() = 0.4;

        data.reconcile(&template);

        assert_eq!(entry_names(&data), ["color", "speed", "glow"]);
        assert_eq!(data.get_vec4("color"), Some(&Vec4::splat(1.0)));
        assert_eq!(data.get_f32("speed"), Some(&0.4));
        assert_eq!(data.entry("speed").unwrap().snapshot(), 2);
        assert_eq!(data.get_f32("glow"), Some(&0.5));
    }

    #[test]
    fn value_bytes_are_little_endian() {
        let mut bytes = [0_u8; 16];
        MaterialDataValue::Vec4(Vec4::new(1.0, 2.0, 3.0, 4.0)).write_bytes(&mut bytes);
        assert_eq!(&bytes[4..8], &2.0_f32.to_le_bytes());

        MaterialDataValue::F32(0.5).write_bytes(&mut bytes);
        assert_eq!(&bytes[..4], &0.5_f32.to_le_bytes());
        assert_eq!(&bytes[4..8], &2.0_f32.to_le_bytes());
    }
}
