use std::time::Instant;

use aphyn::*;

fn main() {
    let mut app = App::new();

    assert!(app.render_mut().add_feature("vignette", VignetteFeature::default()));

    let AppInitCtx {
        mut data,
        mut behavior,
        render,
        ..
    } = app.init_ctx_mut();

    add_defult_modules_to(data.as_mut(), behavior.as_mut(), render);

    let mut data = data.as_mut();
    data.resources_mut().insert(TimeResource {
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

    behavior.as_mut().get_mut(Schedule::Start).insert_system(spawn_cubes);
    behavior
        .as_mut()
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_RENDER)
        .before_system(spawn_cubes);
    behavior.as_mut().get_mut(Schedule::Update).insert_system(update_time);
    behavior.as_mut().get_mut(Schedule::Update).insert_system(rotate_cubes);
    behavior.as_mut().get_mut(Schedule::Update).insert_system(blink_material);
    behavior
        .as_mut()
        .get_mut(Schedule::Update)
        .order_system(update_time)
        .before_system(rotate_cubes);
    behavior
        .as_mut()
        .get_mut(Schedule::Update)
        .order_system(update_time)
        .before_system(blink_material);

    app.run();
}

#[derive(Resource)]
struct ExampleMaterialsResource {
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

fn unlit_color_template(shader: AssetHandle<Shader>) -> MaterialTemplate {
    let mut template = MaterialTemplate::default();
    template.inputs.insert(
        "color".into(),
        MaterialInput::new(MaterialDataValue::Vec4(Vec4::new(1.0, 1.0, 1.0, 1.0))),
    );
    template.passes.push(MaterialTemplatePass {
        shader,
        order: RENDER_ORDER_OPAQUE,
        target: MaterialPassTarget::Main,
        values: vec![MaterialValueBinding {
            input: "color".into(),
            group: 1,
            binding: 0,
            member: "color".into(),
        }]
        .into(),
        textures: FfiVec::new(),
    });

    template
}

fn spawn_cubes(mut world: WorldDataMut) {
    let Some(render_api) = world
        .as_ref()
        .resources()
        .get::<RenderApiResource>()
        .map(RenderApiResource::clone)
    else {
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

    let code = WgslComposer::with_engine_libraries()
        .compose("unlit_color.wgsl", include_str!("unlit_color.wgsl"), &[])
        .unwrap();
    let shader = render_api
        .create_shader("unlit_color.wgsl", &code, GpuShaderDesc::default(), None)
        .unwrap();

    let mut world_mut = world.as_mut();
    let mut resources = world_mut.resources_mut();

    let shader = resources.get_mut::<AssetStorageResource<Shader>>().unwrap().insert(shader);
    let template = resources
        .get_mut::<AssetStorageResource<MaterialTemplate>>()
        .unwrap()
        .insert(unlit_color_template(shader));
    let mesh = resources
        .get_mut::<AssetStorageResource<StandardMesh>>()
        .unwrap()
        .insert(mesh);

    let (steady, blinking) = {
        let templates = resources.get::<AssetStorageResource<MaterialTemplate>>().unwrap();
        let shaders = resources.get::<AssetStorageResource<Shader>>().unwrap();

        let steady = render_api.create_material(templates, shaders, template.clone(), MaterialData::new());

        let mut blinking_data = MaterialData::new();
        blinking_data.insert("color", MaterialDataValue::Vec4(Vec4::new(1.0, 0.45, 0.1, 1.0)));
        let blinking = render_api.create_material(templates, shaders, template, blinking_data);

        (steady, blinking)
    };

    let materials = resources.get_mut::<AssetStorageResource<Material>>().unwrap();
    let (steady, blinking) = (materials.insert(steady), materials.insert(blinking));
    resources.insert(ExampleMaterialsResource {
        blinking: blinking.clone(),
    });

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
    example_materials: Option<Res<ExampleMaterialsResource>>,
    mut materials: ResMut<AssetStorageResource<Material>>,
) {
    let Some(example_materials) = example_materials else {
        return;
    };
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
    fn add_passes(&mut self, _world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
        passes.add(RENDER_ORDER_POST_PROCESSING, |this, ctx| this.draw(ctx));
    }
}

impl VignetteFeature {
    fn draw(&mut self, ctx: &mut RenderPassCtx) {
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
