use std::time::Instant;

use fruits_engine::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        inspector_window::{data::HierarchyWindowEntryComponent, systems::update_hierarchy_window_system},
        ui_interaction::{ButtonClickEvent, DOUBLE_CLICK_INTERVAL, HoverAreaComponent, check_ui_interaction_system},
        game_window::GameViewImageComponent,
        world_preload::{SimulatedWorldResource, preload_assets_into_simulated_world_system, tick_simulated_world_system},
    },
};

const MOVE_SPEED: f32 = 5.0;
const LOOK_SENSITIVITY: f32 = 0.004;
const PITCH_LIMIT: f32 = 89.0_f32.to_radians();
const CLEAR_COLOR: [f64; 4] = [0.05, 0.05, 0.05, 1.0];
const SUN_YAW: f64 = 30.0_f64.to_radians();
const SUN_PITCH: f64 = -50.0_f64.to_radians();
const SUN_COLOR: Vec3<f32> = Vec3::splat(3.0);
const FOCUS_DISTANCE_SCALE: f32 = 3.0;
const FOCUS_MIN_DISTANCE: f32 = 2.0;

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(SceneCameraResource::default());
    world.data_mut().resources_mut().insert(SceneViewResource::default());
    world.data_mut().resources_mut().insert(HierarchyDoubleClickResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(control_scene_camera_system)
        .insert_child_system(focus_scene_camera_on_entity_system)
        .insert_child_system(sync_simulated_window_system)
        .insert_child_system(render_scene_window_system);

    update
        .order_system(preload_assets_into_simulated_world_system)
        .before_system(sync_simulated_window_system)
        .before_system(tick_simulated_world_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(control_scene_camera_system)
        .before_system(render_scene_window_system);

    update
        .order_system(tick_simulated_world_system)
        .before_system(render_scene_window_system);

    update
        .order_system(control_scene_camera_system)
        .before_system(focus_scene_camera_on_entity_system)
        .before_system(render_scene_window_system);

    update
        .order_system(focus_scene_camera_on_entity_system)
        .before_system(update_hierarchy_window_system);
}

#[derive(Component, Default, Debug)]
pub struct SceneViewImageComponent;

#[derive(Resource)]
pub struct SceneCameraResource {
    pub position: Vec3<f32>,
    pub yaw: f32,
    pub pitch: f32,
    pub camera: Camera,
    pub is_controlling: bool,
    pub last_mouse_pos: Vec2<f32>,
    pub last_tick: Instant,
}

impl SceneCameraResource {
    pub fn scale_rotation(&self) -> Mat3<f32> {
        Mat3::rotation_y(self.yaw as f64) * Mat3::rotation_x(self.pitch as f64)
    }
}

impl Default for SceneCameraResource {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, -5.0),
            yaw: 0.0,
            pitch: 0.0,
            camera: Camera {
                near: 0.1,
                far: 1_000.0,
                projection: CameraProjection::Perspective { fov: 90_f32.to_radians() },
            },
            is_controlling: false,
            last_mouse_pos: Vec2::splat(0.0),
            last_tick: Instant::now(),
        }
    }
}

// todo: maybe replace with already existing ui_interaction double-click functionality?
#[derive(Resource, Default)]
pub struct HierarchyDoubleClickResource {
    pub last_click: Option<(EntityId, Instant)>,
}

#[derive(Default)]
pub struct SimulatedViewTarget {
    pub texture: Option<AssetHandle<StandardTexture>>,
    pub material: Option<AssetHandle<StandardMaterial>>,
    pub size: [u32; 2],
}

#[derive(Resource, Default)]
pub struct SceneViewResource(pub SimulatedViewTarget);

fn control_scene_camera_system(
    input: Res<InputResource>,
    mut camera_res: ResMut<SceneCameraResource>,
    image_q: WorldQuery<&HoverAreaComponent, WithFilter<SceneViewImageComponent>>,
) {
    let now = Instant::now();
    let dt = (now - camera_res.last_tick).as_secs_f32();
    camera_res.last_tick = now;

    let mouse_pos = Vec2::from_array(input.mouse.position.map(|v| v as f32));
    let mouse_delta = mouse_pos - camera_res.last_mouse_pos;
    camera_res.last_mouse_pos = mouse_pos;

    if input.mouse.is_just_pressed(MouseButton::Right) && image_q.iter().any(|hover_c| hover_c.is_hovered) {
        camera_res.is_controlling = true;
    }

    if !input.mouse.is_pressed(MouseButton::Right) {
        camera_res.is_controlling = false;
    }

    if !camera_res.is_controlling {
        return;
    }

    camera_res.yaw += mouse_delta.x * LOOK_SENSITIVITY;
    camera_res.pitch = (camera_res.pitch + mouse_delta.y * LOOK_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);

    let axis = |positive: KeyCode, negative: KeyCode| {
        input.keyboard.is_pressed(positive) as i32 as f32 - input.keyboard.is_pressed(negative) as i32 as f32
    };

    let local_dir = Vec3::new(axis(KeyCode::KeyD, KeyCode::KeyA), 0.0, axis(KeyCode::KeyW, KeyCode::KeyS));
    let world_dir = camera_res.scale_rotation() * local_dir;

    camera_res.position += world_dir * (MOVE_SPEED * dt);
}

fn focus_scene_camera_on_entity_system(
    input: Res<InputResource>,
    click_evt: Evt<ButtonClickEvent>,
    entry_q: WorldQuery<&HierarchyWindowEntryComponent>,
    simulated_world: Res<SimulatedWorldResource>,
    mut double_click_res: ResMut<HierarchyDoubleClickResource>,
    mut camera_res: ResMut<SceneCameraResource>,
) {
    // hierarchy entries are respawned every frame, so double-clicks are tracked by the simulated entity
    let clicked_sim_ent = click_evt.iter().find_map(|e| entry_q.get(e.entity)).map(|c| c.simulated_entity);

    let Some(sim_ent) = clicked_sim_ent else {
        if input.mouse.is_just_pressed(MouseButton::Left) {
            double_click_res.last_click = None;
        }
        return;
    };

    let now = Instant::now();
    let is_double_click = double_click_res
        .last_click
        .is_some_and(|(ent, time)| ent == sim_ent && now.duration_since(time) <= DOUBLE_CLICK_INTERVAL);

    double_click_res.last_click = if is_double_click { None } else { Some((sim_ent, now)) };

    return_if!(!is_double_click);
    return_if_not!(Some(simulated_world) = &simulated_world.0);
    return_if_not!(Some(transform) = simulated_world.world.data().entities().get_component::<GlobalTransform>(sim_ent));

    let scale = transform.scale_rotation.to_lossy_scale();
    let distance = (scale.x.max(scale.y).max(scale.z) * FOCUS_DISTANCE_SCALE).max(FOCUS_MIN_DISTANCE);
    let forward = (camera_res.scale_rotation() * Vec3::new(0.0, 0.0, 1.0)).normalized();

    camera_res.position = transform.position - forward * distance;
}

fn sync_simulated_window_system(
    mut simulated_world: ResMut<SimulatedWorldResource>,
    scene_image_q: WorldQuery<&GlobalRectComponent, WithFilter<SceneViewImageComponent>>,
    game_image_q: WorldQuery<&GlobalRectComponent, WithFilter<GameViewImageComponent>>,
) {
    let is_playing = simulated_world.is_playing();
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    let game_rect_c = game_image_q.iter().next().filter(|_| is_playing);
    return_if_not!(Some(rect_c) = game_rect_c.or_else(|| scene_image_q.iter().next()));

    let size = view_size(rect_c);
    let window_res = simulated_world.world.data_mut().into_resources_mut().into_get_mut::<WindowResource>().unwrap();

    // resetting both states also drops window changes requested by the simulated game
    let title = window_res.prev_state().title.clone();
    WindowResource::reset(window_res, &WindowState { fullscreen: FullscreenState::Windowed, size, title });
}

fn view_size(rect_c: &GlobalRectComponent) -> [u32; 2] {
    rect_c.scale.map(|v| v.round().max(1.0) as u32).into_array()
}

fn render_scene_window_system(
    render_api: Res<RenderApiResource>,
    camera_res: Res<SceneCameraResource>,
    mut view_res: ResMut<SceneViewResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut textures: ResMut<AssetStorageResource<StandardTexture>>,
    mut materials: ResMut<AssetStorageResource<StandardMaterial>>,
    mut image_q: WorldQuery<(&GlobalRectComponent, &mut StandardMaterialComponent), WithFilter<SceneViewImageComponent>>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some((rect_c, material_c)) = image_q.iter_mut().next());

    let camera = ViewCamera {
        position: camera_res.position,
        scale_rotation: camera_res.scale_rotation(),
        camera: camera_res.camera,
        light: Some(sun_light()),
    };

    render_simulated_view(&render_api, &mut view_res.0, &mut textures, &mut materials, rect_c, material_c, &mut simulated_world.world, Some(camera));
}

pub struct ViewCamera {
    pub position: Vec3<f32>,
    pub scale_rotation: Mat3<f32>,
    pub camera: Camera,
    pub light: Option<StandardLight>,
}

pub fn render_simulated_view(
    render_api: &RenderApiResource,
    view: &mut SimulatedViewTarget,
    textures: &mut AssetStorageResource<StandardTexture>,
    materials: &mut AssetStorageResource<StandardMaterial>,
    rect_c: &GlobalRectComponent,
    material_c: &mut StandardMaterialComponent,
    world: &mut World,
    camera: Option<ViewCamera>,
) {
    let size = view_size(rect_c);

    if view.texture.is_none() || view.size != size {
        let texture = render_api.create_texture(
            FilterMode::Nearest,
            size,
            &[],
            StandardTextureAssetMetadata {
                format: StandardTextureFormat::Rgba16Float,
                ..Default::default()
            },
        );

        let material = render_api.create_material(
            StandardMaterialAssets {
                color_texture: Some(&texture),
                ..Default::default()
            },
            StandardMaterialAssetMetadata {
                is_lit: false,
                space: RenderSpace::Window,
                color: Vec4::splat(1.0),
                alpha_threshold: Some(0.5).into(),
                ..Default::default()
            },
        );

        match (&view.texture, &view.material) {
            (Some(texture_handle), Some(material_handle)) => {
                *textures.get_mut(texture_handle).unwrap() = texture;
                *materials.get_mut(material_handle).unwrap() = material;
            }
            _ => {
                view.texture = Some(textures.insert(texture));
                view.material = Some(materials.insert(material));
            }
        }

        view.size = size;
    }

    let (Some(texture_handle), Some(material_handle)) = (&view.texture, &view.material) else {
        return;
    };

    material_c.material = material_handle.clone();

    let target_texture = unsafe { textures.get(texture_handle).unwrap().native() }.texture.clone();
    let target_view = target_texture.create_view(&Default::default());

    clear_color_target(&*unsafe { render_api.raw() }, &target_view, CLEAR_COLOR);

    return_if_not!(Some(camera) = camera);

    world.data_mut().resources_mut().insert(CurrentRenderingResource {
        camera_position: camera.position,
        camera_scale_rotation: camera.scale_rotation,
        camera: camera.camera,
        light: camera.light,
        target_texture,
        target_view,
    });

    world.execute_iteration(Schedule::RenderCamera);

    world.data_mut().resources_mut().remove::<CurrentRenderingResource>();
}

fn sun_light() -> StandardLight {
    StandardLight::Directional {
        color: SUN_COLOR,
        direction_dst: Mat3::rotation_y(SUN_YAW) * Mat3::rotation_x(SUN_PITCH) * Vec3::new(0.0, 0.0, 1.0),
    }
}
