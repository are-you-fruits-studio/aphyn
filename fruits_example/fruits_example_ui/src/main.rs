use fruits_engine::*;

fn main() {
    let mut app = App::new();
    let world = app.ecs_mut();

    add_defult_modules_to(world.as_mut());
    fps_counter::add_module_to(world.as_mut());

    world.behavior_mut().get_mut(Schedule::Start).insert_system(init);
    world.behavior_mut().get_mut(Schedule::Update).insert_system(update_cursor_system);

    world
        .behavior_mut()
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_RENDER)
        .before_system(init);

    world
        .behavior_mut()
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_UI)
        .before_system(init);

    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(update_cursor_system)
        .before_group(SYSTEM_GROUP_TRANSFORM);

    let mut world_data = world.data_mut();

    let mut ec = world_data.entities_mut();

    let entity = ec.create_entity();
    ec.set_components(entity, (
        GlobalTransform {
            scale_rotation: Mat::IDENTITY,
            position: Vec3::new(0.0_f32, 0.0_f32, -1.0f32),
        },
        CameraComponent(Camera {
            near: 0.1_f32,
            far: 1_000_f32,
            fov: 90_f32.to_radians(),
        }),
    ));

    println!("start");
    app.run();
    println!("end");
}

#[derive(Component, Default)]
pub struct CursorComponent {
    vel: Vec2<f32>,
}

fn init(mut world: WorldDataMut) {
    let standard_ui_assets = world.as_ref().resources().get::<StandardUiAssetsResource>().unwrap();

    let texture_text_handle = standard_ui_assets.texture_text_px_8_12.clone();
    let font = standard_ui_assets.font_px_8_12.clone();

    let material_white = world.as_mut()
        .resources_mut()
        .get_mut::<RenderApiResource>()
        .unwrap()
        .create_material(Default::default(), StandardMaterialAssetMetadata {
            is_lit: false,
            space: RenderSpace::Window,
            color_tex: AssetHandle::EMPTY,
            color: Vec4::splat(1.0),
            alpha_threshold: Some(0.5).into(),
            ..Default::default()
        });

    let texture_text = world.as_ref()
        .resources()
        .get::<AssetStorageResource<StandardTexture>>()
        .unwrap()
        .get(&texture_text_handle);

    let material_text = world.as_ref()
        .resources()
        .get::<RenderApiResource>()
        .unwrap()
        .create_material(
            StandardMaterialAssets {
                color_texture: texture_text,
                ..Default::default()
            },
            StandardMaterialAssetMetadata {
                is_lit: false,
                space: RenderSpace::Window,
                color_tex: texture_text_handle.clone(),
                color: Vec4::splat(1.0),
                alpha_threshold: Some(0.5).into(),
                ..Default::default()
            },
        );

    let material_white = world.as_mut()
        .resources_mut()
        .get_mut::<AssetStorageResource<StandardMaterial>>()
        .unwrap()
        .insert(material_white);

    let material_text = world.as_mut()
        .resources_mut()
        .get_mut::<AssetStorageResource<StandardMaterial>>()
        .unwrap()
        .insert(material_text);

    let mut ec = world.entities_mut();

    let ent1 = ec.create_entity();
    let ent2 = ec.create_entity();
    let ent3 = ec.create_entity();
    let ent4 = ec.create_entity();

    ec.add_component(
        ent1,
        ParentComponent {
            children: vec![ent2].into(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent1, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent1,
        StandardMaterialComponent {
            material: material_white.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent1, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent1,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::pw(0.5), UiVal::ph(0.5)),
            parent_padding_max: Vec2::new(UiVal::pw(0.5), UiVal::ph(0.5)),
            z: 0.0,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ec.add_component(ent2, ChildComponent { parent: ent1 }).ok().unwrap();
    ec.add_component(
        ent2,
        ParentComponent {
            children: vec![ent3].into(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent2, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent2,
        StandardMaterialComponent {
            material: material_white.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent2, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent2,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(-100.0), UiVal::px(-100.0)),
            parent_padding_max: Vec2::new(UiVal::px(-100.), UiVal::px(-100.0)),
            z: -10.0,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent2, ImageComponent::default()).ok().unwrap();
    ec.add_component(ent2, ChildrenRectMaskComponent).ok().unwrap();

    ec.add_component(ent3, ChildComponent { parent: ent2 }).ok().unwrap();
    ec.add_component(
        ent3,
        ParentComponent {
            children: vec![ent4].into(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent3, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent3,
        StandardMaterialComponent {
            material: material_white.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent3, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent3,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(10.0), UiVal::px(10.0)),
            parent_padding_max: Vec2::new(UiVal::vmin(0.1), UiVal::vmin(0.1)),
            z: -10.0,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent3,
        ImageComponent {
            color: Vec4::splat(0.6),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ec.add_component(ent4, ChildComponent { parent: ent3 }).ok().unwrap();
    ec.add_component(ent4, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent4,
        StandardMaterialComponent {
            material: material_text.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent4, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent4,
        LocalRectComponent {
            z: -10.0,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent4,
        TextComponent {
            font: font.clone(),
            font_size: UiVal::vh(0.15),
            text: String::from("Dashunia - myla pinhvinka").into(),
            horizontal_align: HorizontalAlign::Middle,
            vertical_align: VerticalAlign::Bottom,
            is_y_inverted: true,
            horizontal_spacing: UiVal::px(0.0),
            color: Vec4::new(1.0, 0.0, 1.0, 1.0),
        },
    )
    .ok()
    .unwrap();

    //

    let ent10 = ec.create_entity();
    let ent11 = ec.create_entity();
    let ent12 = ec.create_entity();
    let ent13 = ec.create_entity();

    ec.add_component(ent10, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent10,
        LocalRectComponent {
            pivot: Vec2::splat(0.0),
            anchor: Vec2::splat(0.0),
            scale: Vec2::new(None.into(), None.into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent10,
        RectChildAlignComponent {
            direction: UiDirection::Vertical,
            anchor: Vec2::new(0.5, 0.5),
            min_gap: UiVal::vd(0.1),
            spacing: UiSpacing::Chunk,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent10,
        ParentComponent {
            children: vec![ent11, ent12, ent13].into(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent10, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent10,
        StandardMaterialComponent {
            material: material_white.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent10,
        ImageComponent {
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ec.add_component(ent11, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent11,
        LocalRectComponent {
            scale: Vec2::splat(UiVal::px(50.0).into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent11, ChildComponent { parent: ent10 }).ok().unwrap();
    ec.add_component(ent11, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent11,
        StandardMaterialComponent {
            material: material_white.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent11,
        ImageComponent {
            color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ec.add_component(ent12, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent12,
        LocalRectComponent {
            scale: Vec2::splat(UiVal::px(40.0).into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent12, ChildComponent { parent: ent10 }).ok().unwrap();
    ec.add_component(ent12, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent12,
        StandardMaterialComponent {
            material: material_white.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent12,
        ImageComponent {
            color: Vec4::new(0.0, 1.0, 0.0, 1.0),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ec.add_component(ent13, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(
        ent13,
        LocalRectComponent {
            scale: Vec2::splat(UiVal::px(70.0).into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent13, ChildComponent { parent: ent10 }).ok().unwrap();
    ec.add_component(ent13, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent13,
        StandardMaterialComponent {
            material: material_white.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent13,
        ImageComponent {
            color: Vec4::new(0.0, 0.0, 1.0, 1.0),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    //

    let ent_cursor = ec.create_entity();

    ec.add_component(ent_cursor, GlobalRectComponent::default()).ok().unwrap();
    ec.add_component(ent_cursor, LocalRectComponent {
        anchor: Vec2::splat(0.0),
        pivot: Vec2::splat(0.0),
        scale: Vec2::splat(UiVal::px(3.0).into()),
        ..Default::default()
    }).ok().unwrap();
    ec.add_component(ent_cursor, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(ent_cursor, StandardMaterialComponent { material: material_white.clone() }).ok().unwrap();
    ec.add_component(ent_cursor, ImageComponent {
        color: Vec4::new(1.0, 1.0, 1.0, 1.0),
        ..Default::default()
    }).ok().unwrap();
    ec.add_component(ent_cursor, CursorComponent::default()).ok().unwrap();
}

fn update_cursor_system(
    input: Res<InputResource>,
    mut q: WorldQuery<(&mut CursorComponent, &mut LocalRectComponent)>,
) {
    for (cursor, rect) in q.iter_mut() {
        let a = rect.offset.map(|x| x.val);
        let b = Vec2::from_array(input.mouse.position.map(|x| UiVal::px(x as f32))).map(|x| x.val);
        let x = Vec2::from_fn(|i| damp(a[i], b[i], &mut cursor.vel[i], 5.0, 0.1, 1.0 / 60.0));
        rect.offset = x.map(UiVal::px);
    }
}