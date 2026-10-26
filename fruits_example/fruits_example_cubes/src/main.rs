use std::time::Instant;

use fruits_engine::*;

fn main() {
    let mut app = App::new();
    let world = app.ecs_mut();

    add_defult_modules_to(world.as_mut());

    world.behavior_mut().get_mut(Schedule::Start).insert_system(init_resources);
    world
        .behavior_mut()
        .get_mut(Schedule::Start)
        .insert_system(init_mesh_material);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(update_time);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(move_cube_new);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(rotate_cube);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(log_fps);
    //world.behavior_mut().get_mut(Schedule::Update).insert_system(log_entities);

    world
        .behavior_mut()
        .get_mut(Schedule::Start)
        .order_system(init_resources)
        .before_system(init_mesh_material);
    world
        .behavior_mut()
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_RENDER)
        .before_system(init_mesh_material);

    let mut ec = world.data_mut().into_entities_mut();

    let entity = ec.create_entity();

    ec.add_component(
        entity,
        GlobalTransform {
            scale_rotation: Mat::IDENTITY,
            position: Vec3::new(0.0_f32, 0.0_f32, -1.0f32),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        entity,
        CameraComponent(Camera {
            near: 0.1_f32,
            far: 1_000_f32,
            projection: CameraProjection::Perspective { fov: 90_f32.to_radians() },
        }),
    )
    .ok()
    .unwrap();

    dbg!(ec.entities_count());

    println!("start");
    app.run();
    println!("end");
}

#[derive(Resource)]
struct SampleResource {}

#[derive(Component)]
struct MovingCubeComponent;

#[derive(Component)]
struct RotatingCubeComponent;

#[derive(Resource)]
struct TimeResource {
    pub time: f32,
    pub start: Option<Instant>,
}

#[derive(Resource)]
struct FpsResource {
    pub last_measure_seconds: usize,
    pub count: usize,
}

fn init_resources(mut world: WorldDataMut) {
    world.as_mut().resources_mut().insert(SampleResource {});
    world.as_mut()
        .resources_mut()
        .insert(FpsResource {
            last_measure_seconds: 0,
            count: 0,
        });
    world.as_mut()
        .resources_mut()
        .insert(TimeResource {
            time: 0.0_f32,
            start: None,
        });
}

fn init_mesh_material(mut world: WorldDataMut) {
    let Some(render_api) = world.as_ref().resources().get::<RenderApiResource>() else {
        return;
    };

    let material = render_api.create_material(Default::default(), StandardMaterialAssetMetadata {
        shader: StandardEngineShader::OpaqueLit.into(),
        ..Default::default()
    });

    let mut vertices = [
        StandardVertex {
            position: [0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
            ..Default::default()
        },
        StandardVertex {
            position: [1.0, 0.0, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
            ..Default::default()
        },
        StandardVertex {
            position: [0.0, 1.0, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
            uv: [0.0, 1.0],
            ..Default::default()
        },
        StandardVertex {
            position: [1.0, 1.0, 0.0],
            color: [1.0, 1.0, 0.0, 1.0],
            uv: [1.0, 1.0],
            ..Default::default()
        },
        StandardVertex {
            position: [0.0, 0.0, 1.0],
            color: [0.0, 0.0, 1.0, 1.0],
            uv: [1.0, 0.0],
            ..Default::default()
        },
        StandardVertex {
            position: [1.0, 0.0, 1.0],
            color: [1.0, 0.0, 1.0, 1.0],
            uv: [0.0, 0.0],
            ..Default::default()
        },
        StandardVertex {
            position: [0.0, 1.0, 1.0],
            color: [0.0, 1.0, 1.0, 1.0],
            uv: [1.0, 1.0],
            ..Default::default()
        },
        StandardVertex {
            position: [1.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            uv: [0.0, 1.0],
            ..Default::default()
        },
    ];

    for vertex in vertices.iter_mut() {
        for ele in vertex.position.iter_mut() {
            *ele = *ele * 2.0 - 1.0;
            *ele *= 0.2_f32;
        }

        //vertex.position[2] += 3.0_f32;
        //vertex.position[0] += 1.0_f32;
    }

    let indices = [
        0, 1, 3, 0, 3, 2, 0, 4, 5, 0, 5, 1, 0, 6, 4, 0, 2, 6, 1, 7, 3, 1, 5, 7, 2, 7, 6, 2, 3, 7, 4, 6, 7, 4, 7, 5,
    ];

    let mut vertices = indices.iter().map(|i| vertices[*i]).collect::<Vec<_>>();

    for i in 0..(vertices.len() / 3) {
        let offset = i * 3;
        let vec1 = Vec3::from_array(vertices[offset].position) - Vec3::from_array(vertices[offset + 2].position);
        let vec2 = Vec3::from_array(vertices[offset].position) - Vec3::from_array(vertices[offset + 1].position);
        let normal = vec1.cross(vec2).normalized();

        for j in 0..3 {
            vertices[offset + j].normal = normal.into_array();
        }
    }

    let indices = vertices.iter().enumerate().map(|(i, _)| i as u16).collect::<Vec<_>>();

    let mesh = render_api.create_mesh(&vertices, &indices, None);

    let material = world.as_mut()
        .resources_mut()
        .get_mut::<AssetStorageResource<StandardMaterial>>()
        .unwrap()
        .insert(material);
    let mesh = world.as_mut()
        .resources_mut()
        .get_mut::<AssetStorageResource<StandardMesh>>()
        .unwrap()
        .insert(mesh);

    for _ in 0..3 {
        let mut parent_transform = LocalTransform::IDENTITY;

        parent_transform.scale.x *= 0.7;

        let mut ec = world.entities_mut();

        let parent = ec.create_entity();
        ec.add_component(parent, parent_transform).ok().unwrap();

        let entity = ec.create_entity();

        ec.add_component(entity, StandardMeshComponent { mesh: mesh.clone() })
            .ok()
            .unwrap();
        ec.add_component(
            entity,
            StandardMaterialComponent {
                material: material.clone(),
            },
        )
        .ok()
        .unwrap();
        ec.add_component(entity, LocalTransform::IDENTITY).ok().unwrap();
        ec.add_component(entity, ChildComponent { parent }).ok().unwrap();
        ec.add_component(entity, RotatingCubeComponent).ok().unwrap();
        ec.add_component(entity, MovingCubeComponent).ok().unwrap();
    }
}

fn update_time(mut time: ResMut<TimeResource>) {
    let start = match time.start {
        Some(start) => start,
        None => {
            let new_start = Instant::now();
            time.start = Some(new_start);
            new_start
        }
    };

    time.time = start.elapsed().as_secs_f32();
}

fn move_cube_new(time: Res<TimeResource>, mut query: WorldQuery<(EntityId, &mut LocalTransform, &MovingCubeComponent)>) {
    let mut i = 0;

    for (_entity, transform, _) in query.iter_mut() {
        i += 1;

        if i == 1 {
            transform.position.x = time.time.cos() * 0.5_f32;
        } else if i == 2 {
            transform.position.y = time.time.cos() * 0.5_f32;
        } else if i == 3 {
            transform.position.z = time.time.cos() * 0.5_f32;
        }
    }
}

fn rotate_cube(time: Res<TimeResource>, mut query: WorldQuery<(EntityId, &mut LocalTransform, &RotatingCubeComponent)>) {
    let mut i = 0;

    for (_entity, transform, _) in query.iter_mut() {
        i += 1;

        transform.rotation = Quat::rotation_y(time.time as f64 * 1.0_f64 * i as f64) * Quat::rotation_z(-45.0_f64.to_radians());
    }
}

fn log_fps(mut fps: ResMut<FpsResource>, time: Res<TimeResource>) {
    fps.count += 1;

    if time.time - fps.last_measure_seconds as f32 >= 1.0_f32 {
        println!("fps: {}", fps.count);
        fps.last_measure_seconds = time.time.floor() as usize;

        fps.count = 0;
    }
}
