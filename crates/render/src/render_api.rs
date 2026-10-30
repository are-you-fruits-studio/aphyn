use std::{ffi::c_void, sync::{Arc, Mutex, atomic::AtomicU64}};

use aphyn_ecs::Resource;
use aphyn_ffi::{FfiDroppable, FfiFnMutMut, FfiOption, FfiSliceRef, FfiStaticRef};
use aphyn_math::Mat4;
use wgpu::*;
use winit::window::Window;

use crate::{
    GpuStorageBuffer, GpuUniformBuffer, GpuUniformBufferLayout, GpuVertexBuffer, GpuVertexBufferLayout, GpuVertexWriter,
    DEPTH_TEXTURE_FORMAT, RENDER_TARGET_FORMAT, StandardEngineShader, StandardGenericLight, StandardGlobalBindings, StandardInstance, StandardInstanceBuffer,
    StandardMaterial, StandardMaterialAssetMetadata, StandardMaterialShader, StandardMesh, StandardMeshAssetMetadata, StandardShader, StandardTexture,
    StandardTextureAssetMetadata, StandardTextureFormat, StandardUniformGlobal, StandardVertex, TRANSPARENT_TARGET_FORMAT,
};

// todo: ffi?

pub struct SurfaceConfigCache {
    surface_config: SurfaceConfiguration,
    size: [u32; 2],
}

pub struct RenderApi {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub window: Arc<Window>,
    // todo: ids for other assets (meshes, shaders, materials)
    pub next_texture_id: AtomicU64,
    surface_config: Mutex<SurfaceConfigCache>,
}

impl RenderApi {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let size = [size.width, size.height];

        // todo: move wgpu initialization into ecs Start handle?
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::DX12,
            ..Default::default()
        });

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            required_features: Features::empty(),
            required_limits: Limits::default(),
            label: None,
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        }))
        .unwrap();

        let surface_capabilities = surface.get_capabilities(&adapter);

        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);

        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size[0],
            height: size[1],
            present_mode: surface_capabilities.present_modes[0],
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        let surface_config = Mutex::new(SurfaceConfigCache {
            size,
            surface_config,
        });

        Self {
            device,
            queue,
            surface,
            window,
            surface_config,
            next_texture_id: AtomicU64::new(1),
        }
    }

    pub fn create_mesh(&self, vertices: &[StandardVertex], indices: &[u16], meta: Option<StandardMeshAssetMetadata>) -> StandardMesh {
        StandardMesh::new(&self.device, vertices, indices, meta)
    }
}

pub struct RenderAssets {
    pub texture_white: StandardTexture,
    pub texture_normal_default: StandardTexture,
}

impl RenderAssets {
    pub fn new(api: &RenderApi, render_data: &RenderData) -> Self {
        let texture_white = StandardTexture::new(
            api,
            render_data,
            FilterMode::Linear,
            [2, 2],
            &[255; 16],
            Default::default(),
        );
        let texture_normal_default = StandardTexture::new(
            api,
            render_data,
            FilterMode::Linear,
            [2, 2],
            &[
                128, 128, 255, 255,
                128, 128, 255, 255,
                128, 128, 255, 255,
                128, 128, 255, 255,
            ],
            StandardTextureAssetMetadata {
                raw_texture: Default::default(),
                format: StandardTextureFormat::Rgba8Unorm,
                should_generate_mipmaps: false,
            },
        );

        Self {
            texture_white,
            texture_normal_default,
        }
    }
}

pub struct RenderData {
    pub bind_group_layout_global: BindGroupLayout,
    pub bind_group_layout_material: BindGroupLayout,
    pub pipeline_layout_standard: PipelineLayout,
    pub mip_fill_bind_group_layout: BindGroupLayout,
    pub mip_fill_render_pipeline_layout: PipelineLayout,
    pub mip_fill_sampler: Sampler,
    pub mip_fill_shader: ShaderModule,
    pub engine_pipelines: StandardEnginePipelines,
}

pub struct StandardEnginePipelines {
    pub opaque_lit: RenderPipeline,
    pub opaque_unlit: RenderPipeline,
    pub transparent_lit: RenderPipeline,
    pub transparent_unlit: RenderPipeline,
    pub ui_opaque_unlit: RenderPipeline,
    pub ui_transparent_unlit: RenderPipeline,
}

impl StandardEnginePipelines {
    fn new(device: &Device, pipeline_layout_standard: &PipelineLayout) -> Self {
        let create_render_pipeline_fn = |is_lit: bool, is_transparent: bool, is_ui: bool| -> RenderPipeline {
            let defs = [is_lit.then_some("LIT"), is_transparent.then_some("TRANSPARENT"), is_ui.then_some("UI")];
            let defs = defs.into_iter().flatten().collect::<Vec<_>>();
            let shader = crate::WgslComposer::with_engine_libraries()
                .create_shader_module(device, "shader_standard.wgsl", include_str!("./assets/wgsl/shader_standard.wgsl"), &defs)
                .unwrap();

            let color_target_state = match is_transparent {
                false => ColorTargetState {
                    format: RENDER_TARGET_FORMAT,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                },
                true => ColorTargetState {
                    format: TRANSPARENT_TARGET_FORMAT,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                },
            };

            crate::create_render_pipeline(
                device,
                Some("Standard Render Pipeline"),
                pipeline_layout_standard,
                &shader,
                &[StandardVertex::desc(), StandardInstance::desc()],
                color_target_state,
                Some(DepthStencilState {
                    bias: Default::default(),
                    depth_compare: CompareFunction::LessEqual,
                    depth_write_enabled: !is_transparent,
                    format: DEPTH_TEXTURE_FORMAT,
                    stencil: Default::default(),
                }),
                PrimitiveTopology::TriangleList,
                Some(Face::Back),
            )
        };

        Self {
            opaque_unlit: create_render_pipeline_fn(false, false, false),
            opaque_lit: create_render_pipeline_fn(true, false, false),
            transparent_unlit: create_render_pipeline_fn(false, true, false),
            transparent_lit: create_render_pipeline_fn(true, true, false),
            ui_opaque_unlit: create_render_pipeline_fn(false, false, true),
            ui_transparent_unlit: create_render_pipeline_fn(false, true, true),
        }
    }

    pub fn get(&self, shader: StandardEngineShader) -> &RenderPipeline {
        match shader {
            StandardEngineShader::OpaqueLit => &self.opaque_lit,
            StandardEngineShader::OpaqueUnlit => &self.opaque_unlit,
            StandardEngineShader::TransparentLit => &self.transparent_lit,
            StandardEngineShader::TransparentUnlit => &self.transparent_unlit,
            StandardEngineShader::UiOpaqueUnlit => &self.ui_opaque_unlit,
            StandardEngineShader::UiTransparentUnlit => &self.ui_transparent_unlit,
        }
    }
}

#[repr(C)]
pub struct StandardInstancedDraw<'a> {
    pub target: &'a StandardTexture,
    pub depth: &'a StandardTexture,
    pub globals: &'a StandardGlobalBindings,
    pub material: &'a StandardMaterial,
    pub custom_shader: Option<&'a StandardShader>,
    pub mesh: &'a StandardMesh,
    pub instances: &'a StandardInstanceBuffer,
    pub instance_start: u32,
    pub instance_count: u32,
}

impl RenderData {
    pub fn new(api: &RenderApi) -> Self {
        let bind_group_layout_global = crate::create_bind_group_layout(
            &api.device,
            Some("Standard Global Bind Group Layout"),
            &[
                crate::CreateBindGroupLayoutEntry::BufferUniform,
                crate::CreateBindGroupLayoutEntry::BufferStorage,
            ]
        );
        let bind_group_layout_material = crate::create_bind_group_layout(
            &api.device,
            Some("Standard Material Bind Group Layout"),
            &[
                crate::CreateBindGroupLayoutEntry::BufferUniform,
                crate::CreateBindGroupLayoutEntry::Texture,
                crate::CreateBindGroupLayoutEntry::SamplerFiltering,
                crate::CreateBindGroupLayoutEntry::Texture,
                crate::CreateBindGroupLayoutEntry::SamplerFiltering,
                crate::CreateBindGroupLayoutEntry::Texture,
                crate::CreateBindGroupLayoutEntry::SamplerFiltering,
                crate::CreateBindGroupLayoutEntry::Texture,
                crate::CreateBindGroupLayoutEntry::SamplerFiltering,
                crate::CreateBindGroupLayoutEntry::Texture,
                crate::CreateBindGroupLayoutEntry::SamplerFiltering,
            ],
        );
        let pipeline_layout_standard = api.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Standard Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout_global, &bind_group_layout_material],
            push_constant_ranges: &[],
        });
        let mip_fill_bind_group_layout = crate::create_bind_group_layout(
            &api.device,
            Some("Mip Fill Bind Group Layout"),
            &[
                crate::CreateBindGroupLayoutEntry::Texture,
                crate::CreateBindGroupLayoutEntry::SamplerFiltering,
            ],
        );
        let mip_fill_render_pipeline_layout = api.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            bind_group_layouts: &[&mip_fill_bind_group_layout],
            label: Some("Mip Fill Render Pipeline Layout"),
            push_constant_ranges: &[],
        });
        let mip_fill_sampler = api.device.create_sampler(&SamplerDescriptor {
            label: Some("Mip Fill Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });
        
        let mip_fill_shader = crate::WgslComposer::with_engine_libraries()
            .create_shader_module(
                &api.device,
                "mip_fill_shader.wgsl",
                include_str!("./assets/mip_fill_shader.wgsl"),
                &[],
            )
            .unwrap();

        let engine_pipelines = StandardEnginePipelines::new(&api.device, &pipeline_layout_standard);

        Self {
            engine_pipelines,
            bind_group_layout_global,
            bind_group_layout_material,
            pipeline_layout_standard,
            mip_fill_bind_group_layout,
            mip_fill_render_pipeline_layout,
            mip_fill_sampler,
            mip_fill_shader,
        }
    }
}

pub struct RenderState {
    api: RenderApi,
    render_data: RenderData,
    render_assets: RenderAssets,
}

impl RenderState {
    pub fn new(window: Arc<Window>) -> Self {
        let api = RenderApi::new(window);
        let render_data = RenderData::new(&api);
        let render_assets = RenderAssets::new(&api, &render_data);

        Self {
            api,
            render_data,
            render_assets,
        }
    }

    pub fn api(&self) -> &RenderApi {
        &self.api
    }

    // todo: expose api as a struct, not deconstruct it.
    pub fn device(&self) -> &Device {
        &self.api.device
    }

    pub fn queue(&self) -> &Queue {
        &self.api.queue
    }

    pub fn surface(&self) -> &Surface<'static> {
        &self.api.surface
    }

    pub fn surface_config_format(&self) -> TextureFormat {
        self.api.surface_config.lock().unwrap().surface_config.format
    }

    pub fn window(&self) -> &Window {
        &self.api.window
    }

    pub fn size(&self) -> [u32; 2] {
        self.api.surface_config.lock().unwrap().size
    }

    pub fn render_data(&self) -> &RenderData {
        &self.render_data
    }

    pub fn render_assets(&self) -> &RenderAssets {
        &self.render_assets
    }

    //

    pub fn resize(&self, new_size: [u32; 2]) {
        if new_size[0] <= 0 || new_size[1] <= 0 {
            return;
        }

        let mut surface_config_cache = self.api.surface_config.lock().unwrap();

        surface_config_cache.size = new_size;

        surface_config_cache.surface_config.width = new_size[0];
        surface_config_cache.surface_config.height = new_size[1];

        self.api.surface.configure(&self.api.device, &surface_config_cache.surface_config);
    }

    pub fn create_texture(&self, filter_mode: FilterMode, dimensions: [u32; 2], data: &[u8], meta: StandardTextureAssetMetadata) -> StandardTexture {
        StandardTexture::new(&self.api, &self.render_data, filter_mode, dimensions, data, meta)
    }

    pub fn create_mesh(&self, vertices: &[StandardVertex], indices: &[u16], meta: Option<StandardMeshAssetMetadata>) -> StandardMesh {
        self.api.create_mesh(vertices, indices, meta)
    }

    pub fn create_material(&self, meta: StandardMaterialAssetMetadata) -> StandardMaterial {
        StandardMaterial::new(self, meta)
    }

    pub fn create_shader(&self, name: &str, source: &str) -> Option<StandardShader> {
        StandardShader::new(self, name, source)
    }

    pub fn create_instance_buffer(&self, len: u32) -> StandardInstanceBuffer {
        StandardInstanceBuffer::new(&self.api.device, len)
    }

    pub fn write_instances(&self, buffer: &StandardInstanceBuffer, start: u32, matrices: &[Mat4<f32>]) {
        buffer.write(&self.api.queue, start, matrices);
    }

    pub fn write_instances_with(&self, buffer: &StandardInstanceBuffer, start: u32, count: u32, f: impl FnMut(u32) -> Mat4<f32>) {
        buffer.write_with(&self.api.queue, start, count, f);
    }

    pub fn create_global_bindings(&self) -> StandardGlobalBindings {
        StandardGlobalBindings::new(self)
    }

    pub fn write_global_uniform(&self, bindings: &StandardGlobalBindings, uniform: StandardUniformGlobal) {
        bindings.write_uniform(self, uniform);
    }

    pub fn write_lights_with(&self, bindings: &StandardGlobalBindings, count: u32, mut f: impl FnMut(u32) -> StandardGenericLight) {
        bindings.write_lights_with(self, count, |i| f(i as u32));
    }

    pub fn create_uniform_buffer(&self, layout: GpuUniformBufferLayout) -> GpuUniformBuffer {
        GpuUniformBuffer::new(&self.api.device, layout)
    }

    pub fn write_uniform_buffer_if_dirty(&self, buffer: &mut GpuUniformBuffer) {
        buffer.write_if_dirty(&self.api.queue);
    }

    pub fn create_storage_buffer(&self, len: usize, layout: GpuUniformBufferLayout) -> GpuStorageBuffer {
        GpuStorageBuffer::new(&self.api.device, len, layout)
    }

    pub fn write_storage_buffer_if_dirty(&self, buffer: &mut GpuStorageBuffer) {
        buffer.write_if_dirty(&self.api.queue);
    }

    pub fn create_vertex_buffer(&self, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer {
        GpuVertexBuffer::new(&self.api.device, len, layout)
    }

    pub fn write_vertices_with(&self, buffer: &GpuVertexBuffer, start: usize, count: usize, f: impl FnMut(&mut GpuVertexWriter)) {
        buffer.write_with(&self.api.queue, start, count, f);
    }

    pub fn material_pipeline<'a>(&'a self, material: &StandardMaterial, custom_shader: Option<&'a StandardShader>) -> Option<&'a RenderPipeline> {
        match &material.meta().shader {
            StandardMaterialShader::Engine(shader) => Some(self.render_data.engine_pipelines.get(*shader)),
            StandardMaterialShader::Custom(_) => custom_shader.map(|shader| unsafe { &shader.native().render_pipeline }),
        }
    }

    pub fn draw_instanced(&self, draw: &StandardInstancedDraw) -> bool {
        let target = unsafe { draw.target.native() };
        let depth = unsafe { draw.depth.native() };
        let mesh = unsafe { draw.mesh.native() };

        let target_format = match draw.material.meta().shader.is_transparent() {
            false => RENDER_TARGET_FORMAT,
            true => TRANSPARENT_TARGET_FORMAT,
        };

        if mesh.indices_count == 0
            || target.texture.format() != target_format
            || depth.texture.format() != DEPTH_TEXTURE_FORMAT
        {
            return false;
        }

        let Some(render_pipeline) = self.material_pipeline(draw.material, draw.custom_shader) else {
            return false;
        };

        let [instance_start, instance_count] = draw.instances.clamp_range(draw.instance_start, draw.instance_count);

        if instance_count == 0 {
            return true;
        }

        let mut encoder = self.api.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Standard Instanced Draw Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Standard Instanced Draw Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(render_pipeline);
            render_pass.set_bind_group(0, unsafe { &draw.globals.native().bind_group }, &[]);
            render_pass.set_bind_group(1, unsafe { &draw.material.native().bind_group }, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, unsafe { draw.instances.native() }.buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint16);
            render_pass.draw_indexed(0..(mesh.indices_count as u32), 0, instance_start..(instance_start + instance_count));
        }

        self.api.queue.submit(std::iter::once(encoder.finish()));

        true
    }
}

//

#[repr(C)]
struct RenderApiVTable {
    resize_fn: unsafe extern "C-unwind" fn(*const c_void, new_size: *const u32),
    size_fn: unsafe extern "C-unwind" fn(*const c_void, size_dst: *mut u32),
    create_texture_fn: unsafe extern "C-unwind" fn(*const c_void, filter_mode: FilterMode, dimensions: *const u32, data: FfiSliceRef<u8>, meta: StandardTextureAssetMetadata) -> StandardTexture,
    create_mesh_fn: unsafe extern "C-unwind" fn(*const c_void, vertices: FfiSliceRef<StandardVertex>, indices: FfiSliceRef<u16>, meta: FfiOption<StandardMeshAssetMetadata>) -> StandardMesh,
    create_material_fn: unsafe extern "C-unwind" fn(*const c_void, meta: StandardMaterialAssetMetadata) -> StandardMaterial,
    create_shader_fn: unsafe extern "C-unwind" fn(*const c_void, name: FfiSliceRef<u8>, source: FfiSliceRef<u8>) -> FfiOption<StandardShader>,
    create_instance_buffer_fn: unsafe extern "C-unwind" fn(*const c_void, len: u32) -> StandardInstanceBuffer,
    write_instances_fn: unsafe extern "C-unwind" fn(*const c_void, buffer: &StandardInstanceBuffer, start: u32, matrices: FfiSliceRef<Mat4<f32>>),
    write_instances_with_fn: unsafe extern "C-unwind" fn(*const c_void, buffer: &StandardInstanceBuffer, start: u32, count: u32, f: FfiFnMutMut<u32, Mat4<f32>>),
    create_global_bindings_fn: unsafe extern "C-unwind" fn(*const c_void) -> StandardGlobalBindings,
    write_global_uniform_fn: unsafe extern "C-unwind" fn(*const c_void, bindings: &StandardGlobalBindings, uniform: StandardUniformGlobal),
    write_lights_with_fn: unsafe extern "C-unwind" fn(*const c_void, bindings: &StandardGlobalBindings, count: u32, f: FfiFnMutMut<u32, StandardGenericLight>),
    create_uniform_buffer_fn: unsafe extern "C-unwind" fn(*const c_void, layout: GpuUniformBufferLayout) -> GpuUniformBuffer,
    write_uniform_buffer_if_dirty_fn: unsafe extern "C-unwind" fn(*const c_void, buffer: &mut GpuUniformBuffer),
    create_storage_buffer_fn: unsafe extern "C-unwind" fn(*const c_void, len: usize, layout: GpuUniformBufferLayout) -> GpuStorageBuffer,
    write_storage_buffer_if_dirty_fn: unsafe extern "C-unwind" fn(*const c_void, buffer: &mut GpuStorageBuffer),
    create_vertex_buffer_fn: unsafe extern "C-unwind" fn(*const c_void, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer,
    write_vertices_with_fn: unsafe extern "C-unwind" fn(*const c_void, buffer: &GpuVertexBuffer, start: usize, count: usize, f: FfiFnMutMut<*mut GpuVertexWriter, ()>),
    draw_instanced_fn: unsafe extern "C-unwind" fn(*const c_void, draw: &StandardInstancedDraw) -> bool,
    clone_fn: unsafe extern "C-unwind" fn(*const c_void) -> FfiDroppable,
}

#[derive(Resource)]
#[repr(C)]
pub struct RenderApiResource {
    data: FfiDroppable,
    vtable: FfiStaticRef<RenderApiVTable>,
}

impl RenderApiResource {
    pub fn new(window: Arc<Window>) -> Self {
        unsafe extern "C-unwind" fn ffi_resize(this: *const c_void, new_size: *const u32) {
            unsafe {
                let this = &*(this as *mut Arc<RenderState>);
                let new_size = (new_size as *const [u32; 2]).read();

                this.resize(new_size);
            }
        }
        unsafe extern "C-unwind" fn ffi_size(this: *const c_void, size_dst: *mut u32) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                let size = this.size();

                (size_dst as *mut [u32; 2]).write(size);
            }
        }
        unsafe extern "C-unwind" fn ffi_create_texture(
            this: *const c_void,
            filter_mode: FilterMode,
            dimensions: *const u32,
            data: FfiSliceRef<u8>,
            meta: StandardTextureAssetMetadata,
        ) -> StandardTexture {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);
                let dimensions = (dimensions as *const [u32; 2]).read();
                let data = data.into_slice();

                this.create_texture(filter_mode, dimensions, data, meta)
            }
        }
        unsafe extern "C-unwind" fn ffi_create_mesh(
            this: *const c_void,
            vertices: FfiSliceRef<StandardVertex>,
            indices: FfiSliceRef<u16>,
            meta: FfiOption<StandardMeshAssetMetadata>,
        ) -> StandardMesh {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);
                let vertices = vertices.into_slice();
                let indices = indices.into_slice();

                this.create_mesh(vertices, indices, meta.into())
            }
        }
        unsafe extern "C-unwind" fn ffi_create_material(this: *const c_void, meta: StandardMaterialAssetMetadata) -> StandardMaterial {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.create_material(meta)
            }
        }
        unsafe extern "C-unwind" fn ffi_create_shader(
            this: *const c_void,
            name: FfiSliceRef<u8>,
            source: FfiSliceRef<u8>,
        ) -> FfiOption<StandardShader> {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);
                let name = std::str::from_utf8_unchecked(name.into_slice());
                let source = std::str::from_utf8_unchecked(source.into_slice());

                this.create_shader(name, source).into()
            }
        }
        unsafe extern "C-unwind" fn ffi_create_instance_buffer(this: *const c_void, len: u32) -> StandardInstanceBuffer {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.create_instance_buffer(len)
            }
        }
        unsafe extern "C-unwind" fn ffi_write_instances(
            this: *const c_void,
            buffer: &StandardInstanceBuffer,
            start: u32,
            matrices: FfiSliceRef<Mat4<f32>>,
        ) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.write_instances(buffer, start, matrices.into_slice())
            }
        }
        unsafe extern "C-unwind" fn ffi_write_instances_with(
            this: *const c_void,
            buffer: &StandardInstanceBuffer,
            start: u32,
            count: u32,
            mut f: FfiFnMutMut<u32, Mat4<f32>>,
        ) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.write_instances_with(buffer, start, count, |i| f.execute(i))
            }
        }
        unsafe extern "C-unwind" fn ffi_create_global_bindings(this: *const c_void) -> StandardGlobalBindings {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.create_global_bindings()
            }
        }
        unsafe extern "C-unwind" fn ffi_write_global_uniform(
            this: *const c_void,
            bindings: &StandardGlobalBindings,
            uniform: StandardUniformGlobal,
        ) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.write_global_uniform(bindings, uniform)
            }
        }
        unsafe extern "C-unwind" fn ffi_write_lights_with(
            this: *const c_void,
            bindings: &StandardGlobalBindings,
            count: u32,
            mut f: FfiFnMutMut<u32, StandardGenericLight>,
        ) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.write_lights_with(bindings, count, |i| f.execute(i))
            }
        }
        unsafe extern "C-unwind" fn ffi_create_uniform_buffer(this: *const c_void, layout: GpuUniformBufferLayout) -> GpuUniformBuffer {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.create_uniform_buffer(layout)
            }
        }
        unsafe extern "C-unwind" fn ffi_write_uniform_buffer_if_dirty(this: *const c_void, buffer: &mut GpuUniformBuffer) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.write_uniform_buffer_if_dirty(buffer)
            }
        }
        unsafe extern "C-unwind" fn ffi_create_storage_buffer(this: *const c_void, len: usize, layout: GpuUniformBufferLayout) -> GpuStorageBuffer {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.create_storage_buffer(len, layout)
            }
        }
        unsafe extern "C-unwind" fn ffi_write_storage_buffer_if_dirty(this: *const c_void, buffer: &mut GpuStorageBuffer) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.write_storage_buffer_if_dirty(buffer)
            }
        }
        unsafe extern "C-unwind" fn ffi_create_vertex_buffer(this: *const c_void, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.create_vertex_buffer(len, layout)
            }
        }
        unsafe extern "C-unwind" fn ffi_write_vertices_with(
            this: *const c_void,
            buffer: &GpuVertexBuffer,
            start: usize,
            count: usize,
            mut f: FfiFnMutMut<*mut GpuVertexWriter, ()>,
        ) {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.write_vertices_with(buffer, start, count, |writer| f.execute(writer))
            }
        }
        unsafe extern "C-unwind" fn ffi_draw_instanced(this: *const c_void, draw: &StandardInstancedDraw) -> bool {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                this.draw_instanced(draw)
            }
        }
        unsafe extern "C-unwind" fn ffi_clone(this: *const c_void) -> FfiDroppable {
            unsafe {
                let this = &*(this as *const Arc<RenderState>);

                FfiDroppable::new(Arc::clone(this))
            }
        }

        Self {
            data: FfiDroppable::new(Arc::new(RenderState::new(window))),
            vtable: FfiStaticRef::new(&RenderApiVTable {
                resize_fn: ffi_resize,
                size_fn: ffi_size,
                create_texture_fn: ffi_create_texture,
                create_mesh_fn: ffi_create_mesh,
                create_material_fn: ffi_create_material,
                create_shader_fn: ffi_create_shader,
                create_instance_buffer_fn: ffi_create_instance_buffer,
                write_instances_fn: ffi_write_instances,
                write_instances_with_fn: ffi_write_instances_with,
                create_global_bindings_fn: ffi_create_global_bindings,
                write_global_uniform_fn: ffi_write_global_uniform,
                write_lights_with_fn: ffi_write_lights_with,
                create_uniform_buffer_fn: ffi_create_uniform_buffer,
                write_uniform_buffer_if_dirty_fn: ffi_write_uniform_buffer_if_dirty,
                create_storage_buffer_fn: ffi_create_storage_buffer,
                write_storage_buffer_if_dirty_fn: ffi_write_storage_buffer_if_dirty,
                create_vertex_buffer_fn: ffi_create_vertex_buffer,
                write_vertices_with_fn: ffi_write_vertices_with,
                draw_instanced_fn: ffi_draw_instanced,
                clone_fn: ffi_clone,
            }),
        }
    }

    pub fn resize(&self, new_size: [u32; 2]) {
        unsafe {
            let this = self.data.get();
            let new_size = &raw const new_size as *const u32;

            (self.vtable.resize_fn)(this, new_size)
        }
    }

    pub fn size(&self) -> [u32; 2] {
        unsafe {
            let this = self.data.get();
            let mut size = [0_u32; 2];
            let size_dst = &raw mut size as *mut u32;

            (self.vtable.size_fn)(this, size_dst);

            size
        }
    }

    pub fn create_texture(&self, filter_mode: FilterMode, dimensions: [u32; 2], data: &[u8], meta: StandardTextureAssetMetadata) -> StandardTexture {
        unsafe {
            let this = self.data.get();
            let dimensions = &raw const dimensions as *const u32;
            let data = FfiSliceRef::from_slice(data);

            let result = (self.vtable.create_texture_fn)(this, filter_mode, dimensions, data, meta.into());

            result
        }
    }

    pub fn create_mesh(&self, vertices: &[StandardVertex], indices: &[u16], meta: Option<StandardMeshAssetMetadata>) -> StandardMesh {
        unsafe {
            let this = self.data.get();
            let vertices = FfiSliceRef::from_slice(vertices);
            let indices = FfiSliceRef::from_slice(indices);

            (self.vtable.create_mesh_fn)(this, vertices, indices, meta.into())
        }
    }

    pub fn create_material(&self, meta: StandardMaterialAssetMetadata) -> StandardMaterial {
        unsafe {
            let this = self.data.get();

            (self.vtable.create_material_fn)(this, meta)
        }
    }

    pub fn create_shader(&self, name: &str, source: &str) -> Option<StandardShader> {
        unsafe {
            let this = self.data.get();
            let name = FfiSliceRef::from_slice(name.as_bytes());
            let source = FfiSliceRef::from_slice(source.as_bytes());

            (self.vtable.create_shader_fn)(this, name, source).into_option()
        }
    }

    pub fn create_instance_buffer(&self, len: u32) -> StandardInstanceBuffer {
        unsafe { (self.vtable.create_instance_buffer_fn)(self.data.get(), len) }
    }

    pub fn write_instances(&self, buffer: &StandardInstanceBuffer, start: u32, matrices: &[Mat4<f32>]) {
        unsafe { (self.vtable.write_instances_fn)(self.data.get(), buffer, start, FfiSliceRef::from_slice(matrices)) }
    }

    pub fn write_instances_with(&self, buffer: &StandardInstanceBuffer, start: u32, count: u32, mut f: impl FnMut(u32) -> Mat4<f32>) {
        unsafe { (self.vtable.write_instances_with_fn)(self.data.get(), buffer, start, count, FfiFnMutMut::new(&mut f)) }
    }

    pub fn create_global_bindings(&self) -> StandardGlobalBindings {
        unsafe { (self.vtable.create_global_bindings_fn)(self.data.get()) }
    }

    pub fn write_global_uniform(&self, bindings: &StandardGlobalBindings, uniform: StandardUniformGlobal) {
        unsafe { (self.vtable.write_global_uniform_fn)(self.data.get(), bindings, uniform) }
    }

    pub fn write_lights_with(&self, bindings: &StandardGlobalBindings, count: u32, mut f: impl FnMut(u32) -> StandardGenericLight) {
        unsafe { (self.vtable.write_lights_with_fn)(self.data.get(), bindings, count, FfiFnMutMut::new(&mut f)) }
    }

    pub fn create_uniform_buffer(&self, layout: GpuUniformBufferLayout) -> GpuUniformBuffer {
        unsafe { (self.vtable.create_uniform_buffer_fn)(self.data.get(), layout) }
    }

    pub fn write_uniform_buffer_if_dirty(&self, buffer: &mut GpuUniformBuffer) {
        if buffer.is_dirty() {
            unsafe { (self.vtable.write_uniform_buffer_if_dirty_fn)(self.data.get(), buffer) }
        }
    }

    pub fn create_storage_buffer(&self, len: usize, layout: GpuUniformBufferLayout) -> GpuStorageBuffer {
        unsafe { (self.vtable.create_storage_buffer_fn)(self.data.get(), len, layout) }
    }

    pub fn write_storage_buffer_if_dirty(&self, buffer: &mut GpuStorageBuffer) {
        if buffer.is_dirty() {
            unsafe { (self.vtable.write_storage_buffer_if_dirty_fn)(self.data.get(), buffer) }
        }
    }

    pub fn create_vertex_buffer(&self, len: usize, layout: GpuVertexBufferLayout) -> GpuVertexBuffer {
        unsafe { (self.vtable.create_vertex_buffer_fn)(self.data.get(), len, layout) }
    }

    pub fn write_vertices_with(&self, buffer: &GpuVertexBuffer, start: usize, count: usize, mut f: impl FnMut(&mut GpuVertexWriter)) {
        let mut f = |writer: *mut GpuVertexWriter| f(unsafe { &mut *writer });
        unsafe { (self.vtable.write_vertices_with_fn)(self.data.get(), buffer, start, count, FfiFnMutMut::new(&mut f)) }
    }

    pub fn draw_instanced(&self, draw: &StandardInstancedDraw) -> bool {
        unsafe { (self.vtable.draw_instanced_fn)(self.data.get(), draw) }
    }

    pub fn clone(&self) -> Self {
        unsafe {
            Self {
                data: (self.vtable.clone_fn)(self.data.get()),
                vtable: self.vtable,
            }
        }
    }

    pub unsafe fn raw(&self) -> Arc<RenderState> {
        unsafe { &*(self.data.get() as *mut Arc<RenderState>) }.clone()
    }
}

unsafe impl Send for RenderApiResource {}
unsafe impl Sync for RenderApiResource {}
