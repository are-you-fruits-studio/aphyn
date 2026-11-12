use std::{sync::Arc, time::Instant};

use aphyn_ecs::{ResourcesHolderMut, Schedule, WorldBehavior, WorldBehaviorBuilder, WorldData, WorldDataMut};
use aphyn_render::{CurrentRenderingResource, RenderApiResource};
use aphyn_render_features::CameraComponent;
use aphyn_render_features::RenderFeatureRegistry;
use aphyn_transform::GlobalTransform;
use gilrs::Gilrs;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::PhysicalKey,
    window::{Window, WindowId},
};

use crate::{
    GamepadInputStorage, InputResource, TextInputEvent, TimeResource,
    render_target::RenderTarget,
    run_frame_schedules,
    window_resources::{
        window_attributes_from_state, window_state_apply_changes, window_state_apply_cursor, window_state_refresh_from_window,
    },
};
use aphyn_window::{WindowResource, WindowState};

enum EventLoopHandlerState {
    Created(WorldData, WorldBehaviorBuilder, WindowState, RenderFeatureRegistry),
    Starting,
    Polling {
        data: WorldData,
        behavior: WorldBehavior,
        window: Arc<Window>,
        gamepad_host: Option<Gilrs>,
        render_target: RenderTarget,
        render: RenderFeatureRegistry,
        last_frame: Instant,
    },
}

impl EventLoopHandlerState {
    fn try_start_creating(&mut self) -> Option<(WorldData, WorldBehaviorBuilder, WindowState, RenderFeatureRegistry)> {
        let EventLoopHandlerState::Created { .. } = self else {
            return None;
        };

        let EventLoopHandlerState::Created(data, behavior, window_state, render) =
            std::mem::replace(self, EventLoopHandlerState::Starting)
        else {
            return None;
        };

        Some((data, behavior, window_state, render))
    }
}

pub struct EventLoopHandler(EventLoopHandlerState);

impl EventLoopHandler {
    pub fn new(data: WorldData, behavior: WorldBehaviorBuilder, window_state: WindowState, render: RenderFeatureRegistry) -> Self {
        Self(EventLoopHandlerState::Created(data, behavior, window_state, render))
    }
}

impl ApplicationHandler for EventLoopHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some((mut data, behavior, mut window_state, render)) = self.0.try_start_creating() else {
            return;
        };

        let window_attr = window_attributes_from_state(&window_state, event_loop);

        let window = Arc::new(event_loop.create_window(window_attr).unwrap());

        window_state_apply_cursor(&mut window_state, &window);
        window_state_refresh_from_window(&mut window_state, &window);

        let state = RenderApiResource::new(Arc::clone(&window));

        let render_target = RenderTarget::new(&state);

        data.resources_mut().insert(state);
        data.resources_mut().insert(InputResource::default());
        data.resources_mut().insert(TimeResource::default());
        data.resources_mut().insert(WindowResource::new(window_state));
        let mut behavior = behavior.build();
        behavior.execute_iteration(Schedule::Start, data.as_mut());
        core_system_update_window(data.as_mut(), &window);

        let gamepad_host = Gilrs::new();

        if let Err(err) = &gamepad_host {
            eprintln!("ayf: Failed to init Gilrs(gamepad). {}", err);
        }

        self.0 = EventLoopHandlerState::Polling {
            data,
            behavior,
            window,
            gamepad_host: gamepad_host.ok(),
            render_target,
            render,
            last_frame: Instant::now(),
        };
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: DeviceId, event: DeviceEvent) {
        let EventLoopHandlerState::Polling { .. } = &mut self.0 else {
            return;
        };

        match event {
            // todo
            // DeviceEvent::MouseMotion { delta } => {
            //     let mut input = world.data().resources().get_mut::<InputResource>().unwrap();

            //     input.mouse.position[0] += delta.0;
            //     input.mouse.position[1] += delta.1;
            // },
            _ => (),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let EventLoopHandlerState::Polling {
            data,
            behavior,
            window,
            gamepad_host,
            render_target,
            render,
            last_frame,
        } = &mut self.0
        else {
            return;
        };

        let (mut res, _ent, mut evt) = data.as_tuple_mut();

        let render_state = res.get_mut::<RenderApiResource>().unwrap();

        if window_id != window.id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::CursorMoved { position, .. } => {
                let input = res.get_mut::<InputResource>().unwrap();

                input.mouse.position = [position.x, position.y];
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let input = res.get_mut::<InputResource>().unwrap();

                match state {
                    ElementState::Pressed => input.mouse.press(button),
                    ElementState::Released => input.mouse.release(button),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let input = res.get_mut::<InputResource>().unwrap();

                match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        input.mouse.scroll_delta_lines[0] += x;
                        input.mouse.scroll_delta_lines[1] += y;
                    }
                    MouseScrollDelta::PixelDelta(physical_position) => {
                        input.mouse.scroll_delta_pixels[0] += physical_position.x;
                        input.mouse.scroll_delta_pixels[1] += physical_position.y;
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.repeat {
                    return;
                }

                if let Some(text) = event.text {
                    evt.get_mut().push(TextInputEvent(text.as_str().into()));
                }

                let PhysicalKey::Code(key_code) = event.physical_key else {
                    return;
                };

                let input = res.get_mut::<InputResource>().unwrap();

                match event.state {
                    ElementState::Pressed => input.keyboard.press(key_code),
                    ElementState::Released => input.keyboard.release(key_code),
                }
            }
            WindowEvent::Resized(new_size) => {
                render_state.resize([new_size.width, new_size.height]);
            }
            WindowEvent::RedrawRequested => {
                core_system_gamepad(res, gamepad_host);
                core_system_read_window(data.as_mut(), window);

                let now = Instant::now();
                let real_delta_time = now.duration_since(*last_frame).as_secs_f32();
                *last_frame = now;

                run_frame_schedules(behavior, data.as_mut(), real_delta_time);

                core_system_render(data, render_target, render);

                core_system_clear_events_and_input(data.as_mut());
                core_system_update_window(data.as_mut(), window);

                window.request_redraw();
            }
            WindowEvent::Destroyed => {
                // todo
            }
            _ => {}
        }
    }
}

//

fn core_system_gamepad(mut res: ResourcesHolderMut, gamepad_host: &mut Option<Gilrs>) {
    let input = res.get_mut::<InputResource>().unwrap();

    if let Some(gamepad_host) = gamepad_host {
        fn get_or_create_gamepad(input: &mut InputResource, gamepad_id: usize) -> &mut GamepadInputStorage {
            input.gamepads.entry(gamepad_id).or_insert_with(|| GamepadInputStorage::new())
        }

        for (gamepad_id, _) in gamepad_host.gamepads() {
            get_or_create_gamepad(input, gamepad_id.into());
        }

        while let Some(gamepad_evt) = gamepad_host.next_event() {
            let gamepad_id: usize = gamepad_evt.id.into();

            match gamepad_evt.event {
                gilrs::EventType::Connected => _ = get_or_create_gamepad(input, gamepad_id),
                gilrs::EventType::Disconnected => _ = input.gamepads.remove_swap(&gamepad_id),
                gilrs::EventType::ButtonPressed(button, _) => get_or_create_gamepad(input, gamepad_id).press(button),
                gilrs::EventType::ButtonReleased(button, _) => get_or_create_gamepad(input, gamepad_id).release(button),
                gilrs::EventType::AxisChanged(axis, axis_value, _) => {
                    get_or_create_gamepad(input, gamepad_id).write_axis(axis, axis_value)
                }
                // todo: handle other gamepad events
                _ => (),
            }
        }
    }
}

fn core_system_render(data: &mut WorldData, render_target: &mut RenderTarget, render: &mut RenderFeatureRegistry) {
    let render_api = data.resources().get::<RenderApiResource>().unwrap().clone();

    if render_target.size() != render_api.size() {
        *render_target = RenderTarget::new(&render_api);
    }

    render_target.clear(&render_api);

    let camera = data
        .entities()
        .query::<(&GlobalTransform, &CameraComponent)>()
        .iter()
        .next()
        .map(|(transform, camera)| (*transform, camera.0));

    if let Some((camera_transform, camera)) = camera {
        render.render_view(
            data,
            CurrentRenderingResource {
                camera_position: camera_transform.position,
                camera_scale_rotation: camera_transform.scale_rotation,
                camera,
                light: None,
                target: render_target.texture.clone(),
            },
        );
    }

    render_target.render_to_surface(&render_api);
}

fn core_system_clear_events_and_input(mut world: WorldDataMut) {
    world.events_mut().clear();

    let mut res = world.into_resources_mut();

    let input = res.get_mut::<InputResource>().unwrap();
    input.clear_frame();
}

fn core_system_read_window(world: WorldDataMut, window: &Window) {
    let window_res = world.into_resources_mut().into_get_mut::<WindowResource>().unwrap();

    let changes = WindowResource::take_changes(window_res);
    let state = WindowResource::state_mut(window_res);
    WindowState::apply_changes(state, changes);
    window_state_refresh_from_window(state, window);
}

fn core_system_update_window(world: WorldDataMut, window: &Window) {
    let window_res = world.into_resources_mut().into_get_mut::<WindowResource>().unwrap();

    let (state, changes) = WindowResource::state_and_changes_mut(window_res);
    window_state_apply_changes(state, changes, window);
}
