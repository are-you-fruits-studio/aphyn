use std::time::Instant;

use fruits_engine::*;

fn main() {
    let mut app = App::new();

    add_defult_modules_to(app.ecs_mut().as_mut());

    let ecs = app.ecs_mut();

    ecs.data_mut()
        .resources_mut()
        .insert(BoidSettings {
            attraction_threshold: 1.0,
            damping_factor: 0.2,
        });

    let mut systems = ecs.behavior_mut();

    systems.get_mut(Schedule::Start).insert_system(init);
    systems
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_RENDER)
        .before_system(init);

    let mut update_systems = systems.get_mut(Schedule::Update);

    update_systems.insert_system(accumulate_boid_separation);
    update_systems.insert_system(affect_motor_by_boid);
    update_systems.insert_system(apply_motor);
    update_systems.insert_system(apply_damping);
    update_systems.insert_system(apply_velocity);
    update_systems.insert_system(restrict_boids);
    update_systems.insert_system(rotate_boids_by_velocity);

    update_systems
        .order_system(accumulate_boid_separation)
        .before_system(affect_motor_by_boid);
    update_systems.order_system(affect_motor_by_boid).before_system(apply_motor);
    update_systems.order_system(apply_motor).before_system(apply_damping);
    update_systems.order_system(apply_damping).before_system(apply_velocity);
    update_systems
        .order_system(apply_velocity)
        .before_system(rotate_boids_by_velocity);
    update_systems
        .order_system(rotate_boids_by_velocity)
        .before_system(restrict_boids);

    app.run();
}

#[derive(Component)]
struct Boid {
    target_direction: Vec3<f32>,
}

#[derive(Component)]
struct Velocity(pub Vec3<f32>);

#[derive(Component)]
struct Motor {
    pub acceleration_direction: Vec3<f32>,
    pub strength: f32,
}

#[derive(Component)]
struct BoidTarget {}

#[derive(Resource)]
struct BoidSettings {
    pub attraction_threshold: f32,
    pub damping_factor: f32,
}

fn init(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let material = render_api.create_material(Default::default(), StandardMaterialAssetMetadata {
        shader: StandardEngineShader::OpaqueUnlit.into(),
        ..Default::default()
    });

    let mut vertices = [
        StandardVertex {
            position: [0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
            ..Default::default()
        },
        StandardVertex {
            position: [0.5, 1.0, 0.0],
            color: [0.2, 0.5, 1.0, 1.0],
            ..Default::default()
        },
        StandardVertex {
            position: [1.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
            ..Default::default()
        },
    ];

    for vertex in vertices.iter_mut() {
        vertex.position[0] -= 0.5_f32;
        vertex.position[1] -= 0.5_f32;

        for dimention in vertex.position.iter_mut() {
            *dimention *= 0.2_f32;
        }
    }

    let indices = [0, 2, 1];

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

    let ec = &mut world.entities_mut();

    for _ in 0..100 {
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
        ec.add_component(
            entity,
            Boid {
                target_direction: Vec3::splat(0.0),
            },
        )
        .ok()
        .unwrap();
        ec.add_component(entity, BoidTarget {}).ok().unwrap();
        ec.add_component(
            entity,
            Motor {
                acceleration_direction: Vec3::splat(0.0),
                strength: 0.01,
            },
        )
        .ok()
        .unwrap();
        ec.add_component(entity, Velocity(Vec3::splat(0.0))).ok().unwrap();
        ec.add_component(
            entity,
            GlobalTransform {
                scale_rotation: Mat3::IDENTITY,
                position: Vec3::new(rand::random::<f32>(), rand::random::<f32>(), 0.0),
            },
        )
        .ok()
        .unwrap();
    }

    {
        let camera_entity = ec.create_entity();

        ec.add_component(
            camera_entity,
            GlobalTransform {
                scale_rotation: Mat::IDENTITY,
                position: Vec3::new(0.0_f32, 0.0_f32, -5.0f32),
            },
        )
        .ok()
        .unwrap();
        ec.add_component(
            camera_entity,
            CameraComponent(Camera {
                near: 0.1_f32,
                far: 1_000_f32,
                projection: CameraProjection::Perspective { fov: 90_f32.to_radians() },
            }),
        )
        .ok()
        .unwrap();
    }
}

fn accumulate_boid_separation(
    boid_settings: Res<BoidSettings>,
    targets_queue: WorldQuery<(EntityId, &GlobalTransform, &BoidTarget)>,
    mut boids_queue: WorldQuery<(EntityId, &GlobalTransform, &mut Boid)>,
) {
    let timer = Instant::now();

    for (boid_entity, boid_transform, boid) in boids_queue.iter_mut() {
        let mut sum = Vec3::splat(0.0_f32);

        for (target_entity, target_transform, _) in targets_queue.iter() {
            if target_entity == boid_entity {
                continue;
            }

            let difference = target_transform.position - boid_transform.position;

            let distance = difference.length();

            let attraction_strength = distance as f32 - boid_settings.attraction_threshold;

            sum += difference.normalized() * attraction_strength;
        }

        boid.target_direction += sum.normalized();
    }

    println!(
        "{:>5} fps - {:>10.3} ms",
        (1.0 / timer.elapsed().as_secs_f64()) as u32,
        timer.elapsed().as_secs_f64() * 1000.0
    );
}

fn affect_motor_by_boid(mut query: WorldQuery<(&Boid, &mut Motor)>) {
    for (boid, motor) in query.iter_mut() {
        motor.acceleration_direction = (motor.acceleration_direction.normalized() + boid.target_direction).normalized();
    }
}

fn apply_motor(mut query: WorldQuery<(&Motor, &mut Velocity)>) {
    for (motor, velocity) in query.iter_mut() {
        velocity.0 += motor.acceleration_direction.normalized() * motor.strength;
    }
}

fn apply_damping(mut query: WorldQuery<&mut Velocity>, boid_settings: Res<BoidSettings>) {
    for velocity in query.iter_mut() {
        velocity.0 -= velocity.0 * boid_settings.damping_factor;
    }
}

fn apply_velocity(mut query: WorldQuery<(&Velocity, &mut GlobalTransform)>) {
    for (velocity, transform) in query.iter_mut() {
        transform.position += velocity.0;
    }
}

fn rotate_boids_by_velocity(mut query: WorldQuery<(&mut GlobalTransform, &Velocity, &Boid)>) {
    for (transform, velocity, _) in query.iter_mut() {
        let angle = f32::atan2(velocity.0.x, velocity.0.y);
        transform.scale_rotation = Mat3::rotation_z(-angle as f64)
    }
}

fn restrict_boids(mut query: WorldQuery<(&mut GlobalTransform, &Boid)>) {
    for (transform, _) in query.iter_mut() {
        transform.position = Vec3::new(
            transform.position.x.clamp(-5.0, 5.0),
            transform.position.y.clamp(-5.0, 5.0),
            transform.position.z.clamp(-5.0, 5.0),
        );
    }
}
