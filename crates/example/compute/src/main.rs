use aphyn::*;

const SNOWFLAKE_COUNT: usize = 50_000;

const SNOWFLAKE_MESH: &str = "snowflake.asset";
const SNOWFLAKE_DRAW_SHADER: &str = "snowflake_draw.asset";
const SNOWFLAKES_UPDATE_SHADER: &str = "snowflakes_update.asset";

const LOCATION_POSITION: usize = 5;
const LOCATION_FALL_SPEED: usize = 6;
const LOCATION_ANGLES: usize = 7;
const LOCATION_ANGULAR_VELOCITY: usize = 8;
const LOCATION_SCALE: usize = 9;

// linear, since the render target is HDR
const BACKGROUND_COLOR: [f32; 4] = [0.4, 0.04, 0.1, 1.0];

fn main() {
    let mut app = App::new();

    assert!(app.render_mut().add_feature("snowflakes", SnowflakesFeature::default()));

    let AppInitCtx {
        mut data,
        mut behavior,
        render,
        ..
    } = app.init_ctx_mut();

    add_defult_modules_to(data.as_mut(), behavior.as_mut(), render);

    let mut ec = data.as_mut().into_entities_mut();
    let camera = ec.create_entity();
    ec.add_component(
        camera,
        GlobalTransform {
            scale_rotation: Mat::IDENTITY,
            position: Vec3::new(0.0, 0.0, 0.0),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        camera,
        CameraComponent(Camera {
            near: 0.1,
            far: 50.0,
            projection: CameraProjection::Perspective {
                fov: 70_f32.to_radians(),
            },
        }),
    )
    .ok()
    .unwrap();

    app.run();
}

struct SnowflakesGpu {
    instances: TypedVertexBuffer,
    // static, so it stays out of the simulated buffer and its storage layout
    scales: TypedVertexBuffer,
    params: GpuUniformStorageBuffer,
}

#[derive(Default)]
struct SnowflakesFeature {
    gpu: Option<SnowflakesGpu>,
}

impl RenderFeature for SnowflakesFeature {
    fn add_passes(&mut self, _world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        passes.add(RENDER_ORDER_PREPARE, |_, ctx| clear_background(ctx));
        passes.add(RENDER_ORDER_OPAQUE, |this, ctx| this.update_and_draw(ctx));
    }
}

fn clear_background(ctx: &mut RenderPassCtx) {
    if let Some(current) = ctx.world.resources().get::<CurrentRenderingResource>() {
        clear_color_target(ctx.render_api, &current.target, BACKGROUND_COLOR);
    }
}

impl SnowflakesFeature {
    fn update_and_draw(&mut self, ctx: &mut RenderPassCtx) {
        let render_api = ctx.render_api;
        let core = render_api.core();
        let res = ctx.world.resources();

        let (Some(current), Some(globals), Some(depth), Some(time)) = (
            res.get::<CurrentRenderingResource>(),
            res.get::<GlobalBindingsResource>(),
            res.get::<DepthTextureResource>(),
            res.get::<TimeResource>(),
        ) else {
            return;
        };

        let (Some(mesh), Some(draw_shader), Some(update_shader)) = (
            get_registered_asset::<StandardMesh>(res, SNOWFLAKE_MESH),
            get_registered_asset::<Shader>(res, SNOWFLAKE_DRAW_SHADER),
            get_registered_asset::<ComputeShader>(res, SNOWFLAKES_UPDATE_SHADER),
        ) else {
            return;
        };
        let update_shader = update_shader.shader();

        let world_to_clip = current.world_to_clip();
        let Some(clip_to_world) = world_to_clip.inverse() else {
            return;
        };

        let Some(params_entry) = update_shader.bind_group_layout_entry(0, 0) else {
            return;
        };

        let gpu = self.gpu.get_or_insert_with(|| SnowflakesGpu {
            instances: scatter_snowflakes(render_api, current.camera_position, clip_to_world),
            scales: randomize_scales(render_api),
            params: core.create_uniform_storage_buffer(Some("snowflakes params"), params_entry.size),
        });

        let params = [
            ("world_to_clip", matrix_bytes(&world_to_clip)),
            ("clip_to_world", matrix_bytes(&clip_to_world)),
            ("delta_time", time.delta_time.to_le_bytes().to_vec()),
            ("count", (SNOWFLAKE_COUNT as u32).to_le_bytes().to_vec()),
        ];

        for (name, bytes) in params {
            let Some(member) = params_entry.member(name) else {
                return;
            };

            core.write_uniform_storage_buffer_with(&gpu.params, member.offset, bytes.len(), |dst| dst.copy_from_slice(&bytes));
        }

        let instances = gpu.instances.buffer();

        core.dispatch(&GpuComputePass {
            shader: update_shader,
            bind_groups: &[&[(0, GpuBinding::Buffer(&gpu.params)), (1, GpuBinding::VertexBuffer(instances))]],
            workgroups: [SNOWFLAKE_COUNT.div_ceil(update_shader.workgroup_size()[0]), 1, 1],
        });

        if mesh.indices_count() == 0 {
            return;
        }

        core.draw_indexed(
            &GpuRenderPass {
                shader: draw_shader.shader(),
                target: Some(GpuRenderPassTarget {
                    view: current.target.texture(),
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                depth: Some(GpuRenderPassDepth {
                    view: depth.texture.texture(),
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                bind_groups: &[&[(0, GpuBinding::Buffer(globals.bindings.uniform_buffer()))]],
                vertex_buffers: &[mesh.vertex_buffer(), instances, gpu.scales.buffer()],
                viewport: None,
                scissor: None,
            },
            mesh.index_buffer(),
            0..mesh.indices_count(),
            0..SNOWFLAKE_COUNT,
        );
    }
}

fn get_registered_asset<'a, T: 'static + Send + Sync>(res: ResourcesHolderRef<'a>, key: &str) -> Option<&'a T> {
    let storage = res.get::<AssetStorageResource<T>>()?;
    storage.get(storage.get_registered(key)?)
}

fn scatter_snowflakes(render_api: &RenderApiResource, camera_position: Vec3<f32>, clip_to_world: Mat4<f32>) -> TypedVertexBuffer {
    let layout = GpuVertexBufferLayout::new(
        GpuVertexStepMode::Instance,
        [
            (LOCATION_POSITION, GpuVertexBufferLayoutEntryTy::Float3),
            (LOCATION_FALL_SPEED, GpuVertexBufferLayoutEntryTy::Float),
            (LOCATION_ANGLES, GpuVertexBufferLayoutEntryTy::Float2),
            (LOCATION_ANGULAR_VELOCITY, GpuVertexBufferLayoutEntryTy::Float2),
        ],
    )
    .unwrap();

    let instances = render_api.create_vertex_buffer(SNOWFLAKE_COUNT, layout);

    render_api.write_vertices_with(&instances, 0, SNOWFLAKE_COUNT, |w| {
        let [x, y] = [random_range(-2.0, 2.0), random_range(-1.0, 2.0)];
        let near = clip_to_world.mul_with_projection(Vec3::new(x, y, 0.0));
        let far = clip_to_world.mul_with_projection(Vec3::new(x, y, 1.0));
        let position = camera_position + (far - near).normalized() * random_range(3.0, 40.0);

        let angular_velocity = || random_range(0.3, 2.0) * if rand::random::<bool>() { 1.0 } else { -1.0 };

        w.set(LOCATION_POSITION, [position.x, position.y, position.z]);
        w.set(LOCATION_FALL_SPEED, random_range(0.2, 1.0));
        w.set(LOCATION_ANGLES, [random_range(0.0, 6.28), random_range(0.0, 6.28)]);
        w.set(LOCATION_ANGULAR_VELOCITY, [angular_velocity(), angular_velocity()]);
    });

    instances
}

fn randomize_scales(render_api: &RenderApiResource) -> TypedVertexBuffer {
    let layout = GpuVertexBufferLayout::new(
        GpuVertexStepMode::Instance,
        [(LOCATION_SCALE, GpuVertexBufferLayoutEntryTy::Float)],
    )
    .unwrap();
    let scales = render_api.create_vertex_buffer(SNOWFLAKE_COUNT, layout);

    render_api.write_vertices_with(&scales, 0, SNOWFLAKE_COUNT, |w| {
        w.set(LOCATION_SCALE, random_range(0.2, 0.7));
    });

    scales
}

fn random_range(min: f32, max: f32) -> f32 {
    min + rand::random::<f32>() * (max - min)
}

fn matrix_bytes(matrix: &Mat4<f32>) -> Vec<u8> {
    matrix.as_array().iter().flatten().flat_map(|x| x.to_le_bytes()).collect()
}
