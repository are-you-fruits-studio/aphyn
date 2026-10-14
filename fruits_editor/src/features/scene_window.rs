use std::time::Instant;

use fruits_engine::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        ui_interaction::{HoverAreaComponent, check_ui_interaction_system},
        world_preload::{SimulatedWorldResource, tick_simulated_world_system},
    },
};

const MOVE_SPEED: f32 = 5.0;
const LOOK_SENSITIVITY: f32 = 0.004;
const PITCH_LIMIT: f32 = 89.0_f32.to_radians();
const CLEAR_COLOR: [f64; 4] = [0.05, 0.05, 0.05, 1.0];
const SUN_YAW: f64 = 30.0_f64.to_radians();
const SUN_PITCH: f64 = -50.0_f64.to_radians();
const SUN_COLOR: Vec3<f32> = Vec3::splat(3.0);

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(SceneCameraResource::default());
    world.data_mut().resources_mut().insert(SceneViewResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(control_scene_camera_system)
        .insert_child_system(render_scene_window_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(control_scene_camera_system)
        .before_system(render_scene_window_system);

    update
        .order_system(tick_simulated_world_system)
        .before_system(render_scene_window_system);
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
                fov: 90_f32.to_radians(),
            },
            is_controlling: false,
            last_mouse_pos: Vec2::splat(0.0),
            last_tick: Instant::now(),
        }
    }
}

#[derive(Resource, Default)]
pub struct SceneViewResource {
    pub texture: Option<AssetHandle<StandardTexture>>,
    pub material: Option<AssetHandle<StandardMaterial>>,
    pub size: [u32; 2],
}

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

fn render_scene_window_system(
    render_api: Res<RenderApiResource>,
    camera_res: Res<SceneCameraResource>,
    mut view_res: ResMut<SceneViewResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut textures: ResMut<AssetStorageResource<StandardTexture>>,
    mut materials: ResMut<AssetStorageResource<StandardMaterial>>,
    mut image_q: WorldQuery<(&GlobalRectComponent, &mut StandardMaterialComponent), WithFilter<SceneViewImageComponent>>,
) {
    let Some(simulated_world) = &mut simulated_world.0 else {
        return;
    };

    let Some((rect_c, material_c)) = image_q.iter_mut().next() else {
        return;
    };

    let size = rect_c.scale.map(|v| v.round().max(1.0) as u32).into_array();

    if view_res.texture.is_none() || view_res.size != size {
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

        match (&view_res.texture, &view_res.material) {
            (Some(texture_handle), Some(material_handle)) => {
                *textures.get_mut(texture_handle).unwrap() = texture;
                *materials.get_mut(material_handle).unwrap() = material;
            }
            _ => {
                view_res.texture = Some(textures.insert(texture));
                view_res.material = Some(materials.insert(material));
            }
        }

        view_res.size = size;
    }

    let (Some(texture_handle), Some(material_handle)) = (&view_res.texture, &view_res.material) else {
        return;
    };

    material_c.material = material_handle.clone();

    let target_texture = unsafe { textures.get(texture_handle).unwrap().native() }.texture.clone();
    let target_view = target_texture.create_view(&Default::default());

    clear_color_target(&*unsafe { render_api.raw() }, &target_view, CLEAR_COLOR);

    simulated_world
        .world
        .data_mut()
        .resources_mut()
        .insert(CurrentRenderingResource {
            camera_position: camera_res.position,
            camera_scale_rotation: camera_res.scale_rotation(),
            camera: camera_res.camera,
            light: Some(sun_light()),
            target_texture,
            target_view,
        });

    simulated_world.world.execute_iteration(Schedule::RenderCamera);

    simulated_world
        .world
        .data_mut()
        .resources_mut()
        .remove::<CurrentRenderingResource>();
}

fn sun_light() -> StandardLight {
    StandardLight::Directional {
        color: SUN_COLOR,
        direction_dst: Mat3::rotation_y(SUN_YAW) * Mat3::rotation_x(SUN_PITCH) * Vec3::new(0.0, 0.0, 1.0),
    }
}
