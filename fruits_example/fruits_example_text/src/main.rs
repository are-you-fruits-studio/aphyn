use std::{fmt::Write, time::Instant};

use fruits_engine::*;

fn main() {
    let mut app = App::new();
    let world = app.ecs_mut();

    add_defult_modules_to(world.as_mut());
    fps_counter::add_module_to(world.as_mut());

    let mut world_behavior = world.behavior_mut();

    world_behavior.get_mut(Schedule::Start).insert_system(init);
    world_behavior.get_mut(Schedule::Update).insert_system(update_time);
    world_behavior.get_mut(Schedule::Update).insert_system(write_time_into_text);

    world_behavior
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_RENDER)
        .before_system(init);

    world_behavior
        .get_mut(Schedule::Start)
        .order_group(SYSTEM_GROUP_UI)
        .before_system(init);

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
            projection: CameraProjection::Perspective { fov: 90_f32.to_radians() },
        }),
    ));

    println!("start");
    app.run();
    println!("end");
}

#[derive(Resource)]
struct TimeResource {
    pub time: f32,
    pub frame: u64,
    pub start: Option<Instant>,
}

fn init(mut world: WorldDataMut) {
    world.as_mut()
        .resources_mut()
        .insert(TimeResource {
            time: 0.0_f32,
            frame: 0,
            start: None,
        });

    let standard_ui_assets = world.as_ref().resources().get::<StandardUiAssetsResource>().unwrap();

    let texture_text_handle = standard_ui_assets.texture_text_px_5_7.clone();
    let font = standard_ui_assets.font_px_5_7.clone();

    let material = StandardMaterialAssetMetadata {
        shader: StandardEngineShader::OpaqueUnlit.into(),
        color_tex: texture_text_handle.clone(),
        ..Default::default()
    };

    let material = world.as_ref()
        .resources()
        .get::<RenderApiResource>()
        .unwrap()
        .create_material(material);

    let material = world.as_mut()
        .resources_mut()
        .get_mut::<AssetStorageResource<StandardMaterial>>()
        .unwrap()
        .insert(material);

    let mut ec = world.entities_mut();

    let ent_text = ec.create_entity();
    ec.add_component(ent_text, GlobalTransform::IDENTITY).ok().unwrap();
    ec.add_component(
        ent_text,
        LocalTransform {
            position: Vec3::new(0.0, 0.0, 0.0),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ec.add_component(ent_text, BatchedMeshComponent::default()).ok().unwrap();
    ec.add_component(
        ent_text,
        StandardMaterialComponent {
            material: material.clone(),
        },
    )
    .ok()
    .unwrap();
    ec.add_component(
        ent_text,
        TextComponent {
            font: font.clone(),
            font_size: UiVal::px(0.4),
            text: String::from("Dashunia - myla pinhvinka").into(),
            horizontal_align: HorizontalAlign::Middle,
            vertical_align: VerticalAlign::Middle,
            is_y_inverted: false,
            horizontal_spacing: UiVal::px(0.2),
            color: Vec4::splat(1.0),
        },
    )
    .ok()
    .unwrap();
}

fn write_time_into_text(time: Res<TimeResource>, mut text_q: WorldQuery<&mut TextComponent>) {
    for text_c in text_q.iter_mut() {
        text_c.text.clear();

        write!(text_c.text, "{:.2}", time.time).unwrap();
    }
}

fn update_time(mut time: ResMut<TimeResource>) {
    let start = match time.start {
        Some(start) => start,
        None => {
            let new_start = Instant::now();
            time.start = Some(new_start);
            time.frame = 0;
            new_start
        }
    };

    time.frame += 1;
    time.time = start.elapsed().as_secs_f32();
}
