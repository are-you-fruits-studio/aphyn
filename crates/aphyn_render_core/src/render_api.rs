use std::{ffi::c_void, sync::{Arc, Mutex, atomic::AtomicU64}};

use aphyn_ecs::Resource;
use aphyn_ffi::{FfiDroppable, FfiOption, FfiSliceRef, FfiStaticRef};
use wgpu::*;
use winit::window::Window;

use crate::{StandardMaterial, StandardMaterialAssetMetadata, StandardMesh, StandardShader, StandardMeshAssetMetadata, StandardTexture, StandardTextureAssetMetadata, StandardTextureFormat, StandardVertex};

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

        Self {
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
