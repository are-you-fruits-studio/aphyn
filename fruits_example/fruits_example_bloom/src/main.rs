use fruits_engine::*;

fn main() {
    let mut app = App::new();

    add_defult_modules_to(app.ecs_mut().as_mut());

    let ecs = app.ecs_mut();

    let mut behavior = ecs.behavior_mut();
    let mut start = behavior.get_mut(Schedule::Start);
    
    start.insert_system(setup_scene);
    start.order_group(SYSTEM_GROUP_ASSETS).before_system(setup_scene);

    app.run();
}

fn setup_scene(mut world: WorldDataMut) {
    let (mut res, mut ent, _evt) = world.as_tuple_mut();

    let render_api_res = res.get::<RenderApiResource>().unwrap();
    let ground_material = render_api_res.create_material(Default::default(), StandardMaterialAssetMetadata {
        color: Vec4::splat(1.0),
        alpha_threshold: 0.5.into(),
        emission_color: Vec4::splat(0.0),
        is_lit: true,
        metallic: 0.0,
        roughness: 1.0,
        space: RenderSpace::World,
        ..Default::default()
    });
    let light_material = render_api_res.create_material(Default::default(), StandardMaterialAssetMetadata {
        color: Vec4::splat(1.0),
        alpha_threshold: 0.5.into(),
        emission_color: Vec4::new(1.0, 1.0, 1.0, 10.0),
        is_lit: false,
        metallic: 0.0,
        roughness: 1.0,
        space: RenderSpace::World,
        ..Default::default()
    });
    
    let materials = res.get_mut::<AssetStorageResource<StandardMaterial>>().unwrap();
    let ground_material = materials.insert(ground_material);
    let light_material = materials.insert(light_material);

    let meshes = res.get_mut::<AssetStorageResource<StandardMesh>>().unwrap();
    let cube_mesh = meshes.get_registered("cube.asset").unwrap();

    let ent_camera_handle = ent.create_entity();
    let ent_camera = ent.create_entity();

    ent.add_component(ent_camera_handle, GlobalTransform::default()).ok().unwrap();
    ent.add_component(ent_camera_handle, LocalTransform {
        rotation: Quat::rotation_y(-30.0_f64.to_radians()) * Quat::rotation_x(7.0_f64.to_radians()),
        ..Default::default()
    }).ok().unwrap();
    ent.add_component(ent_camera_handle, ParentComponent {
        children: vec![ent_camera].into(),
    }).ok().unwrap();

    ent.add_component(ent_camera, GlobalTransform::default()).ok().unwrap();
    ent.add_component(ent_camera, LocalTransform {
        position: Vec3::new(0.0, 0.0, -10.0),
        ..Default::default()
    }).ok().unwrap();
    ent.add_component(ent_camera, ChildComponent {
        parent: ent_camera_handle,
    }).ok().unwrap();
    ent.add_component(ent_camera, CameraComponent(Camera {
        far: 1000.0,
        near: 0.1,
        projection: CameraProjection::Perspective { fov: 90.0_f32.to_radians() },
    })).ok().unwrap();

    let ent_light = ent.create_entity();
    ent.add_component(ent_light, GlobalTransform::default()).ok().unwrap();
    ent.add_component(ent_light, LocalTransform {
        position: Vec3::new(-1.0, 0.5, -1.0),
        scale: Vec3::splat(0.1),
        ..Default::default()
    }).ok().unwrap();
    ent.add_component(ent_light, StandardLightComponent::Point {
        color: Vec3::new(1.0, 1.0, 1.0) * 3000.0_f32,
        range: 1000.0,
    }).ok().unwrap();
    ent.add_component(ent_light, StandardMaterialComponent { material: light_material.clone() }).ok().unwrap();
    ent.add_component(ent_light, StandardMeshComponent {
        mesh: cube_mesh.clone(),
    }).ok().unwrap();

    let ent_ground = ent.create_entity();
    ent.add_component(ent_ground, GlobalTransform::default()).ok().unwrap();
    ent.add_component(ent_ground, LocalTransform {
        position: Vec3::new(1.0, -1.0, 1.0) * 100.0_f32,
        scale: Vec3::splat(100.0),
        ..Default::default()
    }).ok().unwrap();
    ent.add_component(ent_ground, StandardMaterialComponent { material: ground_material.clone() }).ok().unwrap();
    ent.add_component(ent_ground, StandardMeshComponent {
        mesh: cube_mesh.clone(),
    }).ok().unwrap();
}
