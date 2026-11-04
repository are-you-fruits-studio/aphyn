use std::sync::Arc;

use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::Resource;
use aphyn_math::Mat4;
use aphyn_render_core::{
    GpuBinding, GpuBlendComponent, GpuBlendFactor, GpuBlendOperation, GpuBlendState, GpuColorWrites, GpuCompareFunction,
    GpuCullMode, GpuFrontFace, GpuPassOps, GpuPrimitiveTopology, GpuRenderPass, GpuRenderPassDepth, GpuRenderPassTarget, GpuSampler,
    GpuShader, GpuShaderDesc, GpuVertexBuffer, RenderApi,
};
use winit::window::Window;

use crate::{
    DEPTH_TEXTURE_FORMAT, FilterMode, GpuVertexBufferLayout, RENDER_TARGET_FORMAT, StandardEngineShader, StandardGenericLight,
    StandardGlobalBindings, StandardInstanceBuffer, StandardMaterial, StandardMaterialAssetMetadata, StandardMaterialShader,
    StandardMesh, StandardMeshAssetMetadata, StandardShader, StandardTexture, StandardTextureAssetMetadata, StandardTextureFormat,
    StandardUniformGlobal, StandardVertex, TRANSPARENT_TARGET_FORMAT, TypedStorageBuffer, TypedUniformBuffer,
    TypedUniformBufferLayout, TypedVertexBuffer, TypedVertexWriter,
};

pub fn standard_shader_desc(is_transparent: bool) -> GpuShaderDesc {
    let blend = match is_transparent {
        false => GpuBlendState::REPLACE,
        true => GpuBlendState {
            color: GpuBlendComponent {
                src_factor: GpuBlendFactor::SrcAlpha,
                dst_factor: GpuBlendFactor::One,
                operation: GpuBlendOperation::Add,
            },
            alpha: GpuBlendComponent {
                src_factor: GpuBlendFactor::One,
                dst_factor: GpuBlendFactor::One,
                operation: GpuBlendOperation::Add,
            },
        },
    };

    GpuShaderDesc {
        topology: GpuPrimitiveTopology::TriangleList,
        front_face: GpuFrontFace::Ccw,
        cull_mode: GpuCullMode::Back,
        blend,
        write_mask: GpuColorWrites::All,
        depth_write: !is_transparent,
        depth_compare: GpuCompareFunction::LessEqual,
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct StandardEngineShaders {
    pub opaque_lit: GpuShader,
    pub opaque_unlit: GpuShader,
    pub transparent_lit: GpuShader,
    pub transparent_unlit: GpuShader,
    pub ui_opaque_unlit: GpuShader,
    pub ui_transparent_unlit: GpuShader,
}

impl StandardEngineShaders {
    fn new(core: &RenderApi) -> Self {
        let create_shader = |is_lit: bool, is_transparent: bool, is_ui: bool| -> GpuShader {
            let defs = [
                is_lit.then_some("LIT"),
                is_transparent.then_some("TRANSPARENT"),
                is_ui.then_some("UI"),
            ];
            let defs = defs.into_iter().flatten().collect::<Vec<_>>();
            let code = crate::WgslComposer::with_engine_libraries()
                .compose(
                    "shader_standard.wgsl",
                    include_str!("./assets/wgsl/shader_standard.wgsl"),
                    &defs,
                )
                .unwrap();
            let label = format!("shader_standard.wgsl {defs:?}");

            core.create_shader(Some(&label), &code, standard_shader_desc(is_transparent))
                .unwrap()
        };

        Self {
            opaque_unlit: create_shader(false, false, false),
            opaque_lit: create_shader(true, false, false),
            transparent_unlit: create_shader(false, true, false),
            transparent_lit: create_shader(true, true, false),
            ui_opaque_unlit: create_shader(false, false, true),
            ui_transparent_unlit: create_shader(false, true, true),
        }
    }

    pub fn get(&self, shader: StandardEngineShader) -> &GpuShader {
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
    pub textures: &'a AssetStorageResource<StandardTexture>,
    pub mesh: &'a StandardMesh,
    pub instances: &'a StandardInstanceBuffer,
    pub instance_start: u32,
    pub instance_count: u32,
}

#[repr(C)]
pub struct StandardBatchedDraw<'a> {
    pub target: &'a StandardTexture,
    pub depth: &'a StandardTexture,
    pub globals: &'a StandardGlobalBindings,
    pub material: &'a StandardMaterial,
    pub custom_shader: Option<&'a StandardShader>,
    pub textures: &'a AssetStorageResource<StandardTexture>,
    pub vertices: &'a GpuVertexBuffer,
    pub vertex_count: usize,
    pub identity_instance: &'a StandardInstanceBuffer,
}

#[derive(Resource)]
#[repr(C)]
pub struct RenderApiResource {
    core: RenderApi,
    engine_shaders: StandardEngineShaders,
    mip_fill_shader: GpuShader,
    mip_fill_sampler: GpuSampler,
    texture_white: StandardTexture,
    texture_normal_default: StandardTexture,
}

impl RenderApiResource {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let size = [size.width as usize, size.height as usize];

        let core = RenderApi::new(window, size).expect("failed to initialize the GPU");

        let engine_shaders = StandardEngineShaders::new(&core);

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

        let texture_white = StandardTexture::new(
            &core,
            &mip_fill_shader,
            &mip_fill_sampler,
            FilterMode::Linear,
            [2, 2],
            &[255; 16],
            Default::default(),
        );
        #[rustfmt::skip]
        let texture_normal_default = StandardTexture::new(
            &core,
            &mip_fill_shader,
            &mip_fill_sampler,
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
            core,
            engine_shaders,
            mip_fill_shader,
            mip_fill_sampler,
            texture_white,
            texture_normal_default,
        }
    }

    pub fn core(&self) -> &RenderApi {
        &self.core
    }

    pub fn engine_shaders(&self) -> &StandardEngineShaders {
        &self.engine_shaders
    }

    pub fn texture_white(&self) -> &StandardTexture {
        &self.texture_white
    }

    pub fn texture_normal_default(&self) -> &StandardTexture {
        &self.texture_normal_default
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
        indices: &[u16],
        meta: Option<StandardMeshAssetMetadata>,
    ) -> StandardMesh {
        StandardMesh::new(&self.core, vertices, indices, meta)
    }

    pub fn create_material(&self, meta: StandardMaterialAssetMetadata) -> StandardMaterial {
        StandardMaterial::new(&self.core, meta)
    }

    pub fn create_shader(&self, name: &str, source: &str) -> Option<StandardShader> {
        StandardShader::new(&self.core, name, source)
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

    pub fn material_shader<'a>(
        &'a self,
        material: &StandardMaterial,
        custom_shader: Option<&'a StandardShader>,
    ) -> Option<&'a GpuShader> {
        match &material.meta().shader {
            StandardMaterialShader::Engine(shader) => Some(self.engine_shaders.get(*shader)),
            StandardMaterialShader::Custom(_) => custom_shader.map(StandardShader::shader),
        }
    }

    pub fn material_textures<'a>(
        &'a self,
        material: &StandardMaterial,
        textures: &'a AssetStorageResource<StandardTexture>,
    ) -> [&'a StandardTexture; 5] {
        material.resolve_textures(textures, &self.texture_white, &self.texture_normal_default)
    }

    pub fn draw_instanced(&self, draw: &StandardInstancedDraw) -> bool {
        let mesh = draw.mesh;

        if mesh.indices_count() == 0 || !are_standard_targets_valid(draw.material, draw.target, draw.depth) {
            return false;
        }

        let Some(shader) = self.material_shader(draw.material, draw.custom_shader) else {
            return false;
        };

        let [instance_start, instance_count] = draw.instances.clamp_range(draw.instance_start, draw.instance_count);

        if instance_count == 0 {
            return true;
        }

        let textures = self.material_textures(draw.material, draw.textures);
        let globals_group = globals_bind_group(draw.globals);
        let material_group = material_bind_group(draw.material, textures);

        self.core.draw_indexed(
            &standard_pass(
                shader,
                draw.target,
                draw.depth,
                &[&globals_group, &material_group],
                &[mesh.vertex_buffer(), draw.instances.buffer()],
            ),
            mesh.index_buffer(),
            0..mesh.indices_count(),
            (instance_start as usize)..((instance_start + instance_count) as usize),
        )
    }

    pub fn draw_batched(&self, draw: &StandardBatchedDraw) -> bool {
        if !are_standard_targets_valid(draw.material, draw.target, draw.depth) {
            return false;
        }

        let Some(shader) = self.material_shader(draw.material, draw.custom_shader) else {
            return false;
        };

        let vertex_count = draw.vertex_count.min(draw.vertices.len());

        if vertex_count == 0 {
            return true;
        }

        let textures = self.material_textures(draw.material, draw.textures);
        let globals_group = globals_bind_group(draw.globals);
        let material_group = material_bind_group(draw.material, textures);

        self.core.draw(
            &standard_pass(
                shader,
                draw.target,
                draw.depth,
                &[&globals_group, &material_group],
                &[draw.vertices, draw.identity_instance.buffer()],
            ),
            0..vertex_count,
            0..1,
        )
    }

    pub fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            engine_shaders: self.engine_shaders.clone(),
            mip_fill_shader: self.mip_fill_shader.clone(),
            mip_fill_sampler: self.mip_fill_sampler.clone(),
            texture_white: self.texture_white.clone(),
            texture_normal_default: self.texture_normal_default.clone(),
        }
    }
}

fn are_standard_targets_valid(material: &StandardMaterial, target: &StandardTexture, depth: &StandardTexture) -> bool {
    let target_format = match material.meta().shader.is_transparent() {
        false => RENDER_TARGET_FORMAT,
        true => TRANSPARENT_TARGET_FORMAT,
    };

    target.texture().format() == target_format && depth.texture().format() == DEPTH_TEXTURE_FORMAT
}

fn globals_bind_group(globals: &StandardGlobalBindings) -> [(usize, GpuBinding<'_>); 2] {
    [
        (0, GpuBinding::Buffer(globals.uniform_buffer())),
        (1, GpuBinding::Buffer(globals.lights_buffer())),
    ]
}

// binding indices follow `standard_bindings.wgsl`
fn material_bind_group<'a>(material: &'a StandardMaterial, textures: [&'a StandardTexture; 5]) -> [(usize, GpuBinding<'a>); 11] {
    let texture = |t: &'a StandardTexture| GpuBinding::Texture {
        texture: t.texture(),
        mip_level: None,
    };
    let sampler = |t: &'a StandardTexture| GpuBinding::Sampler(t.sampler());
    let [color, roughness, metallic, normal, emission] = textures;

    [
        (0, GpuBinding::Buffer(material.uniform_buffer())),
        (1, texture(color)),
        (2, sampler(color)),
        (3, texture(roughness)),
        (4, sampler(roughness)),
        (5, texture(metallic)),
        (6, sampler(metallic)),
        (7, texture(normal)),
        (8, sampler(normal)),
        (9, texture(emission)),
        (10, sampler(emission)),
    ]
}

fn standard_pass<'a>(
    shader: &'a GpuShader,
    target: &'a StandardTexture,
    depth: &'a StandardTexture,
    bind_groups: &'a [&'a [(usize, GpuBinding<'a>)]],
    vertex_buffers: &'a [&'a GpuVertexBuffer],
) -> GpuRenderPass<'a> {
    GpuRenderPass {
        shader,
        target: Some(GpuRenderPassTarget {
            view: target.texture(),
            mip_level: 0,
            ops: GpuPassOps::Load,
        }),
        depth: Some(GpuRenderPassDepth {
            view: depth.texture(),
            mip_level: 0,
            ops: GpuPassOps::Load,
        }),
        bind_groups,
        vertex_buffers,
        viewport: None,
        scissor: None,
    }
}
