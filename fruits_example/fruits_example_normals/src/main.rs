use fruits_engine::*;

fn main() {
    let mut app = App::new();
    let world = app.ecs_mut();

    add_defult_modules_to(world.as_mut());

    let mut behavior = world.behavior_mut();

    let mut start = behavior.get_mut(Schedule::Start);

    start.insert_system(init_scene_system);
    
    start
        .order_group(SYSTEM_GROUP_ASSETS)
        .before_system(init_scene_system);

    let mut update = behavior.get_mut(Schedule::Update);
    
    update.insert_system(follow_cursor_system);

    let mut ec = world.data_mut().into_entities_mut();

    let ent_camera = ec.create_entity();

    ec.add_component(ent_camera, GlobalTransform {
        scale_rotation: Mat::IDENTITY,
        position: Vec3::new(0.0, 0.0, -1.0),
    }).ok().unwrap();
    ec.add_component(ent_camera, CameraComponent(Camera {
        near: 0.1_f32,
        far: 1_000_f32,
        projection: CameraProjection::Perspective { fov: 90_f32.to_radians() },
    })).ok().unwrap();

    let ent_light = ec.create_entity();

    ec.add_component(ent_light, GlobalTransform::default()).ok().unwrap();
    ec.add_component(ent_light, LocalTransform {
        position: Vec3::new(0.0, 0.0, 0.0),
        ..Default::default()
    }).ok().unwrap();
    ec.add_component(ent_light, StandardLightComponent::Point {
        color: Vec3::new(1.0, 1.0, 1.0) * 1.0_f32,
        range: 100.0,
    }).ok().unwrap();
    ec.add_component(ent_light, FollowCursorComponent { distance: 1.5 }).ok().unwrap();

    app.run();
}

#[derive(Component)]
struct FollowCursorComponent {
    pub distance: f32,
}

fn init_scene_system(mut world: WorldDataMut) {
    let render_api = world.resources().get::<RenderApiResource>().unwrap();
    let textures = world.resources().get::<AssetStorageResource<StandardTexture>>().unwrap();

    let texture_normal = textures.get_registered("normal_map.asset");
    let texture_normal = textures.get(&texture_normal.cloned().unwrap_or_default());

    let material = render_api.create_material(
        StandardMaterialAssets {
            color_texture: texture_normal,
            normal_texture: texture_normal,
            ..Default::default()
        },
        StandardMaterialAssetMetadata {
            is_lit: true,
            alpha_threshold: 0.0.into(),
            color: Vec4::splat(1.0),
            metallic: 0.0,
            roughness: 1.0,
            space: RenderSpace::World,
            emission_color: Vec4::splat(0.0),
            ..Default::default()
        },
    );

    let materials = world.resources_mut().into_get_mut::<AssetStorageResource<StandardMaterial>>().unwrap();

    let material = materials.insert(material);

    let mut ent = world.entities_mut();

    let ent_normal = ent.create_entity();

    let create_vertex = |uv, position| StandardVertex {
        position,
        normal: [0.0, 0.0, -1.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
        color: [1.0; 4],
        uv,
    };

    ent.add_component(ent_normal, GlobalTransform::default()).ok().unwrap();
    ent.add_component(ent_normal, LocalTransform {
        position: Vec3::new(0.0, 0.0, 1.0),
        scale: Vec3::splat(1.0),
        ..Default::default()
    }).ok().unwrap();
    ent.add_component(ent_normal, StandardMaterialComponent { material }).ok().unwrap();
    ent.add_component(ent_normal, BatchedMeshComponent {
        vertices: vec![
            create_vertex([0.0, 0.0], [-1.0, -1.0, 0.0]),
            create_vertex([1.0, 0.0], [1.0, -1.0, 0.0]),
            create_vertex([0.0, 1.0], [-1.0, 1.0, 0.0]),
            create_vertex([1.0, 1.0], [1.0, 1.0, 0.0]),
        ].into(),
        indices: vec![
            0, 1, 2,
            1, 3, 2,
        ].into(),
    }).ok().unwrap();
}

fn follow_cursor_system(
    mut follow_q: WorldQuery<(&FollowCursorComponent, &mut LocalTransform)>,
    camera_q: WorldQuery<(&CameraComponent, &GlobalTransform)>,
    render_state: Res<RenderApiResource>,
    input: Res<InputResource>,
    mut gizmos: ResMut<GizmosResource>,
) {
    let (camera, camera_transform) = camera_q.iter().next().unwrap();
    
    let window_size = render_state.size();

    let aspect = window_size[0] as f32 / window_size[1] as f32;

    let window_size = Vec2::from_array(window_size.map(|u| u as f32));

    let projection_matrix = camera.0.projection_matrix(aspect);

    let mouse_pos = Vec2::from_array(input.mouse.position.map(|f| f as f32));
    let mut mouse_clip_pos = mouse_pos / window_size * 2.0 - Vec2::splat(1.0);
    mouse_clip_pos.y *= -1.0;
    
    let mouse_world_pos = camera_transform.position + projection_matrix.inverse().unwrap().mul_with_projection(mouse_clip_pos.xyn(0.1));
    let mouse_world_dir = mouse_world_pos - camera_transform.position;
    let mouse_world_dir = mouse_world_dir / mouse_world_dir.z;

    for (follow_c, transform) in follow_q.iter_mut() {
        transform.position = camera_transform.position + mouse_world_dir * follow_c.distance;
        draw_gizmo_cross(gizmos.space(RenderSpace::World), transform.position, Vec4::splat(1.0));
    }
}

fn draw_gizmo_cross(gizmos: &mut FfiVec<GizmoLine>, pos: Vec3<f32>, color: Vec4<f32>) {
    let scale = 0.1_f32;

    gizmos.push(GizmoLine {
        color,
        start: pos - Vec3::<f32>::X * scale,
        end: pos + Vec3::<f32>::X * scale,
    });
    gizmos.push(GizmoLine {
        color,
        start: pos - Vec3::<f32>::Y * scale,
        end: pos + Vec3::<f32>::Y * scale,
    });
    gizmos.push(GizmoLine {
        color,
        start: pos - Vec3::<f32>::Z * scale,
        end: pos + Vec3::<f32>::Z * scale,
    });
}