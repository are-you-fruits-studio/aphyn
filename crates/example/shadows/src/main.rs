use aphyn::*;

fn main() {
    let mut app = App::new();

    let AppInitCtx {
        mut data,
        mut behavior,
        render,
        ..
    } = app.init_ctx_mut();

    add_defult_modules_to(data.as_mut(), behavior.as_mut(), render);

    let mut behavior = behavior.as_mut();

    let mut start = behavior.get_mut(Schedule::Start);
    start.insert_system(setup_scene);
    start.order_group(SYSTEM_GROUP_ASSETS).before_system(setup_scene);

    behavior.get_mut(Schedule::Update).insert_system(rotate_sun);

    app.run();
}

#[derive(Component, Default)]
struct SunComponent;

fn rotate_sun(time: Res<TimeResource>, mut query: WorldQuery<&mut LocalTransform, WithFilter<SunComponent>>) {
    let angle = time.frame_time * 0.2;

    for transform in query.iter_mut() {
        transform.rotation = Quat::rotation_y(angle) * Quat::rotation_x(-0.6);
    }
}

fn setup_scene(mut world: WorldDataMut) {
    let (mut res, mut ent, _evt) = world.as_tuple_mut();

    res.get_mut::<AmbientLightResource>().unwrap().color = Vec3::splat(0.05);

    let mut ground_data = MaterialData::new();
    ground_data.insert("roughness", MaterialDataValue::F32(0.9));
    let ground_material = create_material_from_template_key(res.as_ref(), BUILTIN_TEMPLATE_OPAQUE_LIT, ground_data).unwrap();

    let mut cube_data = MaterialData::new();
    cube_data.insert("color", MaterialDataValue::Vec4(Vec4::new(0.9, 0.5, 0.3, 1.0)));
    cube_data.insert("roughness", MaterialDataValue::F32(0.6));
    let cube_material = create_material_from_template_key(res.as_ref(), BUILTIN_TEMPLATE_OPAQUE_LIT, cube_data).unwrap();

    let materials = res.get_mut::<AssetStorageResource<Material>>().unwrap();
    let ground_material = materials.insert(ground_material);
    let cube_material = materials.insert(cube_material);

    let meshes = res.get_mut::<AssetStorageResource<StandardMesh>>().unwrap();
    let cube_mesh = meshes.get_registered("cube.asset").unwrap().clone();

    let ent_camera = ent.create_entity();
    ent.set_components(
        ent_camera,
        (
            GlobalTransform::default(),
            LocalTransform {
                position: Vec3::new(0.0, 9.0, -16.0),
                rotation: Quat::rotation_x(30.0_f64.to_radians()),
                ..Default::default()
            },
            CameraComponent(Camera {
                far: 1000.0,
                near: 0.1,
                projection: CameraProjection::Perspective {
                    fov: 70.0_f32.to_radians(),
                },
            }),
        ),
    );

    let mut spawn_cube = |position: Vec3<f32>, scale: Vec3<f32>, material: &AssetHandle<Material>| {
        let ent_cube = ent.create_entity();
        ent.set_components(
            ent_cube,
            (
                GlobalTransform::default(),
                LocalTransform {
                    position,
                    scale,
                    ..Default::default()
                },
                MaterialComponent {
                    material: material.clone(),
                },
                MeshComponent { mesh: cube_mesh.clone() },
            ),
        );
    };

    spawn_cube(Vec3::new(0.0, -1.0, 0.0), Vec3::new(30.0, 1.0, 30.0), &ground_material);
    spawn_cube(Vec3::new(0.0, 1.0, 0.0), Vec3::splat(1.0), &cube_material);
    spawn_cube(Vec3::new(-4.0, 3.0, 3.0), Vec3::new(0.5, 3.0, 0.5), &cube_material);
    spawn_cube(Vec3::new(4.0, 0.5, -2.0), Vec3::splat(0.5), &cube_material);
    spawn_cube(Vec3::new(7.0, 2.5, 6.0), Vec3::new(1.5, 0.25, 1.5), &cube_material);
    spawn_cube(Vec3::new(7.0, 1.0, 6.0), Vec3::new(0.25, 1.0, 0.25), &cube_material);

    for i in 0..8 {
        let z = 8.0 + i as f32 * 4.0;
        spawn_cube(Vec3::new(-8.0, 1.5, z), Vec3::new(0.5, 1.5, 0.5), &cube_material);
    }

    let ent_sun = ent.create_entity();
    ent.set_components(
        ent_sun,
        (
            GlobalTransform::default(),
            LocalTransform::default(),
            StandardLightComponent::Directional {
                color: Vec3::splat(2.5),
                shadows: true,
            },
            SunComponent,
        ),
    );

    let ent_spot = ent.create_entity();
    ent.set_components(
        ent_spot,
        (
            GlobalTransform::default(),
            LocalTransform {
                position: Vec3::new(7.0, 6.0, 6.0),
                ..Default::default()
            },
            StandardLightComponent::Spot {
                color: Vec3::new(1.0, 0.9, 0.6) * 60.0_f32,
                range: 20.0,
                fov: 70.0_f32.to_radians(),
                shadows: true,
            },
        ),
    );

    // point light shadows are not supported yet, the flag is ignored
    let ent_point = ent.create_entity();
    ent.set_components(
        ent_point,
        (
            GlobalTransform::default(),
            LocalTransform {
                position: Vec3::new(-5.0, 2.0, -4.0),
                ..Default::default()
            },
            StandardLightComponent::Point {
                color: Vec3::new(0.3, 0.5, 1.0) * 10.0_f32,
                range: 20.0,
                shadows: true,
            },
        ),
    );
}
