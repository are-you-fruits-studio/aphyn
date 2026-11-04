// todo: update to the latest wgpu
// todo: expose labels in the APIs of all GPU resources
// todo: use Results instead of Option / bool

use std::{
    borrow::Borrow,
    ffi::c_void,
    hash::Hash,
    num::NonZeroU64,
    ops::Range,
    sync::{Arc, Mutex, Weak},
};

use aphyn_ffi::{FfiDroppable, FfiFnMutMut, FfiOption, FfiSliceMut, FfiSliceRef, FfiStaticRef, FfiStrSliceRef, FfiTypedDroppable};
use aphyn_utils::index_version_collection::VersionIndex;
use wgpu::{
    LoadOp,
    rwh::{HasDisplayHandle, HasWindowHandle},
};

use crate::{BufferResource, RenderApiResources, RenderApiState, SamplerResource, ShaderResource, TextureResource, buffers::{
    GpuIndexBuffer, GpuIndexFormat, GpuUniformStorageBuffer, GpuVertexBuffer, GpuVertexBufferLayout,
    GpuVertexBufferLayoutEntryTy,
}};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TextureSampleKind {
    Float,
    Sint,
    Uint,
    Depth,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BindGroupBindingTy {
    Texture(TextureSampleKind),
    Sampler,
    ComparisonSampler,
    UniformBuffer,
    StorageBuffer { read_only: bool },
    // todo: storage textures
}

impl BindGroupBindingTy {
    pub(crate) fn to_wgpu(self) -> wgpu::BindingType {
        match self {
            Self::Texture(kind) => wgpu::BindingType::Texture {
                sample_type: match kind {
                    TextureSampleKind::Float => wgpu::TextureSampleType::Float { filterable: true },
                    TextureSampleKind::Sint => wgpu::TextureSampleType::Sint,
                    TextureSampleKind::Uint => wgpu::TextureSampleType::Uint,
                    TextureSampleKind::Depth => wgpu::TextureSampleType::Depth,
                },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            Self::Sampler => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            Self::ComparisonSampler => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            Self::UniformBuffer => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            Self::StorageBuffer { read_only } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        }
    }

    pub(crate) fn visibility(self) -> wgpu::ShaderStages {
        match self {
            // writable storage in the vertex stage needs `Features::VERTEX_WRITABLE_STORAGE`
            Self::StorageBuffer { read_only: false } => wgpu::ShaderStages::FRAGMENT,
            _ => wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }

    fn accepts(self, binding: BindGroupBinding, resources: &RenderApiResources) -> bool {
        match (self, binding) {
            (Self::Texture(kind), BindGroupBinding::Texture(texture, mip_level)) => match resources.textures.get(texture) {
                Some(texture) => {
                    let is_kind_matching = match texture.format() {
                        GpuTextureFormat::Depth32Float => kind == TextureSampleKind::Depth,
                        _ => kind == TextureSampleKind::Float,
                    };

                    is_kind_matching && texture.view(mip_level).is_some()
                }
                None => false,
            },
            (Self::Sampler | Self::ComparisonSampler, BindGroupBinding::Sampler(sampler)) => match resources.samplers.get(sampler) {
                Some(sampler) => sampler.comparison() == (self == Self::ComparisonSampler),
                None => false,
            },
            (Self::UniformBuffer | Self::StorageBuffer { .. }, BindGroupBinding::Buffer(buffer)) => {
                resources.buffers.contains_index(buffer)
            }
            _ => false,
        }
    }
}

pub(crate) type BindGroupLayoutEntry = (usize, BindGroupBindingTy);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BindGroupLayoutEntries {
    entries: Vec<BindGroupLayoutEntry>,
}

impl Borrow<[BindGroupLayoutEntry]> for BindGroupLayoutEntries {
    fn borrow(&self) -> &[BindGroupLayoutEntry] {
        &self.entries
    }
}

impl BindGroupLayoutEntries {
    pub fn new(entries: Vec<BindGroupLayoutEntry>) -> Self {
        Self {
            entries,
        }
    }

    pub fn entries(&self) -> &Vec<BindGroupLayoutEntry> {
        &self.entries
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BindGroupBinding {
    Texture(VersionIndex, Option<usize>),
    Sampler(VersionIndex),
    Buffer(VersionIndex),
}

impl BindGroupBinding {
    pub(crate) fn refers_to(self, key: ResourceKey) -> bool {
        match (self, key) {
            (Self::Texture(id, _), ResourceKey::Texture(key_id))
            | (Self::Sampler(id), ResourceKey::Sampler(key_id))
            | (Self::Buffer(id), ResourceKey::Buffer(key_id)) => id == key_id,
            _ => false,
        }
    }

    pub(crate) fn to_wgpu<'a>(self, resources: &'a RenderApiResources) -> Option<wgpu::BindingResource<'a>> {
        Some(match self {
            Self::Texture(texture, mip_level) => {
                wgpu::BindingResource::TextureView(resources.textures.get(texture)?.view(mip_level)?)
            }
            Self::Sampler(sampler) => wgpu::BindingResource::Sampler(resources.samplers.get(sampler)?.sampler()),
            Self::Buffer(buffer) => resources.buffers.get(buffer)?.buffer.as_entire_binding(),
        })
    }
}

pub(crate) type BindGroupEntry = (usize, BindGroupBindingTy, BindGroupBinding);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BindGroupEntries {
    pub(crate) entries: Vec<BindGroupEntry>,
}

impl Borrow<[BindGroupEntry]> for BindGroupEntries {
    fn borrow(&self) -> &[BindGroupEntry] {
        &self.entries
    }
}

pub(crate) type VertexAttributeEntry = (usize, GpuVertexBufferLayoutEntryTy);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct PipelineKey {
    pub(crate) shader: VersionIndex,
    pub(crate) target: Option<wgpu::TextureFormat>,
    pub(crate) depth: Option<GpuTextureFormat>,
    pub(crate) vertex_buffers: Vec<GpuVertexBufferLayout>,
}

struct RenderSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: [usize; 2],
}

struct SurfaceDrawTarget<'a> {
    view: &'a wgpu::TextureView,
    format: wgpu::TextureFormat,
    ops: GpuPassOps<[f32; 4]>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ResourceKey {
    Texture(VersionIndex),
    Sampler(VersionIndex),
    Buffer(VersionIndex),
    Shader(VersionIndex),
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GpuResourceId(pub(crate) ResourceKey);

pub(crate) struct ResourceGuard {
    state: Weak<Mutex<RenderApiState>>,
    key: ResourceKey,
}

impl Drop for ResourceGuard {
    fn drop(&mut self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };

        // a poisoned lock means this drop may be unwinding, and panicking again would abort
        if let Ok(mut state) = state.lock() {
            state.release(self.key);
        }
    }
}

fn parse_shader(wgsl: &str) -> Option<naga::Module> {
    match naga::front::wgsl::parse_str(wgsl) {
        Ok(module) => Some(module),
        Err(error) => {
            println!("failed to parse shader: {}", error.emit_to_string(wgsl));
            None
        }
    }
}

fn reflect_vertex_attributes(module: &naga::Module) -> Option<Vec<VertexAttributeEntry>> {
    let Some(entry_point) = module
        .entry_points
        .iter()
        .find(|entry_point| entry_point.stage == naga::ShaderStage::Vertex && entry_point.name == "vs_main")
    else {
        println!("shader has no vs_main vertex entry point");
        return None;
    };

    let mut attributes = Vec::new();

    for argument in &entry_point.function.arguments {
        match (&argument.binding, &module.types[argument.ty].inner) {
            (Some(binding), _) => push_vertex_attribute(module, &mut attributes, binding, argument.ty)?,
            (None, naga::TypeInner::Struct { members, .. }) => {
                for member in members {
                    if let Some(binding) = &member.binding {
                        push_vertex_attribute(module, &mut attributes, binding, member.ty)?;
                    }
                }
            }
            (None, _) => {}
        }
    }

    attributes.sort();

    Some(attributes)
}

fn push_vertex_attribute(
    module: &naga::Module,
    attributes: &mut Vec<VertexAttributeEntry>,
    binding: &naga::Binding,
    ty: naga::Handle<naga::Type>,
) -> Option<()> {
    let naga::Binding::Location { location, .. } = binding else {
        return Some(());
    };

    let Some(ty) = vertex_attribute_ty(&module.types[ty].inner) else {
        println!("unsupported vertex attribute type @location({location})");
        return None;
    };

    attributes.push((*location as usize, ty));

    Some(())
}

fn vertex_attribute_ty(ty: &naga::TypeInner) -> Option<GpuVertexBufferLayoutEntryTy> {
    let (scalar, size) = match ty {
        naga::TypeInner::Scalar(scalar) => (*scalar, 1),
        naga::TypeInner::Vector { size, scalar } => (*scalar, *size as usize),
        _ => return None,
    };

    if scalar.width != 4 {
        return None;
    }

    Some(match (scalar.kind, size) {
        (naga::ScalarKind::Float, 1) => GpuVertexBufferLayoutEntryTy::Float,
        (naga::ScalarKind::Float, 2) => GpuVertexBufferLayoutEntryTy::Float2,
        (naga::ScalarKind::Float, 3) => GpuVertexBufferLayoutEntryTy::Float3,
        (naga::ScalarKind::Float, 4) => GpuVertexBufferLayoutEntryTy::Float4,
        (naga::ScalarKind::Uint, 1) => GpuVertexBufferLayoutEntryTy::UInt,
        (naga::ScalarKind::Uint, 2) => GpuVertexBufferLayoutEntryTy::UInt2,
        (naga::ScalarKind::Uint, 3) => GpuVertexBufferLayoutEntryTy::UInt3,
        (naga::ScalarKind::Uint, 4) => GpuVertexBufferLayoutEntryTy::UInt4,
        (naga::ScalarKind::Sint, 1) => GpuVertexBufferLayoutEntryTy::Int,
        (naga::ScalarKind::Sint, 2) => GpuVertexBufferLayoutEntryTy::Int2,
        (naga::ScalarKind::Sint, 3) => GpuVertexBufferLayoutEntryTy::Int3,
        (naga::ScalarKind::Sint, 4) => GpuVertexBufferLayoutEntryTy::Int4,
        _ => return None,
    })
}

fn check_stage_interface(module: &naga::Module) -> Option<()> {
    let find_entry_point = |stage, name| {
        module
            .entry_points
            .iter()
            .find(|entry_point| entry_point.stage == stage && entry_point.name == name)
    };

    let Some(vertex) = find_entry_point(naga::ShaderStage::Vertex, "vs_main") else {
        println!("shader has no vs_main vertex entry point");
        return None;
    };
    let Some(fragment) = find_entry_point(naga::ShaderStage::Fragment, "fs_main") else {
        println!("shader has no fs_main fragment entry point");
        return None;
    };

    let mut outputs = Vec::new();

    if let Some(result) = &vertex.function.result {
        push_stage_locations(module, &mut outputs, result.binding.as_ref(), result.ty);
    }

    let mut inputs = Vec::new();

    for argument in &fragment.function.arguments {
        push_stage_locations(module, &mut inputs, argument.binding.as_ref(), argument.ty);
    }

    for (location, ty) in inputs {
        match outputs.iter().find(|&&(output_location, _)| output_location == location) {
            None => {
                println!("fs_main reads @location({location}), which vs_main does not output");
                return None;
            }
            Some(&(_, output_ty)) if output_ty != ty => {
                println!("fs_main reads @location({location}) with a different type than vs_main outputs");
                return None;
            }
            Some(_) => {}
        }
    }

    Some(())
}

fn push_stage_locations<'a>(
    module: &'a naga::Module,
    locations: &mut Vec<(u32, &'a naga::TypeInner)>,
    binding: Option<&naga::Binding>,
    ty: naga::Handle<naga::Type>,
) {
    match (binding, &module.types[ty].inner) {
        (Some(naga::Binding::Location { location, .. }), inner) => locations.push((*location, inner)),
        (None, naga::TypeInner::Struct { members, .. }) => {
            for member in members {
                if let Some(naga::Binding::Location { location, .. }) = &member.binding {
                    locations.push((*location, &module.types[member.ty].inner));
                }
            }
        }
        _ => {}
    }
}

fn vertex_layouts_satisfy_attributes(attributes: &[VertexAttributeEntry], layouts: &[&GpuVertexBufferLayout]) -> bool {
    let entries = layouts.iter().flat_map(|layout| layout.entries()).collect::<Vec<_>>();

    let has_duplicates = entries
        .iter()
        .enumerate()
        .any(|(i, entry)| entries[..i].iter().any(|other| other.location == entry.location));

    !has_duplicates
        && attributes
            .iter()
            .all(|&(location, ty)| entries.iter().any(|entry| entry.location == location && entry.ty == ty))
}

fn reflect_bind_group_layouts(module: &naga::Module) -> Option<Vec<Vec<BindGroupLayoutEntry>>> {
    let mut groups = Vec::<Vec<BindGroupLayoutEntry>>::new();

    for (_, global) in module.global_variables.iter() {
        let Some(binding) = &global.binding else {
            continue;
        };

        let ty = match global.space {
            naga::AddressSpace::Uniform => BindGroupBindingTy::UniformBuffer,
            naga::AddressSpace::Storage { access } => BindGroupBindingTy::StorageBuffer {
                read_only: !access.contains(naga::StorageAccess::STORE),
            },
            naga::AddressSpace::Handle => match &module.types[global.ty].inner {
                naga::TypeInner::Sampler { comparison: false } => BindGroupBindingTy::Sampler,
                naga::TypeInner::Sampler { comparison: true } => BindGroupBindingTy::ComparisonSampler,
                naga::TypeInner::Image {
                    dim: naga::ImageDimension::D2,
                    arrayed: false,
                    class,
                } => match class {
                    naga::ImageClass::Sampled { kind, multi: false } => BindGroupBindingTy::Texture(match kind {
                        naga::ScalarKind::Float => TextureSampleKind::Float,
                        naga::ScalarKind::Sint => TextureSampleKind::Sint,
                        naga::ScalarKind::Uint => TextureSampleKind::Uint,
                        _ => return unsupported_binding(binding),
                    }),
                    naga::ImageClass::Depth { multi: false } => BindGroupBindingTy::Texture(TextureSampleKind::Depth),
                    // todo: storage textures
                    _ => return unsupported_binding(binding),
                },
                _ => return unsupported_binding(binding),
            },
            _ => return unsupported_binding(binding),
        };

        let group = binding.group as usize;

        if groups.len() <= group {
            groups.resize_with(group + 1, Vec::new);
        }

        groups[group].push((binding.binding as usize, ty));
    }

    for entries in &mut groups {
        entries.sort();
    }

    Some(groups)
}

fn unsupported_binding<T>(binding: &naga::ResourceBinding) -> Option<T> {
    println!(
        "unsupported shader binding @group({}) @binding({})",
        binding.group, binding.binding
    );
    None
}

fn create_instance() -> wgpu::Instance {
    wgpu::Instance::new(&wgpu::InstanceDescriptor {
        // todo: use primary, not only dx12
        backends: wgpu::Backends::DX12,
        ..Default::default()
    })
}

fn request_adapter(instance: &wgpu::Instance, surface: Option<&wgpu::Surface>) -> Option<wgpu::Adapter> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: surface,
        force_fallback_adapter: false,
    }));

    match adapter {
        Ok(adapter) => Some(adapter),
        Err(err) => {
            println!("failed to request adapter: {err}");
            None
        }
    }
}

fn request_device(adapter: &wgpu::Adapter) -> Option<(wgpu::Device, wgpu::Queue)> {
    let device = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        label: None,
        memory_hints: wgpu::MemoryHints::Performance,
        ..Default::default()
    }));

    match device {
        Ok(device) => Some(device),
        Err(err) => {
            println!("failed to request device: {err}");
            None
        }
    }
}

struct RenderApiInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    // lock order: surface, then state
    surface: Option<Mutex<RenderSurface>>,
    // error scopes are one stack per device, so a scope must only be open while this is locked
    state: Arc<Mutex<RenderApiState>>,
}

impl RenderApiInner {
    fn new(window: impl HasWindowHandle + HasDisplayHandle + Send + Sync + 'static, size: [usize; 2]) -> Option<Self> {
        let size = size.map(|x| x.max(1));

        let instance = create_instance();

        let surface = match instance.create_surface(window) {
            Ok(surface) => surface,
            Err(err) => {
                println!("failed to create surface: {err}");
                return None;
            }
        };

        let adapter = request_adapter(&instance, Some(&surface))?;
        let (device, queue) = request_device(&adapter)?;

        let capabilities = surface.get_capabilities(&adapter);

        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .or(capabilities.formats.first().copied())?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size[0] as u32,
            height: size[1] as u32,
            present_mode: *capabilities.present_modes.first()?,
            alpha_mode: *capabilities.alpha_modes.first()?,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Some(Self {
            device,
            queue,
            surface: Some(Mutex::new(RenderSurface { surface, config, size })),
            state: Default::default(),
        })
    }

    #[cfg(test)]
    fn new_headless() -> Option<Self> {
        let adapter = request_adapter(&create_instance(), None)?;
        let (device, queue) = request_device(&adapter)?;

        Some(Self {
            device,
            queue,
            surface: None,
            state: Default::default(),
        })
    }

    fn surface_size(&self) -> [usize; 2] {
        self.surface.as_ref().map_or([0, 0], |surface| surface.lock().unwrap().size)
    }

    fn resize_surface(&self, size: [usize; 2]) {
        if size[0] == 0 || size[1] == 0 {
            return;
        }

        let Some(surface) = &self.surface else {
            return;
        };

        let mut surface = surface.lock().unwrap();

        surface.size = size;
        surface.config.width = size[0] as u32;
        surface.config.height = size[1] as u32;

        surface.surface.configure(&self.device, &surface.config);
    }

    fn create_guard(&self, key: ResourceKey) -> FfiTypedDroppable<ResourceGuard> {
        FfiTypedDroppable::new(ResourceGuard {
            state: Arc::downgrade(&self.state),
            key,
        })
    }

    fn create_vertex_buffer(&self, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer {
        let id = self.create_buffer_resource("aphyn vertex buffer", len * layout.total_size(), wgpu::BufferUsages::VERTEX);

        GpuVertexBuffer {
            id,
            layout,
            len,
            _guard: self.create_guard(ResourceKey::Buffer(id)),
        }
    }

    fn create_uniform_storage_buffer(&self, size: usize) -> GpuUniformStorageBuffer {
        let alignment = wgpu::COPY_BUFFER_ALIGNMENT as usize;

        // a binding cannot be empty, so the GPU buffer gets at least one aligned word
        let id = self.create_buffer_resource(
            "aphyn uniform storage buffer",
            size.next_multiple_of(alignment).max(alignment),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::STORAGE,
        );

        GpuUniformStorageBuffer {
            id,
            size,
            _guard: self.create_guard(ResourceKey::Buffer(id)),
        }
    }

    fn create_index_buffer(&self, len: usize, format: GpuIndexFormat) -> GpuIndexBuffer {
        let id = self.create_buffer_resource("aphyn index buffer", len * format.gpu_size(), wgpu::BufferUsages::INDEX);

        GpuIndexBuffer {
            id,
            format,
            len,
            _guard: self.create_guard(ResourceKey::Buffer(id)),
        }
    }

    fn create_buffer_resource(&self, label: &str, size: usize, usage: wgpu::BufferUsages) -> VersionIndex {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size as u64,
            usage: usage | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        self.state.lock().unwrap().resources.buffers.insert(BufferResource { buffer })
    }

    fn create_texture(&self, mut size: [usize; 2], format: GpuTextureFormat, mip_level_count: usize) -> GpuTexture {
        size = size.map(|x| x.max(1));
        let mip_level_count = mip_level_count.max(1);

        let wgpu_format = format.to_wgpu();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aphyn texture"),
            size: wgpu::Extent3d {
                width: size[0] as u32,
                height: size[1] as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_level_count as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_format,
            usage: format.usages(),
            view_formats: &[wgpu_format],
        });

        let texture_view = texture.create_view(&Default::default());
        let mip_views = (0..mip_level_count)
            .map(|mip_level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: mip_level as u32,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        let texture_resource = TextureResource::new(
            texture,
            texture_view,
            mip_views,
            format,
            size,
            mip_level_count,
        );

        let id = self.state.lock().unwrap().resources.textures.insert(texture_resource);

        GpuTexture {
            id,
            format,
            size,
            mip_level_count,
            _guard: self.create_guard(ResourceKey::Texture(id)),
        }
    }

    fn create_sampler(&self, filter: GpuFilterMode, compare: Option<GpuCompareFunction>) -> GpuSampler {
        let wgpu_filter = filter.to_wgpu();

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aphyn sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu_filter,
            min_filter: wgpu_filter,
            mipmap_filter: wgpu_filter,
            compare: compare.map(GpuCompareFunction::to_wgpu),
            ..Default::default()
        });

        let id = self.state.lock().unwrap().resources.samplers.insert(SamplerResource::new(
            sampler,
            compare.is_some(),
        ));

        GpuSampler {
            id,
            _guard: self.create_guard(ResourceKey::Sampler(id)),
        }
    }

    fn create_shader(&self, wgsl: &str, desc: GpuShaderDesc) -> Option<GpuShader> {
        let parsed_module = parse_shader(wgsl)?;
        let bind_group_layouts = reflect_bind_group_layouts(&parsed_module)?;
        let vertex_attributes = reflect_vertex_attributes(&parsed_module)?;
        check_stage_interface(&parsed_module)?;

        let mut state = self.state.lock().unwrap();

        self.device.push_error_scope(wgpu::ErrorFilter::Validation);

        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aphyn shader"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });

        if let Some(err) = pollster::block_on(self.device.pop_error_scope()) {
            println!("failed to create shader: {err}");
            return None;
        }

        let id = state.resources.shaders.insert(ShaderResource::new(
            module,
            desc,
            bind_group_layouts,
            vertex_attributes,
        ));

        Some(GpuShader {
            id,
            _guard: self.create_guard(ResourceKey::Shader(id)),
        })
    }

    fn write_buffer_resource_with(
        &self,
        id: VersionIndex,
        buffer_size: usize,
        offset: usize,
        mut size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        let state = self.state.lock().unwrap();

        let Some(buffer_resource) = state.resources.buffers.get(id) else {
            return false;
        };
        let buffer = buffer_resource.buffer.clone();

        if offset >= buffer_size {
            return false;
        }

        size = size.min(buffer_size - offset);

        // wgpu panics on unaligned writes instead of reporting a validation error
        let alignment = wgpu::COPY_BUFFER_ALIGNMENT as usize;
        if offset % alignment != 0 || size % alignment != 0 {
            return false;
        }

        let Some(size) = NonZeroU64::new(size as u64) else {
            return false;
        };

        let Some(mut buffer_view) = self.queue.write_buffer_with(&buffer, offset as u64, size) else {
            return false;
        };

        // the writer may call back into the API
        drop(state);

        writer(&mut buffer_view);

        true
    }

    fn write_texture(&self, texture: &GpuTexture, mip_level: usize, data: &[u8]) -> bool {
        let state = self.state.lock().unwrap();

        let Some(texture) = state.resources.textures.get(texture.id) else {
            return false;
        };

        if texture.format() == GpuTextureFormat::Depth32Float {
            return false;
        }

        if mip_level >= texture.mip_level_count() {
            return false;
        }

        let [width, height] = texture.size().map(|x| (x >> mip_level).max(1));
        let bytes_per_row = width * texture.format().bytes_per_pixel() as usize;

        if data.len() < bytes_per_row * height {
            return false;
        }

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture.texture(),
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                depth_or_array_layers: 1,
                width: width as u32,
                height: height as u32,
            },
        );

        true
    }

    fn draw(&self, pass: FfiRenderPass, vertices: Range<usize>, instances: Range<usize>) -> bool {
        self.encode_pass(pass, None, None, |render_pass| {
            render_pass.draw(to_u32_range(vertices), to_u32_range(instances));
        })
    }

    fn clear_color_texture(&self, texture: &GpuTexture, mip_level: usize, color: [f32; 4]) -> bool {
        if texture.format == GpuTextureFormat::Depth32Float {
            return false;
        }

        self.clear_texture_resource(texture, mip_level, |encoder, view| {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aphyn clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: GpuPassOps::Clear(color).to_wgpu(),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
        })
    }

    fn clear_depth_texture(&self, texture: &GpuTexture, mip_level: usize, depth: f32) -> bool {
        if texture.format != GpuTextureFormat::Depth32Float {
            return false;
        }

        self.clear_texture_resource(texture, mip_level, |encoder, view| {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aphyn clear pass"),
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: GpuPassOps::Clear(depth).to_wgpu(),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        })
    }

    fn clear_texture_resource(
        &self,
        texture: &GpuTexture,
        mip_level: usize,
        begin_pass: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> bool {
        let Self { device, queue, .. } = self;

        let state = self.state.lock().unwrap();

        let Some(texture) = state.resources.textures.get(texture.id) else {
            return false;
        };
        let Some(view) = texture.mip_views().get(mip_level) else {
            return false;
        };

        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("aphyn clear encoder"),
        });

        begin_pass(&mut encoder, view);

        queue.submit([encoder.finish()]);

        if let Some(err) = pollster::block_on(device.pop_error_scope()) {
            println!("failed to clear texture: {err}");
            return false;
        }

        true
    }

    fn copy_texture(&self, src: &GpuTexture, dst: &GpuTexture) -> bool {
        if src.id == dst.id {
            return false;
        }

        let state = self.state.lock().unwrap();

        let (Some(src), Some(dst)) = (state.resources.textures.get(src.id), state.resources.textures.get(dst.id)) else {
            return false;
        };

        if src.format() == GpuTextureFormat::Depth32Float || dst.format() == GpuTextureFormat::Depth32Float {
            return false;
        }

        if !src.format().is_copy_compatible(dst.format()) || src.size() != dst.size() {
            return false;
        }

        let [width, height] = src.size();

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("aphyn copy encoder"),
        });

        encoder.copy_texture_to_texture(
            src.texture().as_image_copy(),
            dst.texture().as_image_copy(),
            wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit([encoder.finish()]);

        true
    }

    fn draw_to_surface(
        &self,
        pass: FfiRenderPass,
        ops: GpuPassOps<[f32; 4]>,
        vertices: Range<usize>,
        instances: Range<usize>,
    ) -> bool {
        if pass.target.is_some() {
            return false;
        }

        let Some(surface) = &self.surface else {
            return false;
        };

        // held until present, so that a resize can't reconfigure the surface while a frame is acquired
        let surface = surface.lock().unwrap();

        let format = surface.config.format;

        let Ok(frame) = surface.surface.get_current_texture() else {
            return false;
        };

        let view = frame.texture.create_view(&Default::default());

        let surface_target = SurfaceDrawTarget {
            view: &view,
            format,
            ops,
        };

        let is_drawn = self.encode_pass(pass, None, Some(surface_target), |render_pass| {
            render_pass.draw(to_u32_range(vertices), to_u32_range(instances));
        });

        if is_drawn {
            frame.present();
        }

        is_drawn
    }

    fn draw_indexed(
        &self,
        pass: FfiRenderPass,
        index_buffer: &GpuIndexBuffer,
        indices: Range<usize>,
        instances: Range<usize>,
    ) -> bool {
        self.encode_pass(pass, Some(index_buffer), None, |render_pass| {
            render_pass.draw_indexed(to_u32_range(indices), 0, to_u32_range(instances));
        })
    }

    // todo: indirect draws

    fn resolve_bind_group_entries(
        resources: &RenderApiResources,
        layout: &[BindGroupLayoutEntry],
        group: usize,
        bindings: &[FfiBindGroupEntry],
    ) -> Option<Vec<BindGroupEntry>> {
        layout
            .iter()
            .map(|&(i, ty)| {
                let entry = bindings.iter().find(|entry| entry.group == group && entry.binding == i)?;
                let binding = entry.resource.to_internal();

                ty.accepts(binding, resources).then_some((i, ty, binding))
            })
            .collect()
    }

    fn encode_pass(
        &self,
        pass: FfiRenderPass,
        index_buffer: Option<&GpuIndexBuffer>,
        surface_target: Option<SurfaceDrawTarget>,
        draw: impl FnOnce(&mut wgpu::RenderPass),
    ) -> bool {
        let Self { device, queue, .. } = self;

        let FfiRenderPass {
            shader: shader_handle,
            target: pass_target,
            depth: pass_depth,
            bind_group_entries,
            bind_group_count,
            vertex_buffers: pass_vertex_buffers,
            viewport,
            scissor,
        } = pass;
        let bind_group_entries = bind_group_entries.into_slice();
        let pass_vertex_buffers = pass_vertex_buffers.into_slice();

        // wgpu panics on slices of empty buffers
        if pass_vertex_buffers.iter().any(|buffer| buffer.size() == 0) || index_buffer.is_some_and(|buffer| buffer.size() == 0) {
            return false;
        }

        let mut state = self.state.lock().unwrap();
        let RenderApiState { cache, resources } = &mut *state;

        let Some(shader) = resources.shaders.get(shader_handle.id) else {
            return false;
        };

        if bind_group_count < shader.bind_group_layouts().len() {
            return false;
        }

        let has_duplicate_bindings = bind_group_entries.iter().enumerate().any(|(i, entry)| {
            bind_group_entries[..i]
                .iter()
                .any(|other| other.group == entry.group && other.binding == entry.binding)
        });

        if has_duplicate_bindings {
            return false;
        }

        let vertex_layouts = pass_vertex_buffers.iter().map(|buffer| buffer.layout()).collect::<Vec<_>>();

        if !vertex_layouts_satisfy_attributes(&shader.vertex_attributes(), &vertex_layouts) {
            return false;
        }

        let mut bind_groups = Vec::with_capacity(shader.bind_group_layouts().len());

        for (group, layout) in shader.bind_group_layouts().iter().enumerate() {
            let Some(entries) = Self::resolve_bind_group_entries(resources, layout, group, bind_group_entries) else {
                return false;
            };
            let Some(bind_group) = cache.get_or_create_bind_group(device, resources, &entries) else {
                return false;
            };

            bind_groups.push(bind_group);
        }

        let target = match (surface_target, pass_target) {
            (Some(_), Some(_)) => return false,
            (Some(surface), None) => Some((surface.view, surface.format, surface.ops)),
            (None, None) => None,
            (None, Some(target)) => {
                let Some(texture) = resources.textures.get(target.view.id) else {
                    return false;
                };
                let Some(view) = texture.mip_views().get(target.mip_level) else {
                    return false;
                };

                Some((view, texture.format().to_wgpu(), target.ops))
            }
        };
        let depth = match pass_depth {
            None => None,
            Some(depth) => {
                let Some(texture) = resources.textures.get(depth.view.id) else {
                    return false;
                };
                let Some(view) = texture.mip_views().get(depth.mip_level) else {
                    return false;
                };

                Some((view, texture.format(), depth.ops))
            }
        };

        let pipeline_key = PipelineKey {
            shader: shader_handle.id,
            target: target.map(|(_, format, _)| format),
            depth: depth.map(|(_, format, _)| format),
            // todo: avoid cloning the layouts into the key on every draw
            vertex_buffers: vertex_layouts.iter().map(|&layout| layout.clone()).collect(),
        };

        let Some(pipeline) = cache.get_or_create_pipeline(device, resources, pipeline_key) else {
            return false;
        };

        let Some(vertex_buffers) = pass_vertex_buffers
            .iter()
            .map(|buffer| resources.buffers.get(buffer.id))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let index_buffer = match index_buffer {
            None => None,
            Some(buffer) => match resources.buffers.get(buffer.id) {
                Some(buffer_resource) => Some((buffer_resource, buffer.format)),
                None => return false,
            },
        };

        let color_attachments = target
            .map(|(view, _, ops)| wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: ops.to_wgpu(),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        let depth_stencil_attachment = depth.map(|(view, _, ops)| wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations {
                load: ops.to_wgpu(),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        });

        device.push_error_scope(wgpu::ErrorFilter::Validation);

        // todo: batch draws into a frame encoder instead of submitting per draw
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("aphyn command encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aphyn render pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                ..Default::default()
            });

            render_pass.set_pipeline(&pipeline);
            for (i, bind_group) in bind_groups.iter().enumerate() {
                render_pass.set_bind_group(i as u32, bind_group, &[]);
            }
            for (i, buffer) in vertex_buffers.iter().enumerate() {
                render_pass.set_vertex_buffer(i as u32, buffer.buffer.slice(..));
            }
            if let Some((buffer, format)) = index_buffer {
                render_pass.set_index_buffer(buffer.buffer.slice(..), format.to_wgpu());
            }
            if let Some(viewport) = viewport {
                render_pass.set_viewport(
                    viewport.x,
                    viewport.y,
                    viewport.width,
                    viewport.height,
                    viewport.min_depth,
                    viewport.max_depth,
                );
            }
            if let Some(scissor) = scissor {
                render_pass.set_scissor_rect(
                    scissor.x as u32,
                    scissor.y as u32,
                    scissor.width as u32,
                    scissor.height as u32,
                );
            }

            draw(&mut render_pass);
        }

        queue.submit([encoder.finish()]);

        if let Some(err) = pollster::block_on(device.pop_error_scope()) {
            println!("failed to draw: {err}");
            return false;
        }

        true
    }
}

fn to_u32_range(range: Range<usize>) -> Range<u32> {
    range.start as u32..range.end as u32
}

#[repr(C)]
#[derive(Copy, Clone)]
struct FfiArray<T, const N: usize>([T; N]);

#[repr(C)]
#[derive(Copy, Clone)]
struct FfiRange {
    start: usize,
    end: usize,
}

impl FfiRange {
    fn from_range(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    fn into_range(self) -> Range<usize> {
        self.start..self.end
    }
}

#[repr(C)]
struct FfiRenderPass<'a> {
    shader: &'a GpuShader,
    target: Option<&'a GpuRenderPassTarget<'a>>,
    depth: Option<&'a GpuRenderPassDepth<'a>>,
    // flat, because nested `FfiSliceRef`s can't be read through a shared reference
    bind_group_entries: FfiSliceRef<'a, FfiBindGroupEntry<'a>>,
    bind_group_count: usize,
    vertex_buffers: FfiSliceRef<'a, &'a GpuVertexBuffer>,
    viewport: Option<&'a GpuViewport>,
    scissor: Option<&'a GpuScissor>,
}

#[repr(C)]
struct FfiBindGroupEntry<'a> {
    group: usize,
    binding: usize,
    resource: FfiBinding<'a>,
}

#[repr(C)]
#[derive(Copy, Clone)]
enum FfiBinding<'a> {
    Texture {
        texture: &'a GpuTexture,
        mip_level: FfiOption<usize>,
    },
    Sampler(&'a GpuSampler),
    Buffer(&'a GpuUniformStorageBuffer),
}

impl FfiBinding<'_> {
    fn to_internal(self) -> BindGroupBinding {
        match self {
            Self::Texture { texture, mip_level } => BindGroupBinding::Texture(texture.id, mip_level.into_option()),
            Self::Sampler(sampler) => BindGroupBinding::Sampler(sampler.id),
            Self::Buffer(buffer) => BindGroupBinding::Buffer(buffer.id),
        }
    }
}

#[repr(C)]
struct RenderApiVTable {
    surface_size_fn: unsafe extern "C-unwind" fn(this: *const c_void) -> FfiArray<usize, 2>,
    resize_surface_fn: unsafe extern "C-unwind" fn(this: *const c_void, size: FfiArray<usize, 2>),
    create_vertex_buffer_fn:
        unsafe extern "C-unwind" fn(this: *const c_void, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer,
    create_uniform_storage_buffer_fn: unsafe extern "C-unwind" fn(this: *const c_void, size: usize) -> GpuUniformStorageBuffer,
    create_index_buffer_fn: unsafe extern "C-unwind" fn(this: *const c_void, len: usize, format: GpuIndexFormat) -> GpuIndexBuffer,
    create_texture_fn: unsafe extern "C-unwind" fn(
        this: *const c_void,
        size: FfiArray<usize, 2>,
        format: GpuTextureFormat,
        mip_level_count: usize,
    ) -> GpuTexture,
    create_sampler_fn: unsafe extern "C-unwind" fn(
        this: *const c_void,
        filter: GpuFilterMode,
        compare: FfiOption<GpuCompareFunction>,
    ) -> GpuSampler,
    create_shader_fn:
        unsafe extern "C-unwind" fn(this: *const c_void, wgsl: FfiStrSliceRef<'_>, desc: GpuShaderDesc) -> FfiOption<GpuShader>,
    write_buffer_with_fn: unsafe extern "C-unwind" fn(
        this: *const c_void,
        id: VersionIndex,
        buffer_size: usize,
        offset: usize,
        size: usize,
        writer: FfiFnMutMut<'_, FfiSliceMut<'static, u8>, ()>,
    ) -> bool,
    write_texture_fn:
        unsafe extern "C-unwind" fn(this: *const c_void, texture: &GpuTexture, mip_level: usize, data: FfiSliceRef<'_, u8>) -> bool,
    draw_fn:
        unsafe extern "C-unwind" fn(this: *const c_void, pass: FfiRenderPass<'_>, vertices: FfiRange, instances: FfiRange) -> bool,
    draw_indexed_fn: unsafe extern "C-unwind" fn(
        this: *const c_void,
        pass: FfiRenderPass<'_>,
        index_buffer: &GpuIndexBuffer,
        indices: FfiRange,
        instances: FfiRange,
    ) -> bool,
    draw_to_surface_fn: unsafe extern "C-unwind" fn(
        this: *const c_void,
        pass: FfiRenderPass<'_>,
        ops: GpuPassOps<[f32; 4]>,
        vertices: FfiRange,
        instances: FfiRange,
    ) -> bool,
    clear_color_texture_fn:
        unsafe extern "C-unwind" fn(this: *const c_void, texture: &GpuTexture, mip_level: usize, color: FfiArray<f32, 4>) -> bool,
    clear_depth_texture_fn:
        unsafe extern "C-unwind" fn(this: *const c_void, texture: &GpuTexture, mip_level: usize, depth: f32) -> bool,
    copy_texture_fn: unsafe extern "C-unwind" fn(this: *const c_void, src: &GpuTexture, dst: &GpuTexture) -> bool,
    clone_fn: unsafe extern "C-unwind" fn(this: *const c_void) -> FfiDroppable,
}

unsafe fn inner_ref<'a>(this: *const c_void) -> &'a RenderApiInner {
    unsafe { &*(this as *const Arc<RenderApiInner>) }
}

// todo: rename
#[repr(C)]
pub struct RenderApiNew {
    data: FfiDroppable,
    vtable: FfiStaticRef<RenderApiVTable>,
}

// the data is an `Arc<RenderApiInner>`, which is `Send + Sync`
unsafe impl Send for RenderApiNew {}
unsafe impl Sync for RenderApiNew {}

impl RenderApiNew {
    pub fn new(window: impl HasWindowHandle + HasDisplayHandle + Send + Sync + 'static, size: [usize; 2]) -> Option<Self> {
        RenderApiInner::new(window, size).map(Self::from_inner)
    }

    #[cfg(test)]
    fn new_headless() -> Option<Self> {
        RenderApiInner::new_headless().map(Self::from_inner)
    }

    #[cfg(test)]
    fn inner(&self) -> &RenderApiInner {
        unsafe { inner_ref(self.data.get()) }
    }

    fn from_inner(inner: RenderApiInner) -> Self {
        unsafe extern "C-unwind" fn ffi_surface_size(this: *const c_void) -> FfiArray<usize, 2> {
            FfiArray(unsafe { inner_ref(this) }.surface_size())
        }
        unsafe extern "C-unwind" fn ffi_resize_surface(this: *const c_void, size: FfiArray<usize, 2>) {
            unsafe { inner_ref(this) }.resize_surface(size.0)
        }
        unsafe extern "C-unwind" fn ffi_create_vertex_buffer(
            this: *const c_void,
            len: usize,
            layout: GpuVertexBufferLayout,
        ) -> GpuVertexBuffer {
            unsafe { inner_ref(this) }.create_vertex_buffer(len, layout)
        }
        unsafe extern "C-unwind" fn ffi_create_uniform_storage_buffer(this: *const c_void, size: usize) -> GpuUniformStorageBuffer {
            unsafe { inner_ref(this) }.create_uniform_storage_buffer(size)
        }
        unsafe extern "C-unwind" fn ffi_create_index_buffer(
            this: *const c_void,
            len: usize,
            format: GpuIndexFormat,
        ) -> GpuIndexBuffer {
            unsafe { inner_ref(this) }.create_index_buffer(len, format)
        }
        unsafe extern "C-unwind" fn ffi_create_texture(
            this: *const c_void,
            size: FfiArray<usize, 2>,
            format: GpuTextureFormat,
            mip_level_count: usize,
        ) -> GpuTexture {
            unsafe { inner_ref(this) }.create_texture(size.0, format, mip_level_count)
        }
        unsafe extern "C-unwind" fn ffi_create_sampler(
            this: *const c_void,
            filter: GpuFilterMode,
            compare: FfiOption<GpuCompareFunction>,
        ) -> GpuSampler {
            unsafe { inner_ref(this) }.create_sampler(filter, compare.into_option())
        }
        unsafe extern "C-unwind" fn ffi_create_shader(
            this: *const c_void,
            wgsl: FfiStrSliceRef<'_>,
            desc: GpuShaderDesc,
        ) -> FfiOption<GpuShader> {
            unsafe { inner_ref(this) }.create_shader(wgsl.into_slice(), desc).into()
        }
        unsafe extern "C-unwind" fn ffi_write_buffer_with(
            this: *const c_void,
            id: VersionIndex,
            buffer_size: usize,
            offset: usize,
            size: usize,
            mut writer: FfiFnMutMut<'_, FfiSliceMut<'static, u8>, ()>,
        ) -> bool {
            unsafe { inner_ref(this) }.write_buffer_resource_with(id, buffer_size, offset, size, |bytes| {
                // not actually 'static: the caller side only hands it to an `FnOnce(&mut [u8])`, which can't keep it
                let bytes = unsafe { &mut *(bytes as *mut [u8]) };
                writer.execute(FfiSliceMut::from_slice(bytes))
            })
        }
        unsafe extern "C-unwind" fn ffi_write_texture(
            this: *const c_void,
            texture: &GpuTexture,
            mip_level: usize,
            data: FfiSliceRef<'_, u8>,
        ) -> bool {
            unsafe { inner_ref(this) }.write_texture(texture, mip_level, data.into_slice())
        }
        unsafe extern "C-unwind" fn ffi_draw(
            this: *const c_void,
            pass: FfiRenderPass<'_>,
            vertices: FfiRange,
            instances: FfiRange,
        ) -> bool {
            unsafe { inner_ref(this) }.draw(pass, vertices.into_range(), instances.into_range())
        }
        unsafe extern "C-unwind" fn ffi_draw_indexed(
            this: *const c_void,
            pass: FfiRenderPass<'_>,
            index_buffer: &GpuIndexBuffer,
            indices: FfiRange,
            instances: FfiRange,
        ) -> bool {
            unsafe { inner_ref(this) }.draw_indexed(pass, index_buffer, indices.into_range(), instances.into_range())
        }
        unsafe extern "C-unwind" fn ffi_draw_to_surface(
            this: *const c_void,
            pass: FfiRenderPass<'_>,
            ops: GpuPassOps<[f32; 4]>,
            vertices: FfiRange,
            instances: FfiRange,
        ) -> bool {
            unsafe { inner_ref(this) }.draw_to_surface(pass, ops, vertices.into_range(), instances.into_range())
        }
        unsafe extern "C-unwind" fn ffi_clear_color_texture(
            this: *const c_void,
            texture: &GpuTexture,
            mip_level: usize,
            color: FfiArray<f32, 4>,
        ) -> bool {
            unsafe { inner_ref(this) }.clear_color_texture(texture, mip_level, color.0)
        }
        unsafe extern "C-unwind" fn ffi_clear_depth_texture(
            this: *const c_void,
            texture: &GpuTexture,
            mip_level: usize,
            depth: f32,
        ) -> bool {
            unsafe { inner_ref(this) }.clear_depth_texture(texture, mip_level, depth)
        }
        unsafe extern "C-unwind" fn ffi_copy_texture(this: *const c_void, src: &GpuTexture, dst: &GpuTexture) -> bool {
            unsafe { inner_ref(this) }.copy_texture(src, dst)
        }
        unsafe extern "C-unwind" fn ffi_clone(this: *const c_void) -> FfiDroppable {
            FfiDroppable::new(Arc::clone(unsafe { &*(this as *const Arc<RenderApiInner>) }))
        }

        Self {
            data: FfiDroppable::new(Arc::new(inner)),
            vtable: FfiStaticRef::new(&RenderApiVTable {
                surface_size_fn: ffi_surface_size,
                resize_surface_fn: ffi_resize_surface,
                create_vertex_buffer_fn: ffi_create_vertex_buffer,
                create_uniform_storage_buffer_fn: ffi_create_uniform_storage_buffer,
                create_index_buffer_fn: ffi_create_index_buffer,
                create_texture_fn: ffi_create_texture,
                create_sampler_fn: ffi_create_sampler,
                create_shader_fn: ffi_create_shader,
                write_buffer_with_fn: ffi_write_buffer_with,
                write_texture_fn: ffi_write_texture,
                draw_fn: ffi_draw,
                draw_indexed_fn: ffi_draw_indexed,
                draw_to_surface_fn: ffi_draw_to_surface,
                clear_color_texture_fn: ffi_clear_color_texture,
                clear_depth_texture_fn: ffi_clear_depth_texture,
                copy_texture_fn: ffi_copy_texture,
                clone_fn: ffi_clone,
            }),
        }
    }

    pub fn clone(&self) -> Self {
        Self {
            data: unsafe { (self.vtable.clone_fn)(self.data.get()) },
            vtable: self.vtable,
        }
    }

    pub fn surface_size(&self) -> [usize; 2] {
        unsafe { (self.vtable.surface_size_fn)(self.data.get()) }.0
    }

    pub fn resize_surface(&self, size: [usize; 2]) {
        unsafe { (self.vtable.resize_surface_fn)(self.data.get(), FfiArray(size)) }
    }

    pub fn create_vertex_buffer(&self, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer {
        unsafe { (self.vtable.create_vertex_buffer_fn)(self.data.get(), len, layout) }
    }

    pub fn create_uniform_storage_buffer(&self, size: usize) -> GpuUniformStorageBuffer {
        unsafe { (self.vtable.create_uniform_storage_buffer_fn)(self.data.get(), size) }
    }

    pub fn create_index_buffer(&self, len: usize, format: GpuIndexFormat) -> GpuIndexBuffer {
        unsafe { (self.vtable.create_index_buffer_fn)(self.data.get(), len, format) }
    }

    pub fn create_texture(&self, size: [usize; 2], format: GpuTextureFormat, mip_level_count: usize) -> GpuTexture {
        unsafe { (self.vtable.create_texture_fn)(self.data.get(), FfiArray(size), format, mip_level_count) }
    }

    pub fn create_sampler(&self, filter: GpuFilterMode) -> GpuSampler {
        unsafe { (self.vtable.create_sampler_fn)(self.data.get(), filter, FfiOption::None) }
    }

    pub fn create_comparison_sampler(&self, filter: GpuFilterMode, compare: GpuCompareFunction) -> GpuSampler {
        unsafe { (self.vtable.create_sampler_fn)(self.data.get(), filter, FfiOption::Some(compare)) }
    }

    pub fn create_shader(&self, wgsl: &str, desc: GpuShaderDesc) -> Option<GpuShader> {
        unsafe { (self.vtable.create_shader_fn)(self.data.get(), FfiStrSliceRef::from_slice(wgsl), desc) }.into_option()
    }

    pub fn write_vertex_buffer_with(
        &self,
        buffer: &GpuVertexBuffer,
        offset: usize,
        size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        self.write_buffer_with(buffer.id, buffer.size(), offset, size, writer)
    }

    pub fn write_uniform_storage_buffer_with(
        &self,
        buffer: &GpuUniformStorageBuffer,
        offset: usize,
        size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        self.write_buffer_with(buffer.id, buffer.size, offset, size, writer)
    }

    pub fn write_index_buffer_with(
        &self,
        buffer: &GpuIndexBuffer,
        offset: usize,
        size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        self.write_buffer_with(buffer.id, buffer.size(), offset, size, writer)
    }

    fn write_buffer_with(
        &self,
        id: VersionIndex,
        buffer_size: usize,
        offset: usize,
        size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        let mut writer = Some(writer);
        let mut writer = |bytes: FfiSliceMut<'static, u8>| {
            if let Some(writer) = writer.take() {
                writer(bytes.into_slice_mut());
            }
        };

        unsafe { (self.vtable.write_buffer_with_fn)(self.data.get(), id, buffer_size, offset, size, FfiFnMutMut::new(&mut writer)) }
    }

    pub fn write_texture(&self, texture: &GpuTexture, mip_level: usize, data: &[u8]) -> bool {
        unsafe { (self.vtable.write_texture_fn)(self.data.get(), texture, mip_level, FfiSliceRef::from_slice(data)) }
    }

    pub fn draw(&self, pass: &GpuRenderPass, vertices: Range<usize>, instances: Range<usize>) -> bool {
        pass.with_ffi(|pass| unsafe {
            (self.vtable.draw_fn)(
                self.data.get(),
                pass,
                FfiRange::from_range(vertices),
                FfiRange::from_range(instances),
            )
        })
    }

    pub fn draw_indexed(
        &self,
        pass: &GpuRenderPass,
        index_buffer: &GpuIndexBuffer,
        indices: Range<usize>,
        instances: Range<usize>,
    ) -> bool {
        pass.with_ffi(|pass| unsafe {
            (self.vtable.draw_indexed_fn)(
                self.data.get(),
                pass,
                index_buffer,
                FfiRange::from_range(indices),
                FfiRange::from_range(instances),
            )
        })
    }

    // todo: allow several draws per frame
    pub fn draw_to_surface(
        &self,
        pass: &GpuRenderPass,
        ops: GpuPassOps<[f32; 4]>,
        vertices: Range<usize>,
        instances: Range<usize>,
    ) -> bool {
        pass.with_ffi(|pass| unsafe {
            (self.vtable.draw_to_surface_fn)(
                self.data.get(),
                pass,
                ops,
                FfiRange::from_range(vertices),
                FfiRange::from_range(instances),
            )
        })
    }

    pub fn clear_color_texture(&self, texture: &GpuTexture, mip_level: usize, color: [f32; 4]) -> bool {
        unsafe { (self.vtable.clear_color_texture_fn)(self.data.get(), texture, mip_level, FfiArray(color)) }
    }

    pub fn clear_depth_texture(&self, texture: &GpuTexture, mip_level: usize, depth: f32) -> bool {
        unsafe { (self.vtable.clear_depth_texture_fn)(self.data.get(), texture, mip_level, depth) }
    }

    pub fn copy_texture(&self, src: &GpuTexture, dst: &GpuTexture) -> bool {
        unsafe { (self.vtable.copy_texture_fn)(self.data.get(), src, dst) }
    }
}

#[repr(C)]
pub struct GpuShader {
    id: VersionIndex,
    _guard: FfiTypedDroppable<ResourceGuard>,
}

impl GpuShader {
    pub fn id(&self) -> GpuResourceId {
        GpuResourceId(ResourceKey::Shader(self.id))
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct GpuShaderDesc {
    pub topology: GpuPrimitiveTopology,
    pub front_face: GpuFrontFace,
    pub cull_mode: GpuCullMode,
    pub blend: GpuBlendState,
    pub write_mask: GpuColorWrites,
    pub depth_write: bool,
    pub depth_compare: GpuCompareFunction,
}

impl Default for GpuShaderDesc {
    fn default() -> Self {
        Self {
            topology: Default::default(),
            front_face: Default::default(),
            cull_mode: Default::default(),
            blend: Default::default(),
            write_mask: Default::default(),
            depth_write: true,
            depth_compare: Default::default(),
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuPrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    #[default]
    TriangleList,
    TriangleStrip,
}

impl GpuPrimitiveTopology {
    pub(crate) fn to_wgpu(self) -> wgpu::PrimitiveTopology {
        match self {
            Self::PointList => wgpu::PrimitiveTopology::PointList,
            Self::LineList => wgpu::PrimitiveTopology::LineList,
            Self::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            Self::TriangleList => wgpu::PrimitiveTopology::TriangleList,
            Self::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuFrontFace {
    #[default]
    Ccw,
    Cw,
}

impl GpuFrontFace {
    pub(crate) fn to_wgpu(self) -> wgpu::FrontFace {
        match self {
            Self::Ccw => wgpu::FrontFace::Ccw,
            Self::Cw => wgpu::FrontFace::Cw,
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuCullMode {
    None,
    Front,
    #[default]
    Back,
}

impl GpuCullMode {
    pub(crate) fn to_wgpu(self) -> Option<wgpu::Face> {
        match self {
            Self::None => None,
            Self::Front => Some(wgpu::Face::Front),
            Self::Back => Some(wgpu::Face::Back),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GpuBlendState {
    pub color: GpuBlendComponent,
    pub alpha: GpuBlendComponent,
}

impl GpuBlendState {
    pub const REPLACE: Self = Self {
        color: GpuBlendComponent::REPLACE,
        alpha: GpuBlendComponent::REPLACE,
    };

    pub const ALPHA_BLENDING: Self = Self {
        color: GpuBlendComponent {
            src_factor: GpuBlendFactor::SrcAlpha,
            dst_factor: GpuBlendFactor::OneMinusSrcAlpha,
            operation: GpuBlendOperation::Add,
        },
        alpha: GpuBlendComponent::OVER,
    };

    pub const PREMULTIPLIED_ALPHA_BLENDING: Self = Self {
        color: GpuBlendComponent::OVER,
        alpha: GpuBlendComponent::OVER,
    };

    pub(crate) fn to_wgpu(self) -> Option<wgpu::BlendState> {
        Some(wgpu::BlendState {
            color: self.color.to_wgpu(),
            alpha: self.alpha.to_wgpu(),
        })
    }
}

impl Default for GpuBlendState {
    fn default() -> Self {
        Self::REPLACE
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GpuBlendComponent {
    pub src_factor: GpuBlendFactor,
    pub dst_factor: GpuBlendFactor,
    pub operation: GpuBlendOperation,
}

impl GpuBlendComponent {
    pub const REPLACE: Self = Self {
        src_factor: GpuBlendFactor::One,
        dst_factor: GpuBlendFactor::Zero,
        operation: GpuBlendOperation::Add,
    };

    pub const OVER: Self = Self {
        src_factor: GpuBlendFactor::One,
        dst_factor: GpuBlendFactor::OneMinusSrcAlpha,
        operation: GpuBlendOperation::Add,
    };

    fn to_wgpu(self) -> wgpu::BlendComponent {
        wgpu::BlendComponent {
            src_factor: self.src_factor.to_wgpu(),
            dst_factor: self.dst_factor.to_wgpu(),
            operation: self.operation.to_wgpu(),
        }
    }
}

// todo: constant factors (needs a way to set the blend constant) and dual-source `Src1*` factors
#[repr(C)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuBlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
}

impl GpuBlendFactor {
    fn to_wgpu(self) -> wgpu::BlendFactor {
        match self {
            Self::Zero => wgpu::BlendFactor::Zero,
            Self::One => wgpu::BlendFactor::One,
            Self::Src => wgpu::BlendFactor::Src,
            Self::OneMinusSrc => wgpu::BlendFactor::OneMinusSrc,
            Self::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            Self::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            Self::Dst => wgpu::BlendFactor::Dst,
            Self::OneMinusDst => wgpu::BlendFactor::OneMinusDst,
            Self::DstAlpha => wgpu::BlendFactor::DstAlpha,
            Self::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
            Self::SrcAlphaSaturated => wgpu::BlendFactor::SrcAlphaSaturated,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuBlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

impl GpuBlendOperation {
    fn to_wgpu(self) -> wgpu::BlendOperation {
        match self {
            Self::Add => wgpu::BlendOperation::Add,
            Self::Subtract => wgpu::BlendOperation::Subtract,
            Self::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
            Self::Min => wgpu::BlendOperation::Min,
            Self::Max => wgpu::BlendOperation::Max,
        }
    }
}

// todo: per-channel write masks
#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuColorWrites {
    #[default]
    All,
    Color,
}

impl GpuColorWrites {
    pub(crate) fn to_wgpu(self) -> wgpu::ColorWrites {
        match self {
            Self::All => wgpu::ColorWrites::ALL,
            Self::Color => wgpu::ColorWrites::COLOR,
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuCompareFunction {
    Never,
    Less,
    Equal,
    #[default]
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl GpuCompareFunction {
    pub(crate) fn to_wgpu(self) -> wgpu::CompareFunction {
        match self {
            Self::Never => wgpu::CompareFunction::Never,
            Self::Less => wgpu::CompareFunction::Less,
            Self::Equal => wgpu::CompareFunction::Equal,
            Self::LessEqual => wgpu::CompareFunction::LessEqual,
            Self::Greater => wgpu::CompareFunction::Greater,
            Self::NotEqual => wgpu::CompareFunction::NotEqual,
            Self::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
            Self::Always => wgpu::CompareFunction::Always,
        }
    }
}

#[repr(C)]
pub struct GpuRenderPass<'a> {
    pub shader: &'a GpuShader,
    pub target: Option<GpuRenderPassTarget<'a>>,
    pub depth: Option<GpuRenderPassDepth<'a>>,
    pub bind_groups: &'a [&'a [(usize, GpuBinding<'a>)]],
    pub vertex_buffers: &'a [&'a GpuVertexBuffer],
    pub viewport: Option<GpuViewport>,
    pub scissor: Option<GpuScissor>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum GpuBinding<'a> {
    // `None` binds all mip levels
    Texture {
        texture: &'a GpuTexture,
        mip_level: Option<usize>,
    },
    Sampler(&'a GpuSampler),
    Buffer(&'a GpuUniformStorageBuffer),
}

impl<'a> GpuBinding<'a> {
    fn to_ffi(self) -> FfiBinding<'a> {
        match self {
            Self::Texture { texture, mip_level } => FfiBinding::Texture {
                texture,
                mip_level: mip_level.into(),
            },
            Self::Sampler(sampler) => FfiBinding::Sampler(sampler),
            Self::Buffer(buffer) => FfiBinding::Buffer(buffer),
        }
    }
}

impl GpuRenderPass<'_> {
    fn with_ffi<R>(&self, f: impl FnOnce(FfiRenderPass) -> R) -> R {
        let bind_group_entries = self
            .bind_groups
            .iter()
            .enumerate()
            .flat_map(|(group, bindings)| {
                bindings.iter().map(move |&(binding, resource)| FfiBindGroupEntry {
                    group,
                    binding,
                    resource: resource.to_ffi(),
                })
            })
            .collect::<Vec<_>>();

        f(FfiRenderPass {
            shader: self.shader,
            target: self.target.as_ref(),
            depth: self.depth.as_ref(),
            bind_group_entries: FfiSliceRef::from_slice(&bind_group_entries),
            bind_group_count: self.bind_groups.len(),
            vertex_buffers: FfiSliceRef::from_slice(self.vertex_buffers),
            viewport: self.viewport.as_ref(),
            scissor: self.scissor.as_ref(),
        })
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq)]
pub struct GpuViewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct GpuScissor {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuPassOps<T> {
    Clear(T),
    #[default]
    Load,
}

impl GpuPassOps<[f32; 4]> {
    fn to_wgpu(self) -> LoadOp<wgpu::Color> {
        match self {
            Self::Clear(color) => {
                let [r, g, b, a] = color.map(|f| f as f64);

                LoadOp::Clear(wgpu::Color { r, g, b, a })
            }
            Self::Load => LoadOp::Load,
        }
    }
}
impl GpuPassOps<f32> {
    fn to_wgpu(self) -> LoadOp<f32> {
        match self {
            Self::Clear(color) => LoadOp::Clear(color),
            Self::Load => LoadOp::Load,
        }
    }
}

#[repr(C)]
pub struct GpuRenderPassTarget<'a> {
    pub view: &'a GpuTexture,
    pub mip_level: usize,
    pub ops: GpuPassOps<[f32; 4]>,
}
#[repr(C)]
pub struct GpuRenderPassDepth<'a> {
    pub view: &'a GpuTexture,
    pub mip_level: usize,
    pub ops: GpuPassOps<f32>,
}

#[repr(C)]
pub struct GpuTexture {
    id: VersionIndex,
    format: GpuTextureFormat,
    size: [usize; 2],
    mip_level_count: usize,
    _guard: FfiTypedDroppable<ResourceGuard>,
}

impl GpuTexture {
    pub fn id(&self) -> GpuResourceId {
        GpuResourceId(ResourceKey::Texture(self.id))
    }

    pub fn format(&self) -> GpuTextureFormat {
        self.format
    }

    pub fn size(&self) -> [usize; 2] {
        self.size
    }

    pub fn mip_level_count(&self) -> usize {
        self.mip_level_count
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuTextureFormat {
    Rgba8Unorm,
    #[default]
    Rgba8UnormSrgb,
    Rgba16Float,
    Depth32Float,
}

impl GpuTextureFormat {
    pub(crate) fn to_wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Self::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
            Self::Depth32Float => wgpu::TextureFormat::Depth32Float,
        }
    }
    fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Rgba8Unorm | Self::Rgba8UnormSrgb | Self::Depth32Float => 4,
            Self::Rgba16Float => 8,
        }
    }
    fn is_copy_compatible(self, other: Self) -> bool {
        self.to_wgpu().remove_srgb_suffix() == other.to_wgpu().remove_srgb_suffix()
    }
    // storage textures are rejected by reflection, and depth formats can't be copy destinations
    fn usages(self) -> wgpu::TextureUsages {
        match self {
            Self::Rgba8Unorm | Self::Rgba8UnormSrgb | Self::Rgba16Float => {
                wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
            }
            Self::Depth32Float => {
                wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
            }
        }
    }
}

#[repr(C)]
pub struct GpuSampler {
    id: VersionIndex,
    _guard: FfiTypedDroppable<ResourceGuard>,
}

impl GpuSampler {
    pub fn id(&self) -> GpuResourceId {
        GpuResourceId(ResourceKey::Sampler(self.id))
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuFilterMode {
    #[default]
    Nearest,
    Linear,
}

impl GpuFilterMode {
    fn to_wgpu(self) -> wgpu::FilterMode {
        match self {
            Self::Nearest => wgpu::FilterMode::Nearest,
            Self::Linear => wgpu::FilterMode::Linear,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffers::GpuVertexStepMode;

    const FULLSCREEN_WGSL: &str = r#"
        @vertex
        fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
            let uv = vec2f(f32((index << 1) & 2), f32(index & 2));
            return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
        }

        @fragment
        fn fs_main() -> @location(0) vec4f {
            return vec4f(1.0);
        }
    "#;

    fn pass_without_target(shader: &GpuShader) -> GpuRenderPass<'_> {
        GpuRenderPass {
            shader,
            target: None,
            depth: None,
            bind_groups: &[],
            vertex_buffers: &[],
            viewport: None,
            scissor: None,
        }
    }

    #[test]
    fn draws_into_a_texture_without_a_surface() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: &target,
                mip_level: 0,
                ops: GpuPassOps::Clear([0.0; 4]),
            }),
            ..pass_without_target(&shader)
        };

        assert!(api.draw(&pass, 0..3, 0..1));
    }

    const COLOR_FORMATS: [GpuTextureFormat; 3] = [
        GpuTextureFormat::Rgba8Unorm,
        GpuTextureFormat::Rgba8UnormSrgb,
        GpuTextureFormat::Rgba16Float,
    ];

    #[test]
    fn creates_and_draws_into_textures_of_all_formats() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let depth = api.create_texture([4, 4], GpuTextureFormat::Depth32Float, 1);

        for format in COLOR_FORMATS {
            let _with_mips = api.create_texture([4, 4], format, 3);
            let target = api.create_texture([4, 4], format, 1);

            let pass = GpuRenderPass {
                target: Some(GpuRenderPassTarget {
                    view: &target,
                    mip_level: 0,
                    ops: GpuPassOps::Clear([0.0; 4]),
                }),
                depth: Some(GpuRenderPassDepth {
                    view: &depth,
                    mip_level: 0,
                    ops: GpuPassOps::Clear(1.0),
                }),
                ..pass_without_target(&shader)
            };

            assert!(api.draw(&pass, 0..3, 0..1));
        }
    }

    #[test]
    fn draws_with_engine_blend_states_and_write_masks() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let additive = GpuBlendComponent {
            src_factor: GpuBlendFactor::One,
            dst_factor: GpuBlendFactor::One,
            operation: GpuBlendOperation::Add,
        };
        let descs = [
            (GpuBlendState::REPLACE, GpuColorWrites::All),
            (GpuBlendState::ALPHA_BLENDING, GpuColorWrites::All),
            (GpuBlendState::PREMULTIPLIED_ALPHA_BLENDING, GpuColorWrites::All),
            (
                GpuBlendState {
                    color: GpuBlendComponent {
                        src_factor: GpuBlendFactor::SrcAlpha,
                        ..additive
                    },
                    alpha: additive,
                },
                GpuColorWrites::All,
            ),
            (
                GpuBlendState {
                    color: additive,
                    alpha: additive,
                },
                GpuColorWrites::All,
            ),
            (GpuBlendState::ALPHA_BLENDING, GpuColorWrites::Color),
        ];

        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba16Float, 1);

        for (blend, write_mask) in descs {
            let desc = GpuShaderDesc {
                blend,
                write_mask,
                ..Default::default()
            };
            let shader = api.create_shader(FULLSCREEN_WGSL, desc).unwrap();

            let pass = GpuRenderPass {
                target: Some(GpuRenderPassTarget {
                    view: &target,
                    mip_level: 0,
                    ops: GpuPassOps::Clear([0.0; 4]),
                }),
                ..pass_without_target(&shader)
            };

            assert!(api.draw(&pass, 0..3, 0..1), "{blend:?} {write_mask:?}");
        }
    }

    fn read_texture_rgba8(api: &RenderApiNew, texture: &GpuTexture, mip_level: usize) -> Vec<u8> {
        let [width, height] = texture.size.map(|x| (x >> mip_level).max(1));
        let bytes_per_row = width * 4;
        let padded_bytes_per_row = bytes_per_row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);

        let buffer = api.inner().device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = api.inner().device.create_command_encoder(&Default::default());
        {
            let state = api.inner().state.lock().unwrap();
            let texture = state.resources.textures.get(texture.id).unwrap();

            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    mip_level: mip_level as u32,
                    ..texture.texture().as_image_copy()
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row as u32),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: width as u32,
                    height: height as u32,
                    depth_or_array_layers: 1,
                },
            );
        }
        api.inner().queue.submit([encoder.finish()]);

        buffer.slice(..).map_async(wgpu::MapMode::Read, |result| result.unwrap());
        api.inner().device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

        let mapped = buffer.slice(..).get_mapped_range();

        mapped
            .chunks(padded_bytes_per_row)
            .flat_map(|row| &row[..bytes_per_row])
            .copied()
            .collect()
    }

    #[test]
    fn clears_color_textures_but_not_with_depth() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        for format in COLOR_FORMATS {
            for mip_level_count in [1, 3] {
                let texture = api.create_texture([4, 4], format, mip_level_count);

                assert!(
                    api.clear_color_texture(&texture, 0, [0.0, 0.5, 1.0, 1.0]),
                    "{format:?} {mip_level_count}"
                );
                assert!(!api.clear_depth_texture(&texture, 0, 1.0), "{format:?} {mip_level_count}");
            }
        }
    }

    #[test]
    fn clears_depth_textures_but_not_with_color() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let texture = api.create_texture([4, 4], GpuTextureFormat::Depth32Float, 1);

        assert!(api.clear_depth_texture(&texture, 0, 1.0));
        assert!(!api.clear_color_texture(&texture, 0, [0.0; 4]));
    }

    #[test]
    fn clear_sets_the_pixels() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        assert!(api.clear_color_texture(&texture, 0, [1.0, 0.0, 1.0, 0.0]));

        let pixels = read_texture_rgba8(&api, &texture, 0);

        assert_eq!(pixels, [255, 0, 255, 0].repeat(16));
    }

    #[test]
    fn clears_a_single_mip_level() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 3);

        assert_eq!(texture.mip_level_count(), 3);
        assert!(api.clear_color_texture(&texture, 0, [1.0, 0.0, 0.0, 1.0]));
        assert!(api.clear_color_texture(&texture, 1, [0.0, 1.0, 0.0, 1.0]));
        assert!(api.clear_color_texture(&texture, 2, [0.0, 0.0, 1.0, 1.0]));

        assert_eq!(read_texture_rgba8(&api, &texture, 0), [255, 0, 0, 255].repeat(16));
        assert_eq!(read_texture_rgba8(&api, &texture, 1), [0, 255, 0, 255].repeat(4));
        assert_eq!(read_texture_rgba8(&api, &texture, 2), [0, 0, 255, 255]);
    }

    #[test]
    fn draws_into_a_single_mip_level() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 3);

        for mip_level in 0..3 {
            for clear_level in 0..3 {
                assert!(api.clear_color_texture(&texture, clear_level, [0.0; 4]));
            }

            let pass = GpuRenderPass {
                target: Some(GpuRenderPassTarget {
                    view: &texture,
                    mip_level,
                    ops: GpuPassOps::Load,
                }),
                ..pass_without_target(&shader)
            };

            assert!(api.draw(&pass, 0..3, 0..1), "{mip_level}");

            for read_level in 0..3 {
                let pixel = match read_level == mip_level {
                    true => [255; 4],
                    false => [0; 4],
                };
                let pixel_count = 16 >> (2 * read_level);

                assert_eq!(
                    read_texture_rgba8(&api, &texture, read_level),
                    pixel.repeat(pixel_count),
                    "{mip_level} {read_level}"
                );
            }
        }
    }

    const SAMPLE_MIP_WGSL: &str = r#"
        @group(0) @binding(0) var tex: texture_2d<f32>;
        @group(0) @binding(1) var samp: sampler;

        @vertex
        fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
            let uv = vec2f(f32((index << 1) & 2), f32(index & 2));
            return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
        }

        @fragment
        fn fs_main() -> @location(0) vec4f {
            return textureSampleLevel(tex, samp, vec2f(0.5), 0.0);
        }
    "#;

    fn draw_sampling_mip(
        api: &RenderApiNew,
        shader: &GpuShader,
        target: &GpuTexture,
        target_mip_level: usize,
        texture: &GpuTexture,
        mip_level: Option<usize>,
        sampler: &GpuSampler,
    ) -> bool {
        let bindings = [
            (0, GpuBinding::Texture { texture, mip_level }),
            (1, GpuBinding::Sampler(sampler)),
        ];
        let bind_groups: [&[(usize, GpuBinding)]; 1] = [&bindings];

        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: target,
                mip_level: target_mip_level,
                ops: GpuPassOps::Load,
            }),
            bind_groups: &bind_groups,
            ..pass_without_target(shader)
        };

        api.draw(&pass, 0..3, 0..1)
    }

    const GROUP_1_ONLY_WGSL: &str = r#"
        @group(1) @binding(2) var tex: texture_2d<f32>;
        @group(1) @binding(3) var samp: sampler;

        @vertex
        fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
            let uv = vec2f(f32((index << 1) & 2), f32(index & 2));
            return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
        }

        @fragment
        fn fs_main() -> @location(0) vec4f {
            return textureSampleLevel(tex, samp, vec2f(0.5), 0.0);
        }
    "#;

    fn draw_with_bind_groups(
        api: &RenderApiNew,
        shader: &GpuShader,
        target: &GpuTexture,
        bind_groups: &[&[(usize, GpuBinding)]],
    ) -> bool {
        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: target,
                mip_level: 0,
                ops: GpuPassOps::Load,
            }),
            bind_groups,
            ..pass_without_target(shader)
        };

        api.draw(&pass, 0..3, 0..1)
    }

    #[test]
    fn ignores_bindings_and_groups_the_shader_does_not_declare() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let buffer = api.create_uniform_storage_buffer(16);

        let tex = GpuBinding::Texture {
            texture: &texture,
            mip_level: None,
        };
        let samp = GpuBinding::Sampler(&sampler);
        let buf = GpuBinding::Buffer(&buffer);

        let sample_mip = api.create_shader(SAMPLE_MIP_WGSL, GpuShaderDesc::default()).unwrap();
        let fullscreen = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let group_1_only = api.create_shader(GROUP_1_ONLY_WGSL, GpuShaderDesc::default()).unwrap();

        assert!(draw_with_bind_groups(
            &api,
            &sample_mip,
            &target,
            &[&[(0, tex), (1, samp), (5, buf)]]
        ));
        assert!(draw_with_bind_groups(
            &api,
            &sample_mip,
            &target,
            &[&[(0, tex), (1, samp)], &[(0, buf)]]
        ));
        assert!(draw_with_bind_groups(&api, &fullscreen, &target, &[&[(0, tex)], &[(0, buf)]]));
        assert!(draw_with_bind_groups(
            &api,
            &group_1_only,
            &target,
            &[&[], &[(2, tex), (3, samp)]]
        ));
        assert!(draw_with_bind_groups(
            &api,
            &group_1_only,
            &target,
            &[&[(0, buf)], &[(3, samp), (2, tex), (4, buf)]]
        ));
    }

    #[test]
    fn fails_when_declared_bindings_are_missing_mistyped_or_duplicated() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let buffer = api.create_uniform_storage_buffer(16);

        let tex = GpuBinding::Texture {
            texture: &texture,
            mip_level: None,
        };
        let samp = GpuBinding::Sampler(&sampler);
        let buf = GpuBinding::Buffer(&buffer);

        let sample_mip = api.create_shader(SAMPLE_MIP_WGSL, GpuShaderDesc::default()).unwrap();
        let fullscreen = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let group_1_only = api.create_shader(GROUP_1_ONLY_WGSL, GpuShaderDesc::default()).unwrap();

        assert!(!draw_with_bind_groups(&api, &sample_mip, &target, &[]));
        assert!(!draw_with_bind_groups(&api, &sample_mip, &target, &[&[(0, tex)]]));
        assert!(!draw_with_bind_groups(&api, &sample_mip, &target, &[&[(0, samp), (1, samp)]]));
        assert!(!draw_with_bind_groups(&api, &sample_mip, &target, &[&[(0, tex), (1, buf)]]));
        assert!(!draw_with_bind_groups(
            &api,
            &sample_mip,
            &target,
            &[&[(0, tex), (1, samp), (0, tex)]]
        ));
        assert!(!draw_with_bind_groups(
            &api,
            &sample_mip,
            &target,
            &[&[(0, tex), (1, samp), (5, buf), (5, buf)]]
        ));
        assert!(!draw_with_bind_groups(
            &api,
            &sample_mip,
            &target,
            &[&[(0, tex), (1, samp)], &[(0, buf), (0, buf)]]
        ));
        assert!(!draw_with_bind_groups(&api, &fullscreen, &target, &[&[(0, buf), (0, buf)]]));
        assert!(!draw_with_bind_groups(
            &api,
            &group_1_only,
            &target,
            &[&[(2, tex), (3, samp)]]
        ));
        assert!(!draw_with_bind_groups(
            &api,
            &group_1_only,
            &target,
            &[&[(2, tex), (3, samp)], &[(2, tex)]]
        ));
    }

    #[test]
    fn draws_a_mip_level_from_the_previous_one() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(SAMPLE_MIP_WGSL, GpuShaderDesc::default()).unwrap();
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 3);

        assert!(api.clear_color_texture(&texture, 0, [1.0, 0.0, 1.0, 0.0]));

        for mip_level in 1..3 {
            assert!(draw_sampling_mip(
                &api,
                &shader,
                &texture,
                mip_level,
                &texture,
                Some(mip_level - 1),
                &sampler
            ));
        }

        assert_eq!(read_texture_rgba8(&api, &texture, 1), [255, 0, 255, 0].repeat(4));
        assert_eq!(read_texture_rgba8(&api, &texture, 2), [255, 0, 255, 0]);

        assert!(!draw_sampling_mip(&api, &shader, &texture, 1, &texture, None, &sampler));
        assert!(!draw_sampling_mip(&api, &shader, &texture, 1, &texture, Some(1), &sampler));
    }

    #[test]
    fn draws_into_a_depth_mip_level() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 2);
        let depth = api.create_texture([4, 4], GpuTextureFormat::Depth32Float, 2);

        assert!(api.clear_depth_texture(&depth, 1, 1.0));

        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: &target,
                mip_level: 1,
                ops: GpuPassOps::Clear([0.0; 4]),
            }),
            depth: Some(GpuRenderPassDepth {
                view: &depth,
                mip_level: 1,
                ops: GpuPassOps::Load,
            }),
            ..pass_without_target(&shader)
        };

        assert!(api.draw(&pass, 0..3, 0..1));
    }

    #[test]
    fn rejects_out_of_range_mip_levels() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(SAMPLE_MIP_WGSL, GpuShaderDesc::default()).unwrap();
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 3);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 3);
        let depth = api.create_texture([4, 4], GpuTextureFormat::Depth32Float, 1);

        assert!(draw_sampling_mip(&api, &shader, &target, 2, &texture, Some(2), &sampler));
        assert!(!draw_sampling_mip(&api, &shader, &target, 3, &texture, Some(2), &sampler));
        assert!(!draw_sampling_mip(&api, &shader, &target, 2, &texture, Some(3), &sampler));

        assert!(!api.clear_color_texture(&texture, 3, [0.0; 4]));
        assert!(!api.clear_depth_texture(&depth, 1, 1.0));

        let fullscreen = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let pass = GpuRenderPass {
            depth: Some(GpuRenderPassDepth {
                view: &depth,
                mip_level: 1,
                ops: GpuPassOps::Clear(1.0),
            }),
            ..pass_without_target(&fullscreen)
        };

        assert!(!api.draw(&pass, 0..3, 0..1));
    }

    #[test]
    fn dropping_a_texture_evicts_bind_groups_of_all_its_mip_levels() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(SAMPLE_MIP_WGSL, GpuShaderDesc::default()).unwrap();
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 2);

        let bind_group_count = |api: &RenderApiNew| api.inner().state.lock().unwrap().cache.bind_groups().len();

        for mip_level in [None, Some(0), Some(1)] {
            assert!(draw_sampling_mip(&api, &shader, &target, 0, &texture, mip_level, &sampler));
        }
        assert_eq!(bind_group_count(&api), 3);

        drop(texture);
        assert_eq!(bind_group_count(&api), 0);
    }

    #[test]
    fn writes_color_textures_but_not_depth() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        for format in COLOR_FORMATS {
            let texture = api.create_texture([4, 4], format, 1);
            let data = vec![0; 4 * 4 * format.bytes_per_pixel() as usize];

            assert!(api.write_texture(&texture, 0, &data));
        }

        let depth = api.create_texture([4, 4], GpuTextureFormat::Depth32Float, 1);

        assert!(!api.write_texture(&depth, 0, &[0; 4 * 4 * 4]));
    }

    #[test]
    fn copy_sets_the_pixels() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let src = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let dst = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        assert!(api.clear_color_texture(&src, 0, [1.0, 0.0, 1.0, 0.0]));
        assert!(api.clear_color_texture(&dst, 0, [0.0, 1.0, 0.0, 1.0]));
        assert!(api.copy_texture(&src, &dst));

        let pixels = read_texture_rgba8(&api, &dst, 0);

        assert_eq!(pixels, [255, 0, 255, 0].repeat(16));
    }

    #[test]
    fn copies_between_srgb_and_linear() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let linear = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let srgb = api.create_texture([4, 4], GpuTextureFormat::Rgba8UnormSrgb, 1);

        assert!(api.copy_texture(&linear, &srgb));
        assert!(api.copy_texture(&srgb, &linear));
    }

    #[test]
    fn copies_between_textures_with_different_mip_counts() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let single = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let with_mips = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 3);

        assert!(api.copy_texture(&single, &with_mips));
        assert!(api.copy_texture(&with_mips, &single));
    }

    #[test]
    fn copy_rejects_mismatches() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let other_size = api.create_texture([8, 8], GpuTextureFormat::Rgba8Unorm, 1);
        let other_format = api.create_texture([4, 4], GpuTextureFormat::Rgba16Float, 1);
        let depth = api.create_texture([4, 4], GpuTextureFormat::Depth32Float, 1);
        let other_depth = api.create_texture([4, 4], GpuTextureFormat::Depth32Float, 1);

        assert!(!api.copy_texture(&texture, &other_size));
        assert!(!api.copy_texture(&texture, &other_format));
        assert!(!api.copy_texture(&depth, &other_depth));
        assert!(!api.copy_texture(&texture, &texture));
    }

    #[test]
    fn surface_operations_without_a_surface_do_nothing() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();

        api.resize_surface([4, 4]);

        assert_eq!(api.surface_size(), [0, 0]);
        assert!(!api.draw_to_surface(&pass_without_target(&shader), GpuPassOps::Load, 0..3, 0..1));
    }

    #[test]
    fn api_is_shareable_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<RenderApiNew>();
    }

    #[test]
    fn draws_and_writes_from_several_threads() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
                    let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
                    let buffer = api.create_uniform_storage_buffer(16);

                    let pass = GpuRenderPass {
                        target: Some(GpuRenderPassTarget {
                            view: &target,
                            mip_level: 0,
                            ops: GpuPassOps::Clear([0.0; 4]),
                        }),
                        ..pass_without_target(&shader)
                    };

                    for _ in 0..16 {
                        assert!(api.write_uniform_storage_buffer_with(&buffer, 0, 16, |bytes| bytes.fill(1)));
                        assert!(api.draw(&pass, 0..3, 0..1));
                    }
                });
            }
        });
    }

    #[test]
    fn writer_can_call_back_into_the_api() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let buffer = api.create_uniform_storage_buffer(16);

        let is_written = api.write_uniform_storage_buffer_with(&buffer, 0, 16, |bytes| {
            let other = api.create_uniform_storage_buffer(16);

            assert!(api.write_uniform_storage_buffer_with(&other, 0, 16, |other_bytes| other_bytes.fill(2)));

            bytes.fill(1);
        });

        assert!(is_written);
    }

    const BINDINGS_WGSL: &str = r#"
        @group(0) @binding(0) var tex: texture_2d<f32>;
        @group(0) @binding(1) var samp: sampler;
        @group(0) @binding(2) var<uniform> color: vec4f;

        @vertex
        fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
            let uv = vec2f(f32((index << 1) & 2), f32(index & 2));
            return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
        }

        @fragment
        fn fs_main() -> @location(0) vec4f {
            return textureSample(tex, samp, vec2f(0.0)) * color;
        }
    "#;

    fn draw_with_bindings(
        api: &RenderApiNew,
        shader: &GpuShader,
        target: &GpuTexture,
        texture: &GpuTexture,
        sampler: &GpuSampler,
        buffer: &GpuUniformStorageBuffer,
    ) -> bool {
        let bindings = [
            (
                0,
                GpuBinding::Texture {
                    texture,
                    mip_level: None,
                },
            ),
            (1, GpuBinding::Sampler(sampler)),
            (2, GpuBinding::Buffer(buffer)),
        ];
        let bind_groups: [&[(usize, GpuBinding)]; 1] = [&bindings];

        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: target,
                mip_level: 0,
                ops: GpuPassOps::Clear([0.0; 4]),
            }),
            bind_groups: &bind_groups,
            ..pass_without_target(shader)
        };

        api.draw(&pass, 0..3, 0..1)
    }

    const VERTEX_INPUT_WGSL: &str = r#"
        @vertex
        fn vs_main(@location(0) position: vec2f) -> @builtin(position) vec4f {
            return vec4f(position, 0.0, 1.0);
        }

        @fragment
        fn fs_main() -> @location(0) vec4f {
            return vec4f(1.0);
        }
    "#;

    fn vertex_input_layout() -> GpuVertexBufferLayout {
        GpuVertexBufferLayout::new(GpuVertexStepMode::Vertex, [(0, GpuVertexBufferLayoutEntryTy::Float2)]).unwrap()
    }

    #[test]
    fn creates_empty_buffers() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let vertices = api.create_vertex_buffer(0, vertex_input_layout());
        let uniforms = api.create_uniform_storage_buffer(0);
        let indices = api.create_index_buffer(0, GpuIndexFormat::U16);

        assert_eq!([vertices.size(), uniforms.size(), indices.size()], [0, 0, 0]);
        assert_eq!(api.inner().state.lock().unwrap().resources.buffers.len(), 3);

        assert!(!api.write_vertex_buffer_with(&vertices, 0, 8, |_| unreachable!()));
        assert!(!api.write_uniform_storage_buffer_with(&uniforms, 0, 4, |_| unreachable!()));
        assert!(!api.write_index_buffer_with(&indices, 0, 2, |_| unreachable!()));
    }

    #[test]
    fn uniform_storage_buffer_writes_stay_within_the_requested_size() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let buffer = api.create_uniform_storage_buffer(6);

        assert_eq!(buffer.size(), 6);
        assert!(!api.write_uniform_storage_buffer_with(&buffer, 6, 2, |_| unreachable!()));
        assert!(api.write_uniform_storage_buffer_with(&buffer, 0, 4, |bytes| assert_eq!(bytes.len(), 4)));
    }

    #[test]
    fn unaligned_writes_fail() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let uniforms = api.create_uniform_storage_buffer(6);
        let indices = api.create_index_buffer(3, GpuIndexFormat::U16);

        assert!(!api.write_uniform_storage_buffer_with(&uniforms, 4, 2, |_| unreachable!()));
        assert!(!api.write_uniform_storage_buffer_with(&uniforms, 0, 6, |_| unreachable!()));
        assert!(!api.write_uniform_storage_buffer_with(&uniforms, 2, 2, |_| unreachable!()));
        assert!(!api.write_index_buffer_with(&indices, 0, 6, |_| unreachable!()));
        assert!(api.write_index_buffer_with(&indices, 0, 4, |bytes| assert_eq!(bytes.len(), 4)));
    }

    #[test]
    fn draw_with_an_empty_vertex_buffer_fails() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(VERTEX_INPUT_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let empty = api.create_vertex_buffer(0, vertex_input_layout());
        let filled = api.create_vertex_buffer(3, vertex_input_layout());

        let draw = |vertex_buffers: &[&GpuVertexBuffer], vertices| {
            let pass = GpuRenderPass {
                target: Some(GpuRenderPassTarget {
                    view: &target,
                    mip_level: 0,
                    ops: GpuPassOps::Clear([0.0; 4]),
                }),
                vertex_buffers,
                ..pass_without_target(&shader)
            };

            api.draw(&pass, vertices, 0..1)
        };

        assert!(draw(&[&filled], 0..3));
        assert!(!draw(&[&empty], 0..0));
        assert!(!draw(&[&empty], 0..3));
    }

    #[test]
    fn draw_indexed_with_an_empty_index_buffer_fails() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(VERTEX_INPUT_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let vertices = api.create_vertex_buffer(3, vertex_input_layout());
        let empty = api.create_index_buffer(0, GpuIndexFormat::U16);
        let filled = api.create_index_buffer(3, GpuIndexFormat::U16);

        let vertex_buffers = [&vertices];
        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: &target,
                mip_level: 0,
                ops: GpuPassOps::Clear([0.0; 4]),
            }),
            vertex_buffers: &vertex_buffers,
            ..pass_without_target(&shader)
        };

        assert!(api.draw_indexed(&pass, &filled, 0..3, 0..1));
        assert!(!api.draw_indexed(&pass, &empty, 0..0, 0..1));
        assert!(!api.draw_indexed(&pass, &empty, 0..3, 0..1));
    }

    #[test]
    fn empty_uniform_storage_buffers_can_be_bound_where_they_fit() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let fitting_wgsl = r#"
            @group(0) @binding(0) var<uniform> scale: f32;
            @group(0) @binding(1) var<storage, read> values: array<f32>;

            @vertex
            fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
                let uv = vec2f(f32((index << 1) & 2), f32(index & 2));
                return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
            }

            @fragment
            fn fs_main() -> @location(0) vec4f {
                return vec4f(scale * values[0]);
            }
        "#;

        let fitting_shader = api.create_shader(fitting_wgsl, GpuShaderDesc::default()).unwrap();
        let larger_shader = api.create_shader(BINDINGS_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let uniform = api.create_uniform_storage_buffer(0);
        let storage = api.create_uniform_storage_buffer(0);

        let bindings = [(0, GpuBinding::Buffer(&uniform)), (1, GpuBinding::Buffer(&storage))];
        let bind_groups: [&[(usize, GpuBinding)]; 1] = [&bindings];
        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: &target,
                mip_level: 0,
                ops: GpuPassOps::Clear([0.0; 4]),
            }),
            bind_groups: &bind_groups,
            ..pass_without_target(&fitting_shader)
        };

        assert!(api.draw(&pass, 0..3, 0..1));
        assert!(!draw_with_bindings(
            &api,
            &larger_shader,
            &target,
            &texture,
            &sampler,
            &uniform
        ));
    }

    #[test]
    fn dropping_handles_releases_resources() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let layout = GpuVertexBufferLayout::new(GpuVertexStepMode::Vertex, [(0, GpuVertexBufferLayoutEntryTy::Float)]).unwrap();

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let vertices = api.create_vertex_buffer(4, layout);
        let uniforms = api.create_uniform_storage_buffer(16);
        let indices = api.create_index_buffer(4, GpuIndexFormat::U16);

        let counts = |api: &RenderApiNew| {
            let state = api.inner().state.lock().unwrap();

            [
                state.resources.textures.len(),
                state.resources.samplers.len(),
                state.resources.shaders.len(),
                state.resources.buffers.len(),
            ]
        };

        assert_eq!(counts(&api), [1, 1, 1, 3]);

        drop(texture);
        assert_eq!(counts(&api), [0, 1, 1, 3]);
        drop(sampler);
        assert_eq!(counts(&api), [0, 0, 1, 3]);
        drop(shader);
        assert_eq!(counts(&api), [0, 0, 0, 3]);
        drop(vertices);
        assert_eq!(counts(&api), [0, 0, 0, 2]);
        drop(uniforms);
        assert_eq!(counts(&api), [0, 0, 0, 1]);
        drop(indices);
        assert_eq!(counts(&api), [0, 0, 0, 0]);
    }

    #[test]
    fn dropping_a_bound_resource_evicts_only_its_bind_groups() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(BINDINGS_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let other_texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let other_sampler = api.create_sampler(GpuFilterMode::Linear);
        let buffer = api.create_uniform_storage_buffer(16);
        let other_buffer = api.create_uniform_storage_buffer(16);

        let bind_group_count = |api: &RenderApiNew| api.inner().state.lock().unwrap().cache.bind_groups().len();

        assert!(draw_with_bindings(&api, &shader, &target, &texture, &sampler, &buffer));
        assert!(draw_with_bindings(&api, &shader, &target, &other_texture, &sampler, &buffer));
        assert_eq!(bind_group_count(&api), 2);

        drop(texture);
        assert_eq!(bind_group_count(&api), 1);

        assert!(draw_with_bindings(
            &api,
            &shader,
            &target,
            &other_texture,
            &other_sampler,
            &buffer
        ));
        assert_eq!(bind_group_count(&api), 2);

        drop(sampler);
        assert_eq!(bind_group_count(&api), 1);

        assert!(draw_with_bindings(
            &api,
            &shader,
            &target,
            &other_texture,
            &other_sampler,
            &other_buffer
        ));
        assert_eq!(bind_group_count(&api), 2);

        drop(buffer);
        assert_eq!(bind_group_count(&api), 1);
    }

    #[test]
    fn dropping_a_shader_evicts_only_its_pipelines() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let other_shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        for shader in [&shader, &other_shader] {
            let pass = GpuRenderPass {
                target: Some(GpuRenderPassTarget {
                    view: &target,
                    mip_level: 0,
                    ops: GpuPassOps::Clear([0.0; 4]),
                }),
                ..pass_without_target(shader)
            };

            assert!(api.draw(&pass, 0..3, 0..1));
        }

        let pipeline_count = |api: &RenderApiNew| api.inner().state.lock().unwrap().cache.pipelines().len();

        assert_eq!(pipeline_count(&api), 2);

        drop(shader);

        assert_eq!(pipeline_count(&api), 1);
    }

    #[test]
    fn handles_can_outlive_the_api() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let buffer = api.create_uniform_storage_buffer(16);

        drop(api);
        drop(texture);
        drop(buffer);
    }

    #[test]
    fn clones_share_resources() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let clone = api.clone();

        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let target = clone.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: &target,
                mip_level: 0,
                ops: GpuPassOps::Clear([0.0; 4]),
            }),
            ..pass_without_target(&shader)
        };

        assert!(clone.draw(&pass, 0..3, 0..1));

        drop(api);

        assert!(clone.draw(&pass, 0..3, 0..1));
        assert_eq!(clone.inner().state.lock().unwrap().resources.textures.len(), 1);
    }

    #[test]
    fn handles_outlive_all_clones() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let clone = api.clone();
        let texture = clone.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        drop(api);
        drop(clone);
        drop(texture);
    }

    #[test]
    fn same_binding_index_in_different_groups_is_allowed() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Linear);

        let tex = GpuBinding::Texture {
            texture: &texture,
            mip_level: None,
        };
        let samp = GpuBinding::Sampler(&sampler);

        let group_1_only = api.create_shader(GROUP_1_ONLY_WGSL, GpuShaderDesc::default()).unwrap();

        assert!(draw_with_bind_groups(
            &api,
            &group_1_only,
            &target,
            &[&[(2, samp), (3, tex)], &[(2, tex), (3, samp)]]
        ));
        assert!(!draw_with_bind_groups(
            &api,
            &group_1_only,
            &target,
            &[&[(2, tex), (3, samp)], &[(3, tex), (2, samp)]]
        ));
    }

    #[test]
    fn writer_can_drop_a_handle() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let buffer = api.create_uniform_storage_buffer(16);
        let other = api.create_uniform_storage_buffer(16);

        assert!(api.write_uniform_storage_buffer_with(&buffer, 0, 16, |bytes| {
            drop(other);
            bytes.fill(1);
        }));
        assert_eq!(api.inner().state.lock().unwrap().resources.buffers.len(), 1);
    }

    #[test]
    fn new_resource_in_a_reused_slot_gets_a_new_bind_group() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let shader = api.create_shader(BINDINGS_WGSL, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Linear);
        let buffer = api.create_uniform_storage_buffer(16);

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let old_id = texture.id;

        assert!(draw_with_bindings(&api, &shader, &target, &texture, &sampler, &buffer));

        drop(texture);

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        assert_eq!(texture.id.index, old_id.index);
        assert_ne!(texture.id.version, old_id.version);
        assert!(draw_with_bindings(&api, &shader, &target, &texture, &sampler, &buffer));
        assert_eq!(api.inner().state.lock().unwrap().cache.bind_groups().len(), 1);
    }

    #[test]
    fn handles_are_shareable_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<GpuTexture>();
        assert_send_sync::<GpuSampler>();
        assert_send_sync::<GpuShader>();
        assert_send_sync::<GpuVertexBuffer>();
        assert_send_sync::<GpuUniformStorageBuffer>();
        assert_send_sync::<GpuIndexBuffer>();
    }

    #[test]
    fn drops_handles_while_other_threads_draw() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    let shader = api.create_shader(BINDINGS_WGSL, GpuShaderDesc::default()).unwrap();
                    let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
                    let sampler = api.create_sampler(GpuFilterMode::Linear);
                    let buffer = api.create_uniform_storage_buffer(16);

                    for _ in 0..16 {
                        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

                        assert!(draw_with_bindings(&api, &shader, &target, &texture, &sampler, &buffer));
                    }
                });
            }
        });

        let state = api.inner().state.lock().unwrap();

        assert_eq!(state.resources.textures.len(), 0);
        assert_eq!(state.cache.bind_groups().len(), 0);
    }

    #[test]
    fn reflects_bind_group_layouts() {
        let wgsl = r#"
            @group(0) @binding(1) var tex: texture_2d<f32>;
            @group(0) @binding(0) var samp: sampler;
            @group(0) @binding(2) var shadow: texture_depth_2d;
            @group(0) @binding(3) var shadow_samp: sampler_comparison;
            @group(2) @binding(0) var<uniform> globals: vec4f;
            @group(2) @binding(1) var<storage, read> items: array<vec4f>;
            @group(2) @binding(2) var<storage, read_write> output: array<vec4f>;

            @vertex
            fn vs_main() -> @builtin(position) vec4f {
                return globals + items[0];
            }

            @fragment
            fn fs_main() -> @location(0) vec4f {
                output[0] = vec4f(1.0);
                return textureSample(tex, samp, vec2f(0.0)) * textureSampleCompare(shadow, shadow_samp, vec2f(0.0), 0.5);
            }
        "#;

        let layouts = reflect_bind_group_layouts(&parse_shader(wgsl).unwrap()).unwrap();

        assert_eq!(
            layouts,
            vec![
                vec![
                    (0, BindGroupBindingTy::Sampler),
                    (1, BindGroupBindingTy::Texture(TextureSampleKind::Float)),
                    (2, BindGroupBindingTy::Texture(TextureSampleKind::Depth)),
                    (3, BindGroupBindingTy::ComparisonSampler),
                ],
                vec![],
                vec![
                    (0, BindGroupBindingTy::UniformBuffer),
                    (1, BindGroupBindingTy::StorageBuffer { read_only: true }),
                    (2, BindGroupBindingTy::StorageBuffer { read_only: false }),
                ],
            ]
        );
    }

    #[test]
    fn rejects_storage_textures() {
        let wgsl = r#"
            @group(0) @binding(0) var img: texture_storage_2d<rgba8unorm, write>;

            @fragment
            fn fs_main() -> @location(0) vec4f {
                textureStore(img, vec2u(0), vec4f(1.0));
                return vec4f(1.0);
            }
        "#;

        assert_eq!(reflect_bind_group_layouts(&parse_shader(wgsl).unwrap()), None);
    }

    #[test]
    fn reflects_vertex_attributes_from_arguments_and_structs() {
        let wgsl = r#"
            struct Instance {
                @location(3) color: vec4f,
                @location(1) id: u32,
                @builtin(instance_index) instance: u32,
            }

            @vertex
            fn vs_main(@location(2) position: vec3f, @builtin(vertex_index) vertex: u32, instance: Instance) -> @builtin(position) vec4f {
                return vec4f(position, 1.0);
            }
        "#;

        assert_eq!(
            reflect_vertex_attributes(&parse_shader(wgsl).unwrap()),
            Some(vec![
                (1, GpuVertexBufferLayoutEntryTy::UInt),
                (2, GpuVertexBufferLayoutEntryTy::Float3),
                (3, GpuVertexBufferLayoutEntryTy::Float4),
            ])
        );
    }

    #[test]
    fn rejects_shaders_without_a_vertex_entry_point() {
        let wgsl = r#"
            @fragment
            fn fs_main() -> @location(0) vec4f {
                return vec4f(1.0);
            }
        "#;

        assert_eq!(reflect_vertex_attributes(&parse_shader(wgsl).unwrap()), None);
    }

    #[test]
    fn checks_vertex_layouts_against_attributes() {
        let layout = |entries: &[(usize, GpuVertexBufferLayoutEntryTy)]| {
            GpuVertexBufferLayout::new(GpuVertexStepMode::Vertex, entries.iter().copied()).unwrap()
        };

        let attributes = [
            (0, GpuVertexBufferLayoutEntryTy::Float3),
            (1, GpuVertexBufferLayoutEntryTy::UInt),
        ];

        let vertices = layout(&[
            (0, GpuVertexBufferLayoutEntryTy::Float3),
            (5, GpuVertexBufferLayoutEntryTy::Float2),
        ]);
        let instances = layout(&[(1, GpuVertexBufferLayoutEntryTy::UInt)]);
        let wrong_type = layout(&[(1, GpuVertexBufferLayoutEntryTy::Int)]);
        let duplicate = layout(&[
            (0, GpuVertexBufferLayoutEntryTy::Float3),
            (1, GpuVertexBufferLayoutEntryTy::UInt),
        ]);

        assert!(vertex_layouts_satisfy_attributes(&attributes, &[&vertices, &instances]));
        assert!(!vertex_layouts_satisfy_attributes(&attributes, &[&vertices]));
        assert!(!vertex_layouts_satisfy_attributes(&attributes, &[&vertices, &wrong_type]));
        assert!(!vertex_layouts_satisfy_attributes(&attributes, &[&vertices, &duplicate]));
    }

    #[test]
    fn rejects_fragment_inputs_the_vertex_stage_does_not_output() {
        let wgsl = r#"
            @vertex
            fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
                return vec4f(0.0);
            }

            @fragment
            fn fs_main(@location(0) color: vec4f) -> @location(0) vec4f {
                return color;
            }
        "#;

        assert_eq!(check_stage_interface(&parse_shader(wgsl).unwrap()), None);

        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        assert!(api.create_shader(wgsl, GpuShaderDesc::default()).is_none());
    }

    #[test]
    fn accepts_matching_stage_interfaces_through_structs() {
        let wgsl = r#"
            struct VertexOutput {
                @builtin(position) position: vec4f,
                @location(0) color: vec4f,
                @location(1) uv: vec2f,
                @location(2) @interpolate(flat) id: u32,
                @location(3) unused: vec3f,
            }

            struct FragmentInput {
                @builtin(position) position: vec4f,
                @location(1) uv: vec2f,
                @location(2) @interpolate(flat) id: u32,
            }

            @vertex
            fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
                return VertexOutput(vec4f(0.0), vec4f(1.0), vec2f(0.0), index, vec3f(0.0));
            }

            @fragment
            fn fs_main(input: FragmentInput, @location(0) color: vec4f) -> @location(0) vec4f {
                return color * input.uv.x * f32(input.id);
            }
        "#;

        assert_eq!(check_stage_interface(&parse_shader(wgsl).unwrap()), Some(()));

        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        assert!(api.create_shader(wgsl, GpuShaderDesc::default()).is_some());
    }

    #[test]
    fn rejects_stage_interface_type_mismatches() {
        let wgsl = r#"
            struct VertexOutput {
                @builtin(position) position: vec4f,
                @location(0) color: vec3f,
            }

            @vertex
            fn vs_main() -> VertexOutput {
                return VertexOutput(vec4f(0.0), vec3f(1.0));
            }

            @fragment
            fn fs_main(@location(0) color: vec4f) -> @location(0) vec4f {
                return color;
            }
        "#;

        assert_eq!(check_stage_interface(&parse_shader(wgsl).unwrap()), None);

        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        assert!(api.create_shader(wgsl, GpuShaderDesc::default()).is_none());
    }

    #[test]
    fn rejects_shaders_without_a_fragment_entry_point() {
        let wgsl = r#"
            @vertex
            fn vs_main() -> @builtin(position) vec4f {
                return vec4f(0.0);
            }
        "#;

        assert_eq!(check_stage_interface(&parse_shader(wgsl).unwrap()), None);
    }

    #[test]
    fn failed_pipelines_are_created_once_and_evicted_with_the_shader() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let wgsl = r#"
            @vertex
            fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
                return vec4f(0.0);
            }

            @fragment
            fn fs_main() -> @location(0) vec4u {
                return vec4u(1);
            }
        "#;

        let shader = api.create_shader(wgsl, GpuShaderDesc::default()).unwrap();
        let target = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        let pass = GpuRenderPass {
            target: Some(GpuRenderPassTarget {
                view: &target,
                mip_level: 0,
                ops: GpuPassOps::Clear([0.0; 4]),
            }),
            ..pass_without_target(&shader)
        };

        assert!(!api.draw(&pass, 0..3, 0..1));
        assert!(!api.draw(&pass, 0..3, 0..1));

        {
            let state = api.inner().state.lock().unwrap();

            assert_eq!(state.cache.pipelines().len(), 1);
            assert!(state.cache.pipelines().values().all(Option::is_none));
        }

        drop(shader);

        assert_eq!(api.inner().state.lock().unwrap().cache.pipelines().len(), 0);
    }

    #[test]
    fn resource_id_is_stable_for_the_same_handle() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        assert_eq!(texture.id(), texture.id());
    }

    #[test]
    fn live_textures_have_different_resource_ids() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let first = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let second = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn resource_id_is_not_reused_after_release() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let dropped = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let dropped_index = dropped.id;
        let dropped_id = dropped.id();
        drop(dropped);

        let reused = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        assert_eq!(reused.id.index, dropped_index.index);
        assert_ne!(reused.id(), dropped_id);
    }

    #[test]
    fn resource_ids_differ_between_kinds() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };

        let texture = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let sampler = api.create_sampler(GpuFilterMode::Nearest);
        let shader = api.create_shader(FULLSCREEN_WGSL, GpuShaderDesc::default()).unwrap();
        let buffer = api.create_uniform_storage_buffer(4);

        assert_eq!(texture.id, sampler.id);
        assert_eq!(texture.id, shader.id);
        assert_eq!(texture.id, buffer.id);

        let ids = [texture.id(), sampler.id(), shader.id(), buffer.id()];

        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn resource_ids_are_shared_between_api_clones() {
        let Some(api) = RenderApiNew::new_headless() else {
            return;
        };
        let clone = api.clone();

        let original = api.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);
        let through_clone = clone.create_texture([4, 4], GpuTextureFormat::Rgba8Unorm, 1);

        assert_ne!(original.id(), through_clone.id());
        assert_eq!(through_clone.id.index, original.id.index + 1);
    }
}
