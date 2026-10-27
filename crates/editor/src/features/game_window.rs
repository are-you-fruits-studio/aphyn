use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        scene_window::{SimulatedViewTarget, ViewCamera, render_simulated_view},
        ui_interaction::{HoverAreaComponent, check_ui_interaction_system},
        world_preload::{SimulatedWorldResource, tick_simulated_world_system},
    },
};

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(GameViewResource::default());
    world.data_mut().resources_mut().insert(GameViewFocusResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(forward_game_input_system)
        .insert_child_system(render_game_window_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(forward_game_input_system)
        .before_system(tick_simulated_world_system)
        .before_system(render_game_window_system);
}

#[derive(Component, Default, Debug)]
pub struct GameViewImageComponent;

#[derive(Resource, Default)]
pub struct GameViewResource(pub SimulatedViewTarget);

#[derive(Resource, Default)]
pub struct GameViewFocusResource {
    pub is_focused: bool,
}

fn forward_game_input_system(
    input: Res<InputResource>,
    mut focus_res: ResMut<GameViewFocusResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    image_q: WorldQuery<(&GlobalRectComponent, &HoverAreaComponent), WithFilter<GameViewImageComponent>>,
) {
    let image = image_q.iter().next();

    let is_any_just_pressed = [MouseButton::Left, MouseButton::Right, MouseButton::Middle].into_iter().any(|b| input.mouse.is_just_pressed(b));

    if is_any_just_pressed {
        focus_res.is_focused = image.is_some_and(|(_, hover_c)| hover_c.is_hovered);
    }

    if !simulated_world.is_playing() || image.is_none() {
        focus_res.is_focused = false;
    }

    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some(sim_input) = simulated_world.world.data_mut().into_resources_mut().into_get_mut::<InputResource>());

    *sim_input = match image.filter(|_| focus_res.is_focused) {
        Some((rect_c, _)) => {
            let mut game_input = input.clone();
            let min = rect_c.center - rect_c.scale * 0.5;
            game_input.mouse.position = [game_input.mouse.position[0] - min.x as f64, game_input.mouse.position[1] - min.y as f64];
            game_input
        }
        None => InputResource::default(),
    };
}

fn render_game_window_system(
    render_api: Res<RenderApiResource>,
    mut view_res: ResMut<GameViewResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut textures: ResMut<AssetStorageResource<StandardTexture>>,
    mut materials: ResMut<AssetStorageResource<StandardMaterial>>,
    mut image_q: WorldQuery<(&GlobalRectComponent, &mut StandardMaterialComponent), WithFilter<GameViewImageComponent>>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some((rect_c, material_c)) = image_q.iter_mut().next());

    let camera = simulated_world
        .world
        .data()
        .entities()
        .query::<(&GlobalTransform, &CameraComponent)>()
        .iter()
        .next()
        .map(|(transform, camera_c)| ViewCamera {
            position: transform.position,
            scale_rotation: transform.scale_rotation,
            camera: camera_c.0,
            light: None,
        });

    render_simulated_view(&render_api, &mut view_res.0, &mut textures, &mut materials, rect_c, material_c, &mut simulated_world.world, camera);
}
