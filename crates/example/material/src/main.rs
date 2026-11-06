use std::collections::HashMap;
use std::time::Instant;

use aphyn::*;

fn main() {
    let mut app = App::new();

    assert!(
        app.render_mut()
            .add_material_template("unlit_color", UnlitColorTemplate::new())
    );
    assert!(app.render_mut().add_feature("vignette", VignetteFeature::default()));

    let steady = app.render_mut().create_material("unlit_color").unwrap();
    let mut blinking = app.render_mut().create_material("unlit_color").unwrap();
    *blinking.data.get_vec4_mut("color").unwrap() = Vec4::new(1.0, 0.45, 0.1, 1.0);

    let world = app.ecs_mut();

    add_defult_modules_to(world.as_mut());

    let mut data = world.data_mut();
    let mut resources = data.resources_mut();
    let materials = resources.get_mut::<AssetStorageResource<Material>>().unwrap();
    let materials = ExampleMaterialsResource {
        steady: materials.insert(steady),
        blinking: materials.insert(blinking),
    };
    resources.insert(materials);
    resources.insert(TimeResource {
        start: Instant::now(),
        time: 0.0,
        last_blink_second: 0,
    });

    let mut ec = data.into_entities_mut();
    let camera = ec.create_entity();
    ec.add_component(
        camera,
        GlobalTransform {
            scale_rotation: Mat::IDENTITY,
            position: Vec3::new(0.0, 0.0, -4.0),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        camera,
        CameraComponent(Camera {
            near: 0.1,
            far: 100.0,
            projection: CameraProjection::Perspective {
                fov: 70_f32.to_radians(),
            },
        }),
    )
    .ok()
    .unwrap();

    world.behavior_mut().get_mut(Schedule::Start).insert_system(spawn_cubes);
    world
        .behavior_mut()
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_RENDER)
        .before_system(spawn_cubes);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(update_time);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(rotate_cubes);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(blink_material);
    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(update_time)
        .before_system(rotate_cubes);
    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(update_time)
        .before_system(blink_material);

    app.run();
}

#[derive(Resource)]
struct ExampleMaterialsResource {
    steady: AssetHandle<Material>,
    blinking: AssetHandle<Material>,
}

#[derive(Resource)]
struct TimeResource {
    start: Instant,
    time: f32,
    last_blink_second: u64,
}

#[derive(Component)]
struct SpinningComponent {
    speed: f32,
}

fn spawn_cubes(mut world: WorldDataMut) {
    let Some(render_api) = world.as_ref().resources().get::<RenderApiResource>() else {
        return;
    };

    let corners = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
    ];
    let vertices = corners.map(|corner: [f32; 3]| StandardVertex {
        position: corner.map(|c| (c * 2.0 - 1.0) * 0.35),
        ..Default::default()
    });
    let indices = [
        0, 1, 3, 0, 3, 2, 0, 4, 5, 0, 5, 1, 0, 6, 4, 0, 2, 6, 1, 7, 3, 1, 5, 7, 2, 7, 6, 2, 3, 7, 4, 6, 7, 4, 7, 5,
    ];
    let mesh = render_api.create_mesh(&vertices, &indices, None);

    let mut world_mut = world.as_mut();
    let mut resources = world_mut.resources_mut();
    let mesh = resources
        .get_mut::<AssetStorageResource<StandardMesh>>()
        .unwrap()
        .insert(mesh);
    let materials = resources.get::<ExampleMaterialsResource>().unwrap();
    let (steady, blinking) = (materials.steady.clone(), materials.blinking.clone());

    let mut ec = world.entities_mut();

    for i in 0..10 {
        let column = (i % 5) as f32;
        let row = (i / 5) as f32;
        let material = if i % 2 == 0 { steady.clone() } else { blinking.clone() };

        let entity = ec.create_entity();
        ec.add_component(
            entity,
            LocalTransform {
                position: Vec3::new((column - 2.0) * 1.2, (row - 0.5) * 1.4, 0.0),
                ..LocalTransform::IDENTITY
            },
        )
        .ok()
        .unwrap();
        ec.add_component(entity, MeshComponent { mesh: mesh.clone() }).ok().unwrap();
        ec.add_component(entity, MaterialComponent { material }).ok().unwrap();
        ec.add_component(
            entity,
            SpinningComponent {
                speed: 0.5 + i as f32 * 0.15,
            },
        )
        .ok()
        .unwrap();
    }
}

fn update_time(mut time: ResMut<TimeResource>) {
    time.time = time.start.elapsed().as_secs_f32();
}

fn rotate_cubes(time: Res<TimeResource>, mut query: WorldQuery<(&mut LocalTransform, &SpinningComponent)>) {
    for (transform, spinning) in query.iter_mut() {
        let angle = (time.time * spinning.speed) as f64;
        transform.rotation = Quat::rotation_y(angle) * Quat::rotation_x(angle * 0.7);
    }
}

fn blink_material(
    mut time: ResMut<TimeResource>,
    example_materials: Res<ExampleMaterialsResource>,
    mut materials: ResMut<AssetStorageResource<Material>>,
) {
    let second = time.time as u64;
    if second == time.last_blink_second {
        return;
    }
    time.last_blink_second = second;

    let Some(material) = materials.get_mut(&example_materials.blinking) else {
        return;
    };
    let Some(color) = material.data.get_vec4_mut("color") else {
        return;
    };

    *color = if second % 2 == 0 {
        Vec4::new(1.0, 0.45, 0.1, 1.0)
    } else {
        Vec4::new(0.1, 0.8, 0.9, 1.0)
    };
}

struct UnlitMaterialGpu {
    uniform: TypedUniformBuffer,
    uploaded_snapshot: Option<u64>,
}

struct UnlitGpu {
    shader: GpuShader,
    camera: TypedUniformBuffer,
    depth: GpuTexture,
}

struct UnlitColorTemplate {
    defaults: MaterialData,
    batches: Vec<MaterialBatch>,
    gpu: Option<UnlitGpu>,
    materials: HashMap<AssetHandle<Material>, UnlitMaterialGpu>,
}

impl UnlitColorTemplate {
    fn new() -> Self {
        let mut defaults = MaterialData::new();
        defaults.insert("color", MaterialDataValue::Vec4(Vec4::new(1.0, 1.0, 1.0, 1.0)));

        Self {
            defaults,
            batches: Vec::new(),
            gpu: None,
            materials: HashMap::new(),
        }
    }
}

impl MaterialTemplate for UnlitColorTemplate {
    type Pass = ();

    fn default_data(&self) -> MaterialData {
        self.defaults.clone()
    }

    fn add_passes(&mut self, _world: WorldDataMut, batches: &[MaterialBatch], passes: &mut RenderPassWriter<()>) {
        self.batches.clear();
        self.batches.extend_from_slice(batches);

        if !self.batches.is_empty() {
            passes.add(RENDER_ORDER_OPAQUE, ());
        }
    }

    fn run_pass(&mut self, _pass: &(), ctx: &mut RenderPassCtx) {
        let render_api = ctx.render_api;
        let core = render_api.core();
        let resources = ctx.world.resources();

        let Some(current) = resources.get::<CurrentRenderingResource>() else {
            return;
        };
        let Some(instances) = resources.get::<MaterialInstancesResource>() else {
            return;
        };
        let (Some(meshes), Some(materials)) = (
            resources.get::<AssetStorageResource<StandardMesh>>(),
            resources.get::<AssetStorageResource<Material>>(),
        ) else {
            return;
        };

        let target = current.target.texture();
        let size = target.size();

        if self.gpu.is_none() {
            let Some(shader) = create_shader(
                core,
                "unlit_color.wgsl",
                include_str!("unlit_color.wgsl"),
                GpuShaderDesc::default(),
            ) else {
                return;
            };

            self.gpu = Some(UnlitGpu {
                shader,
                camera: render_api.create_uniform_buffer(TypedUniformBufferLayout::new([GpuRuntimeTy::Float4x4])),
                depth: core.create_texture(Some("unlit_color depth"), size, DEPTH_TEXTURE_FORMAT, 1),
            });
        }
        let gpu = self.gpu.as_mut().unwrap();

        if gpu.depth.size() != size {
            gpu.depth = core.create_texture(Some("unlit_color depth"), size, DEPTH_TEXTURE_FORMAT, 1);
        }
        core.clear_depth_texture(&gpu.depth, 0, 1.0);

        gpu.camera.set(0, current.world_to_clip().into_array());
        render_api.write_uniform_buffer_if_dirty(&mut gpu.camera);

        for batch in &self.batches {
            let (Some(mesh), Some(material)) = (meshes.get(&batch.mesh), materials.get(&batch.material)) else {
                continue;
            };

            let material_gpu = self
                .materials
                .entry(batch.material.clone())
                .or_insert_with(|| UnlitMaterialGpu {
                    uniform: render_api.create_uniform_buffer(TypedUniformBufferLayout::new([GpuRuntimeTy::Float4])),
                    uploaded_snapshot: None,
                });

            let snapshot = material.data.entry("color").map(|entry| entry.snapshot()).unwrap_or(0);
            if material_gpu.uploaded_snapshot != Some(snapshot) {
                let color = material.data.get_vec4_or("color", &self.defaults);
                material_gpu.uniform.set(0, color.into_array());
                render_api.write_uniform_buffer_if_dirty(&mut material_gpu.uniform);
                material_gpu.uploaded_snapshot = Some(snapshot);
            }

            let [instance_start, instance_count] = instances.buffer.clamp_range(batch.instance_start, batch.instance_count);
            if instance_count == 0 {
                continue;
            }

            core.draw_indexed(
                &GpuRenderPass {
                    shader: &gpu.shader,
                    target: Some(GpuRenderPassTarget {
                        view: target,
                        mip_level: 0,
                        ops: GpuPassOps::Load,
                    }),
                    depth: Some(GpuRenderPassDepth {
                        view: &gpu.depth,
                        mip_level: 0,
                        ops: GpuPassOps::Load,
                    }),
                    bind_groups: &[
                        &[(0, GpuBinding::Buffer(gpu.camera.buffer()))],
                        &[(0, GpuBinding::Buffer(material_gpu.uniform.buffer()))],
                    ],
                    vertex_buffers: &[mesh.vertex_buffer(), instances.buffer.buffer()],
                    viewport: None,
                    scissor: None,
                },
                mesh.index_buffer(),
                0..mesh.indices_count(),
                instance_start as usize..(instance_start + instance_count) as usize,
            );
        }
    }
}

struct VignetteGpu {
    shader: GpuShader,
    sampler: GpuSampler,
    source: GpuTexture,
}

#[derive(Default)]
struct VignetteFeature {
    gpu: Option<VignetteGpu>,
}

impl RenderFeature for VignetteFeature {
    type Pass = ();

    fn add_passes(&mut self, _world: WorldDataMut, passes: &mut RenderPassWriter<()>) {
        passes.add(RENDER_ORDER_POST_PROCESSING, ());
    }

    fn run_pass(&mut self, _pass: &(), ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let Some(current) = ctx.world.resources().get::<CurrentRenderingResource>() else {
            return;
        };
        let target = current.target.texture();

        if self.gpu.is_none() {
            let desc = GpuShaderDesc {
                cull_mode: GpuCullMode::None,
                depth_write: false,
                depth_compare: GpuCompareFunction::Always,
                ..Default::default()
            };
            let Some(shader) = create_shader(core, "vignette.wgsl", include_str!("vignette.wgsl"), desc) else {
                return;
            };

            self.gpu = Some(VignetteGpu {
                shader,
                sampler: core.create_sampler(Some("vignette"), GpuFilterMode::Linear),
                source: core.create_texture(Some("vignette source"), target.size(), target.format(), 1),
            });
        }
        let gpu = self.gpu.as_mut().unwrap();

        if gpu.source.size() != target.size() || gpu.source.format() != target.format() {
            gpu.source = core.create_texture(Some("vignette source"), target.size(), target.format(), 1);
        }

        if !core.copy_texture(target, &gpu.source) {
            return;
        }

        core.draw(
            &GpuRenderPass {
                shader: &gpu.shader,
                target: Some(GpuRenderPassTarget {
                    view: target,
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                depth: None,
                bind_groups: &[&[
                    (
                        0,
                        GpuBinding::Texture {
                            texture: &gpu.source,
                            mip_level: None,
                        },
                    ),
                    (1, GpuBinding::Sampler(&gpu.sampler)),
                ]],
                vertex_buffers: &[],
                viewport: None,
                scissor: None,
            },
            0..3,
            0..1,
        );
    }
}

fn create_shader(core: &RenderApi, name: &str, source: &str, desc: GpuShaderDesc) -> Option<GpuShader> {
    let code = match WgslComposer::with_engine_libraries().compose(name, source, &[]) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            return None;
        }
    };

    core.create_shader(Some(name), &code, desc)
}
