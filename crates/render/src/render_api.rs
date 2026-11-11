use std::sync::Arc;

use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::Resource;
use aphyn_math::Mat4;
use aphyn_render_core::{GpuCullMode, GpuSampler, GpuShader, GpuShaderDesc, RenderApi};
use winit::window::Window;

use crate::{
    FilterMode, GpuVertexBufferLayout, Material, MaterialData, MaterialGpuData, MaterialTemplate, MaterialTextureFallback, Shader,
    ShaderAssetMetadata, StandardGenericLight, StandardGlobalBindings, StandardInstanceBuffer, StandardMesh,
    StandardMeshAssetMetadata, StandardTexture, StandardTextureAssetMetadata, StandardTextureFormat, StandardUniformGlobal,
    StandardVertex, TypedStorageBuffer, TypedUniformBuffer, TypedUniformBufferLayout, TypedVertexBuffer, TypedVertexWriter,
};

#[derive(Resource)]
#[repr(C)]
pub struct RenderApiResource {
    core: RenderApi,
    mip_fill_shader: GpuShader,
    mip_fill_sampler: GpuSampler,
    texture_white: StandardTexture,
    texture_black: StandardTexture,
    texture_normal_default: StandardTexture,
}

impl RenderApiResource {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let size = [size.width as usize, size.height as usize];

        let core = RenderApi::new(window, size).expect("failed to initialize the GPU");

        let mip_fill_code = crate::WgslComposer::with_engine_libraries()
            .compose("mip_fill_shader.wgsl", include_str!("./assets/mip_fill_shader.wgsl"), &[])
            .unwrap();
        let mip_fill_shader = core
            .create_shader(
                Some("mip_fill_shader.wgsl"),
                &mip_fill_code,
                GpuShaderDesc {
                    cull_mode: GpuCullMode::None,
                    ..Default::default()
                },
            )
            .unwrap();
        let mip_fill_sampler = core.create_sampler(Some("mip fill"), FilterMode::Linear);

        let fallback_texture = |pixel: [u8; 4], format: StandardTextureFormat| {
            StandardTexture::new(
                &core,
                &mip_fill_shader,
                &mip_fill_sampler,
                FilterMode::Linear,
                [2, 2],
                &pixel.repeat(4),
                StandardTextureAssetMetadata {
                    format,
                    ..Default::default()
                },
            )
        };

        let texture_white = fallback_texture([255; 4], StandardTextureFormat::default());
        let texture_black = fallback_texture([0, 0, 0, 255], StandardTextureFormat::default());
        let texture_normal_default = fallback_texture([128, 128, 255, 255], StandardTextureFormat::Rgba8Unorm);

        Self {
            core,
            mip_fill_shader,
            mip_fill_sampler,
            texture_white,
            texture_black,
            texture_normal_default,
        }
    }

    pub fn core(&self) -> &RenderApi {
        &self.core
    }

    pub fn texture_white(&self) -> &StandardTexture {
        &self.texture_white
    }

    pub fn texture_black(&self) -> &StandardTexture {
        &self.texture_black
    }

    pub fn texture_normal_default(&self) -> &StandardTexture {
        &self.texture_normal_default
    }

    pub fn fallback_texture(&self, fallback: MaterialTextureFallback) -> &StandardTexture {
        match fallback {
            MaterialTextureFallback::White => &self.texture_white,
            MaterialTextureFallback::Black => &self.texture_black,
            MaterialTextureFallback::FlatNormal => &self.texture_normal_default,
        }
    }

    pub fn resize(&self, new_size: [u32; 2]) {
        self.core.resize_surface(new_size.map(|x| x as usize));
    }

    pub fn size(&self) -> [u32; 2] {
        self.core.surface_size().map(|x| x as u32)
    }

    pub fn create_texture(
        &self,
        filter_mode: FilterMode,
        dimensions: [u32; 2],
        data: &[u8],
        meta: StandardTextureAssetMetadata,
    ) -> StandardTexture {
        StandardTexture::new(
            &self.core,
            &self.mip_fill_shader,
            &self.mip_fill_sampler,
            filter_mode,
            dimensions,
            data,
            meta,
        )
    }

    pub fn create_mesh(
        &self,
        vertices: &[StandardVertex],
        indices: &[u32],
        meta: Option<StandardMeshAssetMetadata>,
    ) -> StandardMesh {
        StandardMesh::new(&self.core, vertices, indices, meta)
    }

    pub fn create_shader(
        &self,
        label: &str,
        composed_wgsl: &str,
        desc: GpuShaderDesc,
        meta: Option<ShaderAssetMetadata>,
    ) -> Option<Shader> {
        Shader::new(&self.core, label, composed_wgsl, desc, meta)
    }

    pub fn create_material(
        &self,
        templates: &AssetStorageResource<MaterialTemplate>,
        shaders: &AssetStorageResource<Shader>,
        template: AssetHandle<MaterialTemplate>,
        data: MaterialData,
    ) -> Material {
        let mut material = Material::new(template, data);
        self.rebuild_material_gpu(templates, shaders, &mut material);

        material
    }

    // todo: rebuild the materials of a template or shader when that asset is reloaded
    pub fn rebuild_material_gpu(
        &self,
        templates: &AssetStorageResource<MaterialTemplate>,
        shaders: &AssetStorageResource<Shader>,
        material: &mut Material,
    ) {
        let Some(template) = templates.get(&material.template) else {
            material.gpu = MaterialGpuData::default();
            return;
        };

        material.data.reconcile(template);
        material.gpu = MaterialGpuData::build(&self.core, template, shaders, &material.data);
    }

    pub fn upload_material_changes(&self, material: &mut Material) {
        let Material { data, gpu, .. } = material;
        gpu.upload_changes(&self.core, data);
    }

    pub fn create_instance_buffer(&self, len: u32) -> StandardInstanceBuffer {
        StandardInstanceBuffer::new(&self.core, len)
    }

    pub fn write_instances(&self, buffer: &StandardInstanceBuffer, start: u32, matrices: &[Mat4<f32>]) {
        buffer.write(&self.core, start, matrices);
    }

    pub fn write_instances_with(&self, buffer: &StandardInstanceBuffer, start: u32, count: u32, f: impl FnMut(u32) -> Mat4<f32>) {
        buffer.write_with(&self.core, start, count, f);
    }

    pub fn create_global_bindings(&self) -> StandardGlobalBindings {
        StandardGlobalBindings::new(&self.core)
    }

    pub fn write_global_uniform(&self, bindings: &StandardGlobalBindings, uniform: StandardUniformGlobal) {
        bindings.write_uniform(&self.core, uniform);
    }

    pub fn write_lights_with(&self, bindings: &StandardGlobalBindings, count: u32, mut f: impl FnMut(u32) -> StandardGenericLight) {
        bindings.write_lights_with(&self.core, count, |i| f(i as u32));
    }

    pub fn create_uniform_buffer(&self, layout: TypedUniformBufferLayout) -> TypedUniformBuffer {
        TypedUniformBuffer::new(&self.core, layout)
    }

    pub fn write_uniform_buffer_if_dirty(&self, buffer: &mut TypedUniformBuffer) {
        buffer.write_if_dirty(&self.core);
    }

    pub fn create_storage_buffer(&self, len: usize, layout: TypedUniformBufferLayout) -> TypedStorageBuffer {
        TypedStorageBuffer::new(&self.core, len, layout)
    }

    pub fn write_storage_buffer_if_dirty(&self, buffer: &mut TypedStorageBuffer) {
        buffer.write_if_dirty(&self.core);
    }

    pub fn create_vertex_buffer(&self, len: usize, layout: GpuVertexBufferLayout) -> TypedVertexBuffer {
        TypedVertexBuffer::new(&self.core, len, layout)
    }

    pub fn write_vertices_with(
        &self,
        buffer: &TypedVertexBuffer,
        start: usize,
        count: usize,
        f: impl FnMut(&mut TypedVertexWriter),
    ) {
        buffer.write_with(&self.core, start, count, f);
    }

    pub fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            mip_fill_shader: self.mip_fill_shader.clone(),
            mip_fill_sampler: self.mip_fill_sampler.clone(),
            texture_white: self.texture_white.clone(),
            texture_black: self.texture_black.clone(),
            texture_normal_default: self.texture_normal_default.clone(),
        }
    }
}
