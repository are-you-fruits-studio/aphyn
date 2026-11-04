use std::collections::HashMap;

use aphyn_utils::index_version_collection::VersionCollection;

use crate::{BindGroupEntries, BindGroupEntry, BindGroupLayoutEntries, BindGroupLayoutEntry, GpuShaderDesc, GpuTextureFormat, PipelineKey, ResourceKey, VertexAttributeEntry};


#[derive(Default)]
pub(crate) struct RenderApiState {
    pub cache: RenderApiCache,
    pub resources: RenderApiResources,
}

impl RenderApiState {
    pub fn release(&mut self, key: ResourceKey) {
        let Self { cache, resources } = self;

        match key {
            ResourceKey::Texture(id) => {
                resources.textures.remove(id);
            }
            ResourceKey::Sampler(id) => {
                resources.samplers.remove(id);
            }
            ResourceKey::Buffer(id) => {
                resources.buffers.remove(id);
            }
            ResourceKey::Shader(id) => {
                resources.shaders.remove(id);
                cache.pipelines.retain(|key, _| key.shader != id);
                return;
            }
        }

        cache
            .bind_groups
            .retain(|entries, _| !entries.entries.iter().any(|&(_, _, entry)| entry.refers_to(key)));
    }
}

#[derive(Default)]
pub(crate) struct RenderApiResources {
    pub textures: VersionCollection<TextureResource>,
    pub buffers: VersionCollection<BufferResource>,
    pub samplers: VersionCollection<SamplerResource>,
    pub shaders: VersionCollection<ShaderResource>,
}

#[derive(Default)]
pub(crate) struct RenderApiCache {
    bind_group_layouts: HashMap<BindGroupLayoutEntries, wgpu::BindGroupLayout>,
    bind_groups: HashMap<BindGroupEntries, wgpu::BindGroup>,
    pipeline_layouts: HashMap<Vec<BindGroupLayoutEntries>, wgpu::PipelineLayout>,
    pipelines: HashMap<PipelineKey, Option<wgpu::RenderPipeline>>,
}

impl RenderApiCache {
    pub fn bind_group_layouts(&self) -> &HashMap<BindGroupLayoutEntries, wgpu::BindGroupLayout> {
        &self.bind_group_layouts
    }
    pub fn bind_groups(&self) -> &HashMap<BindGroupEntries, wgpu::BindGroup> {
        &self.bind_groups
    }
    pub fn pipeline_layouts(&self) -> &HashMap<Vec<BindGroupLayoutEntries>, wgpu::PipelineLayout> {
        &self.pipeline_layouts
    }
    pub fn pipelines(&self) -> &HashMap<PipelineKey, Option<wgpu::RenderPipeline>> {
        &self.pipelines
    }

    pub fn get_or_create_bind_group_layout(
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
            .insert(BindGroupLayoutEntries::new(owned_bindings), layout.clone());

        layout
    }

    pub fn get_or_create_bind_group(
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

    pub fn get_or_create_pipeline_layout(
        &mut self,
        device: &wgpu::Device,
        groups: &[Vec<BindGroupLayoutEntry>],
    ) -> wgpu::PipelineLayout {
        let key = groups
            .iter()
            .map(|entries| BindGroupLayoutEntries::new(entries.clone()))
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

    pub fn get_or_create_pipeline(
        &mut self,
        device: &wgpu::Device,
        resources: &RenderApiResources,
        key: PipelineKey,
    ) -> Option<wgpu::RenderPipeline> {
        if let Some(pipeline) = self.pipelines.get(&key) {
            return pipeline.clone();
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
                format,
                blend: desc.blend.to_wgpu(),
                write_mask: desc.write_mask.to_wgpu(),
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
            self.pipelines.insert(key, None);
            return None;
        }

        self.pipelines.insert(key, Some(pipeline.clone()));

        Some(pipeline)
    }
}

pub(crate) struct TextureResource {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    mip_views: Vec<wgpu::TextureView>,
    format: GpuTextureFormat,
    size: [usize; 2],
    mip_level_count: usize,
}

impl TextureResource {
    pub fn new(
        texture: wgpu::Texture,
        texture_view: wgpu::TextureView,
        mip_views: Vec<wgpu::TextureView>,
        format: GpuTextureFormat,
        size: [usize; 2],
        mip_level_count: usize,
    ) -> Self {
        Self {
            texture,
            texture_view,
            mip_views,
            format,
            size,
            mip_level_count,
        }
    }

    pub fn view(&self, mip_level: Option<usize>) -> Option<&wgpu::TextureView> {
        match mip_level {
            None => Some(&self.texture_view),
            Some(mip_level) => self.mip_views.get(mip_level),
        }
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }
    pub fn mip_views(&self) -> &Vec<wgpu::TextureView> {
        &self.mip_views
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

pub(crate) struct SamplerResource {
    sampler: wgpu::Sampler,
    comparison: bool,
}

impl SamplerResource {
    pub fn new(
        sampler: wgpu::Sampler,
        comparison: bool,
    ) -> Self {
        Self {
            sampler,
            comparison,
        }
    }
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
    pub fn comparison(&self) -> bool {
        self.comparison
    }
}

pub(crate) struct ShaderResource {
    module: wgpu::ShaderModule,
    desc: GpuShaderDesc,
    bind_group_layouts: Vec<Vec<BindGroupLayoutEntry>>,
    vertex_attributes: Vec<VertexAttributeEntry>,
}

impl ShaderResource {
    pub fn new(
        module: wgpu::ShaderModule,
        desc: GpuShaderDesc,
        bind_group_layouts: Vec<Vec<BindGroupLayoutEntry>>,
        vertex_attributes: Vec<VertexAttributeEntry>,
    ) -> Self {
        Self {
            module,
            desc,
            bind_group_layouts,
            vertex_attributes,
        }
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        &self.module
    }
    pub fn desc(&self) -> &GpuShaderDesc {
        &self.desc
    }
    pub fn bind_group_layouts(&self) -> &Vec<Vec<BindGroupLayoutEntry>> {
        &self.bind_group_layouts
    }
    pub fn vertex_attributes(&self) -> &Vec<VertexAttributeEntry> {
        &self.vertex_attributes
    }
}

pub(crate) struct BufferResource {
    pub(crate) buffer: wgpu::Buffer,
}
