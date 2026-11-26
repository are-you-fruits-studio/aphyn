use aphyn::*;

const RENDER_ORDER_TRANSFORM_GIZMO: u64 = RENDER_ORDER_GIZMOS + 50_000;

// lives in the simulated world only while the scene view renders, which keeps the gizmo out of the game view
#[derive(Resource, Default)]
pub struct TransformGizmoMeshResource {
    pub vertices: FfiVec<[f32; 10]>,
    pub indices: FfiVec<u32>,
}

struct TransformGizmoGpu {
    shader: GpuShader,
    uniform_buffer: GpuUniformStorageBuffer,
    vertex_buffer: Option<GpuVertexBuffer>,
    index_buffer: Option<GpuIndexBuffer>,
    depth: Option<GpuTexture>,
}

#[derive(Default)]
pub struct TransformGizmoRenderFeature {
    gpu: Option<TransformGizmoGpu>,
}

impl RenderFeature for TransformGizmoRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        let has_mesh = world
            .resources()
            .get::<TransformGizmoMeshResource>()
            .is_some_and(|mesh| !mesh.indices.is_empty());

        if has_mesh {
            passes.add(RENDER_ORDER_TRANSFORM_GIZMO, |this, ctx| this.draw(ctx));
        }
    }
}

impl TransformGizmoRenderFeature {
    fn draw(&mut self, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let resources = ctx.world.resources();

        let (Some(mesh), Some(current)) = (
            resources.get::<TransformGizmoMeshResource>(),
            resources.get::<CurrentRenderingResource>(),
        ) else {
            return;
        };

        let gpu = self.gpu.get_or_insert_with(|| TransformGizmoGpu {
            shader: core
                .create_shader(
                    Some("transform gizmo"),
                    include_str!("./shader_transform_gizmo.wgsl"),
                    GpuShaderDesc {
                        cull_mode: GpuCullMode::None,
                        ..Default::default()
                    },
                )
                .unwrap(),
            uniform_buffer: core.create_uniform_storage_buffer(Some("transform gizmo uniforms"), size_of::<[Vec4<f32>; 5]>()),
            vertex_buffer: None,
            index_buffer: None,
            depth: None,
        });

        let vertices_count = mesh.vertices.len() as usize;
        let indices_count = mesh.indices.len() as usize;

        if gpu.vertex_buffer.as_ref().is_none_or(|buffer| buffer.len() < vertices_count) {
            let layout = GpuVertexBufferLayout::new(
                GpuVertexStepMode::Vertex,
                [
                    (0, GpuVertexBufferLayoutEntryTy::Float3),
                    (1, GpuVertexBufferLayoutEntryTy::Float3),
                    (2, GpuVertexBufferLayoutEntryTy::Float4),
                ],
            )
            .unwrap();

            gpu.vertex_buffer =
                Some(core.create_vertex_buffer(Some("transform gizmo vertices"), vertices_count.next_power_of_two(), layout));
        }

        if gpu.index_buffer.as_ref().is_none_or(|buffer| buffer.len() < indices_count) {
            gpu.index_buffer = Some(core.create_index_buffer(Some("transform gizmo indices"), indices_count.next_power_of_two()));
        }

        let target = current.target.texture();
        let target_size = target.size();

        if gpu.depth.as_ref().is_none_or(|depth| depth.size() != target_size) {
            gpu.depth = Some(core.create_texture(Some("transform gizmo depth"), target_size, GpuTextureFormat::Depth32Float, 1));
        }

        let (Some(vertex_buffer), Some(index_buffer), Some(depth)) = (&gpu.vertex_buffer, &gpu.index_buffer, &gpu.depth) else {
            return;
        };

        let world_to_clip = current.world_to_clip();
        let camera_position = current.camera_position;

        write_buffer_elements_with(core, &gpu.uniform_buffer, 0, 1, |_| {
            let [c0, c1, c2, c3] = *world_to_clip.as_array();
            [c0, c1, c2, c3, [camera_position.x, camera_position.y, camera_position.z, 1.0]]
        });
        write_buffer_elements_with(core, vertex_buffer, 0, vertices_count, |i| mesh.vertices[i as u64]);
        write_buffer_elements_with(core, index_buffer, 0, indices_count, |i| mesh.indices[i as u64]);

        core.draw_indexed(
            &GpuRenderPass {
                label: Some("transform gizmo"),
                shader: &gpu.shader,
                target: Some(GpuRenderPassTarget {
                    view: target,
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                depth: Some(GpuRenderPassDepth {
                    view: depth,
                    mip_level: 0,
                    ops: GpuPassOps::Clear(0.0),
                }),
                bind_groups: &[&[(0, GpuBinding::Buffer(&gpu.uniform_buffer))]],
                vertex_buffers: &[vertex_buffer],
                viewport: None,
                scissor: None,
            },
            index_buffer,
            0..indices_count,
            0..1,
        );
    }
}
