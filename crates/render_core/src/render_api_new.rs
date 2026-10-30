// todo: update to the latest wgpu
// todo: expose labels
// todo: ffi
// todo: use Results instead of Option / bool
// todo: destroy resources (and evict cached bind groups, pipelines)

use std::{borrow::Borrow, collections::HashMap, hash::Hash, num::NonZeroU64, ops::Range};

use aphyn_utils::index_version_collection::{VersionCollection, VersionIndex};
use wgpu::LoadOp;

use crate::buffers::{
    BufferResource, GpuIndexBuffer, GpuIndexFormat, GpuUniformStorageBuffer, GpuVertexBuffer, GpuVertexBufferLayout, GpuVertexBufferLayoutEntryTy,
};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum TextureSampleKind {
    Float,
    Sint,
    Uint,
    Depth,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum BindGroupBindingTy {
    Texture(TextureSampleKind),
    Sampler,
    ComparisonSampler,
    UniformBuffer,
    StorageBuffer { read_only: bool },
    // todo: storage textures
}

impl BindGroupBindingTy {
    fn to_wgpu(self) -> wgpu::BindingType {
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

    fn visibility(self) -> wgpu::ShaderStages {
        match self {
            // writable storage in the vertex stage needs `Features::VERTEX_WRITABLE_STORAGE`
            Self::StorageBuffer { read_only: false } => wgpu::ShaderStages::FRAGMENT,
            _ => wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }

    fn accepts(self, binding: BindGroupBinding, resources: &RenderApiResources) -> bool {
        match (self, binding) {
            (Self::Texture(kind), BindGroupBinding::Texture(texture)) => match resources.textures.get(texture) {
                Some(texture) => match texture.format {
                    GpuTextureFormat::Depth32Float => kind == TextureSampleKind::Depth,
                    _ => kind == TextureSampleKind::Float,
                },
                None => false,
            },
            (Self::Sampler | Self::ComparisonSampler, BindGroupBinding::Sampler(sampler)) => match resources.samplers.get(sampler) {
                Some(sampler) => sampler.comparison == (self == Self::ComparisonSampler),
                None => false,
            },
            (Self::UniformBuffer | Self::StorageBuffer { .. }, BindGroupBinding::Buffer(buffer)) => {
                resources.buffers.contains_index(buffer)
            }
            _ => false,
        }
    }
}

type BindGroupLayoutEntry = (usize, BindGroupBindingTy);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct BindGroupLayoutEntries {
    entries: Vec<BindGroupLayoutEntry>,
}

impl Borrow<[BindGroupLayoutEntry]> for BindGroupLayoutEntries {
    fn borrow(&self) -> &[BindGroupLayoutEntry] {
        &self.entries
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum BindGroupBinding {
    Texture(VersionIndex),
    Sampler(VersionIndex),
    Buffer(VersionIndex),
}

impl BindGroupBinding {
    fn to_wgpu<'a>(self, resources: &'a RenderApiResources) -> Option<wgpu::BindingResource<'a>> {
        Some(match self {
            Self::Texture(texture) => wgpu::BindingResource::TextureView(&resources.textures.get(texture)?.texture_view),
            Self::Sampler(sampler) => wgpu::BindingResource::Sampler(&resources.samplers.get(sampler)?.sampler),
            Self::Buffer(buffer) => resources.buffers.get(buffer)?.buffer.as_entire_binding(),
        })
    }
}

type BindGroupEntry = (usize, BindGroupBindingTy, BindGroupBinding);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct BindGroupEntries {
    entries: Vec<BindGroupEntry>,
}

impl Borrow<[BindGroupEntry]> for BindGroupEntries {
    fn borrow(&self) -> &[BindGroupEntry] {
        &self.entries
    }
}

type VertexAttributeEntry = (usize, GpuVertexBufferLayoutEntryTy);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct PipelineKey {
    shader: VersionIndex,
    target: Option<GpuTextureFormat>,
    depth: Option<GpuTextureFormat>,
    vertex_buffers: Vec<GpuVertexBufferLayout>,
}

struct TextureResource {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    format: GpuTextureFormat,
    size: [usize; 2],
    mip_level_count: usize,
}

struct SamplerResource {
    sampler: wgpu::Sampler,
    comparison: bool,
}

struct ShaderResource {
    module: wgpu::ShaderModule,
    desc: GpuShaderDesc,
    bind_group_layouts: Vec<Vec<BindGroupLayoutEntry>>,
    vertex_attributes: Vec<VertexAttributeEntry>,
}

#[derive(Default)]
struct RenderApiResources {
    textures: VersionCollection<TextureResource>,
    buffers: VersionCollection<BufferResource>,
    samplers: VersionCollection<SamplerResource>,
    shaders: VersionCollection<ShaderResource>,
}

#[derive(Default)]
struct RenderApiCache {
    bind_group_layouts: HashMap<BindGroupLayoutEntries, wgpu::BindGroupLayout>,
    bind_groups: HashMap<BindGroupEntries, wgpu::BindGroup>,
    pipeline_layouts: HashMap<Vec<BindGroupLayoutEntries>, wgpu::PipelineLayout>,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
}

impl RenderApiCache {
    fn get_or_create_bind_group_layout(
        &mut self,
        device: &wgpu::Device,
        bindings: &[BindGroupLayoutEntry],
    ) -> wgpu::BindGroupLayout {
        if let Some(layout) = self.bind_group_layouts.get(bindings) {
            return layout.clone();
        }

        // todo: reuse a single buffer
        let wgpu_bindings = bindings
            .iter()
            .copied()
            .map(|(i, ty)| wgpu::BindGroupLayoutEntry {
                count: None,
                visibility: ty.visibility(),
                binding: i as u32,
                ty: ty.to_wgpu(),
            })
            .collect::<Vec<_>>();

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aphyn bind group layout"),
            entries: &wgpu_bindings,
        });

        let owned_bindings = bindings.to_vec();
        self.bind_group_layouts
            .insert(BindGroupLayoutEntries { entries: owned_bindings }, layout.clone());

        layout
    }

    fn get_or_create_bind_group(
        &mut self,
        device: &wgpu::Device,
        resources: &RenderApiResources,
        bindings: &[BindGroupEntry],
    ) -> Option<wgpu::BindGroup> {
        if let Some(bind_group) = self.bind_groups.get(bindings) {
            return Some(bind_group.clone());
        }

        let binding_types = bindings.iter().map(|&(i, ty, _)| (i, ty)).collect::<Vec<_>>();
        let bind_group_layout = self.get_or_create_bind_group_layout(device, &binding_types);

        // todo: reuse a single buffer
        let wgpu_bindings = bindings
            .iter()
            .map(|&(i, _, binding)| {
                Some(wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: binding.to_wgpu(resources)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aphyn bind group"),
            entries: &wgpu_bindings,
            layout: &bind_group_layout,
        });

        let owned_bindings = bindings.to_vec();
        self.bind_groups
            .insert(BindGroupEntries { entries: owned_bindings }, bind_group.clone());

        Some(bind_group)
    }

    fn get_or_create_pipeline_layout(
        &mut self,
        device: &wgpu::Device,
        groups: &[Vec<BindGroupLayoutEntry>],
    ) -> wgpu::PipelineLayout {
        let key = groups
            .iter()
            .map(|entries| BindGroupLayoutEntries {
                entries: entries.clone(),
            })
            .collect::<Vec<_>>();

        if let Some(layout) = self.pipeline_layouts.get(&key) {
            return layout.clone();
        }

        let bind_group_layouts = groups
            .iter()
            .map(|entries| self.get_or_create_bind_group_layout(device, entries))
            .collect::<Vec<_>>();
        let bind_group_layout_refs = bind_group_layouts.iter().collect::<Vec<_>>();

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aphyn pipeline layout"),
            bind_group_layouts: &bind_group_layout_refs,
            push_constant_ranges: &[],
        });

        self.pipeline_layouts.insert(key, layout.clone());

        layout
    }

    fn get_or_create_pipeline(
        &mut self,
        device: &wgpu::Device,
        resources: &RenderApiResources,
        key: PipelineKey,
    ) -> Option<wgpu::RenderPipeline> {
        if let Some(pipeline) = self.pipelines.get(&key) {
            return Some(pipeline.clone());
        }

        let shader = resources.shaders.get(key.shader)?;
        let desc = &shader.desc;

        let layout = self.get_or_create_pipeline_layout(device, &shader.bind_group_layouts);

        let vertex_attributes = key
            .vertex_buffers
            .iter()
            .map(|layout| layout.to_wgpu_attributes())
            .collect::<Vec<_>>();
        let vertex_buffers = key
            .vertex_buffers
            .iter()
            .zip(&vertex_attributes)
            .map(|(layout, attributes)| layout.desc(attributes))
            .collect::<Vec<_>>();

        let color_targets = key
            .target
            .map(|format| wgpu::ColorTargetState {
                format: format.to_wgpu(),
                blend: desc.blend.to_wgpu(),
                write_mask: wgpu::ColorWrites::ALL,
            })
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();

        let depth_stencil = key.depth.map(|format| wgpu::DepthStencilState {
            format: format.to_wgpu(),
            depth_write_enabled: desc.depth_write,
            depth_compare: desc.depth_compare.to_wgpu(),
            stencil: Default::default(),
            bias: Default::default(),
        });

        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aphyn render pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader.module,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader.module,
                entry_point: Some("fs_main"),
                targets: &color_targets,
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: desc.topology.to_wgpu(),
                strip_index_format: None,
                front_face: desc.front_face.to_wgpu(),
                cull_mode: desc.cull_mode.to_wgpu(),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        if let Some(err) = pollster::block_on(device.pop_error_scope()) {
            println!("failed to create render pipeline: {err}");
            return None;
        }

        self.pipelines.insert(key, pipeline.clone());

        Some(pipeline)
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

// todo: rename
#[repr(C)]
pub struct RenderApiNew {
    device: wgpu::Device,
    queue: wgpu::Queue,
    cache: RenderApiCache,
    resources: RenderApiResources,
}

impl RenderApiNew {
    // todo: create the device and the surface here, and allow rendering into the surface
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            cache: Default::default(),
            resources: Default::default(),
        }
    }

    pub fn create_vertex_buffer(&mut self, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer {
        let id = self.create_buffer_resource("aphyn vertex buffer", len * layout.total_size(), wgpu::BufferUsages::VERTEX);

        GpuVertexBuffer { id, layout, len }
    }

    pub fn create_uniform_storage_buffer(&mut self, size: usize) -> GpuUniformStorageBuffer {
        let id = self.create_buffer_resource(
            "aphyn uniform storage buffer",
            size,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::STORAGE,
        );

        GpuUniformStorageBuffer { id, size }
    }

    pub fn create_index_buffer(&mut self, len: usize, format: GpuIndexFormat) -> GpuIndexBuffer {
        let id = self.create_buffer_resource("aphyn index buffer", len * format.gpu_size(), wgpu::BufferUsages::INDEX);

        GpuIndexBuffer { id, format, len }
    }

    fn create_buffer_resource(&mut self, label: &str, size: usize, usage: wgpu::BufferUsages) -> VersionIndex {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size as u64,
            usage: usage | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        self.resources.buffers.insert(BufferResource { buffer })
    }

    pub fn create_texture(&mut self, mut size: [usize; 2], format: GpuTextureFormat, mip_level_count: usize) -> GpuTexture {
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
            // todo: STORAGE_BINDING is not allowed on sRGB formats, so creating an Rgba8UnormSrgb texture panics
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[wgpu_format],
        });

        let texture_view = texture.create_view(&Default::default());

        let texture_resource = TextureResource {
            texture,
            texture_view,
            format,
            size,
            mip_level_count,
        };

        let id = self.resources.textures.insert(texture_resource);

        GpuTexture { id, format, size }
    }

    pub fn create_sampler(&mut self, filter: GpuFilterMode) -> GpuSampler {
        self.create_sampler_inner(filter, None)
    }

    pub fn create_comparison_sampler(&mut self, filter: GpuFilterMode, compare: GpuCompareFunction) -> GpuSampler {
        self.create_sampler_inner(filter, Some(compare))
    }

    fn create_sampler_inner(&mut self, filter: GpuFilterMode, compare: Option<GpuCompareFunction>) -> GpuSampler {
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

        let id = self.resources.samplers.insert(SamplerResource {
            sampler,
            comparison: compare.is_some(),
        });

        GpuSampler { id }
    }

    pub fn create_shader(&mut self, wgsl: &str, desc: GpuShaderDesc) -> Option<GpuShader> {
        let parsed_module = parse_shader(wgsl)?;
        let bind_group_layouts = reflect_bind_group_layouts(&parsed_module)?;
        let vertex_attributes = reflect_vertex_attributes(&parsed_module)?;

        self.device.push_error_scope(wgpu::ErrorFilter::Validation);

        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aphyn shader"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });

        if let Some(err) = pollster::block_on(self.device.pop_error_scope()) {
            println!("failed to create shader: {err}");
            return None;
        }

        let id = self.resources.shaders.insert(ShaderResource {
            module,
            desc,
            bind_group_layouts,
            vertex_attributes,
        });

        Some(GpuShader { id })
    }

    pub fn write_vertex_buffer_with(
        &mut self,
        buffer: &mut GpuVertexBuffer,
        offset: usize,
        size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        self.write_buffer_resource_with(buffer.id, buffer.size(), offset, size, writer)
    }

    pub fn write_uniform_storage_buffer_with(
        &mut self,
        buffer: &mut GpuUniformStorageBuffer,
        offset: usize,
        size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        self.write_buffer_resource_with(buffer.id, buffer.size, offset, size, writer)
    }

    pub fn write_index_buffer_with(
        &mut self,
        buffer: &mut GpuIndexBuffer,
        offset: usize,
        size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        self.write_buffer_resource_with(buffer.id, buffer.size(), offset, size, writer)
    }

    fn write_buffer_resource_with(
        &mut self,
        id: VersionIndex,
        buffer_size: usize,
        offset: usize,
        mut size: usize,
        writer: impl FnOnce(&mut [u8]),
    ) -> bool {
        let Some(buffer_resource) = self.resources.buffers.get(id) else {
            return false;
        };

        if offset >= buffer_size {
            return false;
        }

        size = size.min(buffer_size - offset);

        let Some(size) = NonZeroU64::new(size as u64) else {
            return false;
        };

        let Some(mut buffer_view) = self.queue.write_buffer_with(&buffer_resource.buffer, offset as u64, size) else {
            return false;
        };

        writer(&mut buffer_view);

        true
    }

    pub fn write_texture(&mut self, texture: &mut GpuTexture, mip_level: usize, data: &[u8]) -> bool {
        let Some(texture) = self.resources.textures.get(texture.id) else {
            return false;
        };

        if mip_level >= texture.mip_level_count {
            return false;
        }

        let [width, height] = texture.size.map(|x| (x >> mip_level).max(1));
        let bytes_per_row = width * texture.format.bytes_per_pixel() as usize;

        if data.len() < bytes_per_row * height {
            return false;
        }

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture.texture,
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

    pub fn draw(&mut self, pass: &GpuRenderPass, vertices: Range<usize>, instances: Range<usize>) -> bool {
        self.encode_pass(pass, None, |render_pass| {
            render_pass.draw(to_u32_range(vertices), to_u32_range(instances));
        })
    }

    pub fn draw_indexed(
        &mut self,
        pass: &GpuRenderPass,
        index_buffer: &GpuIndexBuffer,
        indices: Range<usize>,
        instances: Range<usize>,
    ) -> bool {
        self.encode_pass(pass, Some(index_buffer), |render_pass| {
            render_pass.draw_indexed(to_u32_range(indices), 0, to_u32_range(instances));
        })
    }

    // todo: indirect draws

    fn resolve_bind_group_entries(
        resources: &RenderApiResources,
        layout: &[BindGroupLayoutEntry],
        bindings: &[(usize, GpuBinding)],
    ) -> Option<Vec<BindGroupEntry>> {
        if bindings.len() != layout.len() {
            return None;
        }

        layout
            .iter()
            .map(|&(i, ty)| {
                let (_, binding) = bindings.iter().find(|(j, _)| *j == i)?;
                let binding = binding.to_internal();

                ty.accepts(binding, resources).then_some((i, ty, binding))
            })
            .collect()
    }

    fn encode_pass(
        &mut self,
        pass: &GpuRenderPass,
        index_buffer: Option<&GpuIndexBuffer>,
        draw: impl FnOnce(&mut wgpu::RenderPass),
    ) -> bool {
        let Self {
            device,
            queue,
            cache,
            resources,
        } = self;

        let Some(shader) = resources.shaders.get(pass.shader.id) else {
            return false;
        };

        if pass.bind_groups.len() != shader.bind_group_layouts.len() {
            return false;
        }

        let vertex_layouts = pass.vertex_buffers.iter().map(|buffer| buffer.layout()).collect::<Vec<_>>();

        if !vertex_layouts_satisfy_attributes(&shader.vertex_attributes, &vertex_layouts) {
            return false;
        }

        let mut bind_groups = Vec::with_capacity(pass.bind_groups.len());

        for (layout, bindings) in shader.bind_group_layouts.iter().zip(pass.bind_groups) {
            let Some(entries) = Self::resolve_bind_group_entries(resources, layout, bindings) else {
                return false;
            };
            let Some(bind_group) = cache.get_or_create_bind_group(device, resources, &entries) else {
                return false;
            };

            bind_groups.push(bind_group);
        }

        let target = match &pass.target {
            None => None,
            Some(target) => match resources.textures.get(target.view.id) {
                Some(texture) => Some((texture, target.ops)),
                None => return false,
            },
        };
        let depth = match &pass.depth {
            None => None,
            Some(depth) => match resources.textures.get(depth.view.id) {
                Some(texture) => Some((texture, depth.ops)),
                None => return false,
            },
        };

        let pipeline_key = PipelineKey {
            shader: pass.shader.id,
            target: target.map(|(texture, _)| texture.format),
            depth: depth.map(|(texture, _)| texture.format),
            // todo: avoid cloning the layouts into the key on every draw
            vertex_buffers: vertex_layouts.iter().map(|&layout| layout.clone()).collect(),
        };

        let Some(pipeline) = cache.get_or_create_pipeline(device, resources, pipeline_key) else {
            return false;
        };

        let Some(vertex_buffers) = pass
            .vertex_buffers
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
            .map(|(texture, ops)| wgpu::RenderPassColorAttachment {
                view: &texture.texture_view,
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
        let depth_stencil_attachment = depth.map(|(texture, ops)| wgpu::RenderPassDepthStencilAttachment {
            view: &texture.texture_view,
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
            if let Some(viewport) = pass.viewport {
                render_pass.set_viewport(
                    viewport.x,
                    viewport.y,
                    viewport.width,
                    viewport.height,
                    viewport.min_depth,
                    viewport.max_depth,
                );
            }
            if let Some(scissor) = pass.scissor {
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
pub struct GpuShader {
    id: VersionIndex,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct GpuShaderDesc {
    pub topology: GpuPrimitiveTopology,
    pub front_face: GpuFrontFace,
    pub cull_mode: GpuCullMode,
    pub blend: GpuBlendMode,
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
    fn to_wgpu(self) -> wgpu::PrimitiveTopology {
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
    fn to_wgpu(self) -> wgpu::FrontFace {
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
    fn to_wgpu(self) -> Option<wgpu::Face> {
        match self {
            Self::None => None,
            Self::Front => Some(wgpu::Face::Front),
            Self::Back => Some(wgpu::Face::Back),
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuBlendMode {
    #[default]
    Replace,
    AlphaBlending,
    PremultipliedAlphaBlending,
}

impl GpuBlendMode {
    fn to_wgpu(self) -> Option<wgpu::BlendState> {
        Some(match self {
            Self::Replace => wgpu::BlendState::REPLACE,
            Self::AlphaBlending => wgpu::BlendState::ALPHA_BLENDING,
            Self::PremultipliedAlphaBlending => wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
        })
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
    fn to_wgpu(self) -> wgpu::CompareFunction {
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
    Texture(&'a GpuTexture),
    Sampler(&'a GpuSampler),
    Buffer(&'a GpuUniformStorageBuffer),
}

impl GpuBinding<'_> {
    fn to_internal(self) -> BindGroupBinding {
        match self {
            Self::Texture(texture) => BindGroupBinding::Texture(texture.id),
            Self::Sampler(sampler) => BindGroupBinding::Sampler(sampler.id),
            Self::Buffer(buffer) => BindGroupBinding::Buffer(buffer.id),
        }
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
    pub ops: GpuPassOps<[f32; 4]>,
}
#[repr(C)]
pub struct GpuRenderPassDepth<'a> {
    pub view: &'a GpuTexture,
    pub ops: GpuPassOps<f32>,
}

#[repr(C)]
pub struct GpuTexture {
    id: VersionIndex,
    format: GpuTextureFormat,
    size: [usize; 2],
}

impl GpuTexture {
    pub fn format(&self) -> GpuTextureFormat {
        self.format
    }

    pub fn size(&self) -> [usize; 2] {
        self.size
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
    fn to_wgpu(self) -> wgpu::TextureFormat {
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
}

#[repr(C)]
pub struct GpuSampler {
    id: VersionIndex,
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

        let attributes = [(0, GpuVertexBufferLayoutEntryTy::Float3), (1, GpuVertexBufferLayoutEntryTy::UInt)];

        let vertices = layout(&[(0, GpuVertexBufferLayoutEntryTy::Float3), (5, GpuVertexBufferLayoutEntryTy::Float2)]);
        let instances = layout(&[(1, GpuVertexBufferLayoutEntryTy::UInt)]);
        let wrong_type = layout(&[(1, GpuVertexBufferLayoutEntryTy::Int)]);
        let duplicate = layout(&[(0, GpuVertexBufferLayoutEntryTy::Float3), (1, GpuVertexBufferLayoutEntryTy::UInt)]);

        assert!(vertex_layouts_satisfy_attributes(&attributes, &[&vertices, &instances]));
        assert!(!vertex_layouts_satisfy_attributes(&attributes, &[&vertices]));
        assert!(!vertex_layouts_satisfy_attributes(&attributes, &[&vertices, &wrong_type]));
        assert!(!vertex_layouts_satisfy_attributes(&attributes, &[&vertices, &duplicate]));
    }
}
