use std::sync::Arc;

use fruits_ecs::{ResourcesHolderMut, Schedule, World, WorldBuilder, WorldDataMut};
use fruits_render::CameraComponent;
use fruits_render_core::{CurrentRenderingResource, RenderApiResource};
use fruits_transform::GlobalTransform;
use gilrs::Gilrs;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::PhysicalKey,
    window::{Window, WindowAttributes, WindowId},
};

use crate::{GamepadInputStorage, InputResource, TextInputEvent, WindowResource, WindowState, render_target::RenderTarget};

enum EventLoopHandlerState {
    Created(WorldBuilder),
    Starting,
    Polling {
        world: World,
        window: Arc<Window>,
        gamepad_host: Option<Gilrs>,
        render_target: RenderTarget,
    },
}

impl EventLoopHandlerState {
    fn try_start_creating(&mut self) -> Option<WorldBuilder> {
        let EventLoopHandlerState::Created { .. } = self else {
            return None;
        };

        let EventLoopHandlerState::Created(world) = std::mem::replace(self, EventLoopHandlerState::Starting) else {
            return None;
        };

        Some(world)
    }
}

pub struct EventLoopHandler(EventLoopHandlerState);

impl EventLoopHandler {
    pub fn new(world: WorldBuilder) -> Self {
        Self(EventLoopHandlerState::Created(world))
    }
}

impl ApplicationHandler for EventLoopHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(mut world) = self.0.try_start_creating() else {
            return;
        };

        let mut window_attr = WindowAttributes::default();
        window_attr.title = "fruits_engine app".into();

        let window = Arc::new(event_loop.create_window(window_attr).unwrap());

        let state = RenderApiResource::new(Arc::clone(&window));

        let render_target = RenderTarget::new(&*unsafe { state.raw() });

        let window_state = WindowState::from_window(&window);

        world.data_mut().resources_mut().insert(state);
        world.data_mut().resources_mut().insert(InputResource::new());
        world.data_mut().resources_mut().insert(WindowResource::new(window_state));
        let mut world = world.build();
        world.execute_iteration(Schedule::Start);

        let gamepad_host = Gilrs::new();

        if let Err(err) = &gamepad_host {
            eprintln!("ayf: Failed to init Gilrs(gamepad). {}", err);
        }

        self.0 = EventLoopHandlerState::Polling {
            world,
            window,
            gamepad_host: gamepad_host.ok(),
            render_target,
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
            world,
            window,
            gamepad_host,
            render_target,
        } = &mut self.0
        else {
            return;
        };

        let world_data = world.data_mut();
        let (mut res, _ent, mut evt) = world_data.into_tuple_mut();

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

                world.execute_iteration(Schedule::Update);

                core_system_render(world, render_target);

                core_system_clear_events_and_input(world.data_mut());
                core_system_update_window(world.data_mut(), window);

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
                gilrs::EventType::AxisChanged(axis, axis_value, _) => get_or_create_gamepad(input, gamepad_id).write_axis(axis, axis_value),
                // todo: handle other gamepad events
                _ => (),
            }
        }
    }
}

fn core_system_render(world: &mut World, render_target: &mut RenderTarget) {
    let render_state = unsafe { world.data_mut().as_ref().resources().get::<RenderApiResource>().unwrap().raw() };

    if render_target.size() != render_state.size() {
        *render_target = RenderTarget::new(&render_state);
    }

    render_target.clear(&render_state);

    let camera = world
        .data_mut()
        .as_ref()
        .entities()
        .query::<(&GlobalTransform, &CameraComponent)>()
        .iter()
        .next()
        .map(|(transform, camera)| (*transform, camera.0));

    if let Some((camera_transform, camera)) = camera {
        world.data_mut().resources_mut().insert(CurrentRenderingResource {
            camera_position: camera_transform.position,
            camera_scale_rotation: camera_transform.scale_rotation,
            camera,
            light: None,
            target_texture: render_target.texture.clone(),
            target_view: render_target.texture_view.clone(),
        });

        world.execute_iteration(Schedule::RenderCamera);

        world.data_mut().resources_mut().remove::<CurrentRenderingResource>();
    }

    render_target.render_to_surface(&render_state);
}

fn core_system_clear_events_and_input(mut world: WorldDataMut) {
    world.events_mut().clear();

    let mut res = world.into_resources_mut();

    let input = res.get_mut::<InputResource>().unwrap();
    input.clear_frame();
}

fn core_system_update_window(world: WorldDataMut, window: &Window) {
    let window_res = world.into_resources_mut().into_get_mut::<WindowResource>().unwrap();

    let window_state_next = window_res.next_state().clone();
    let window_state_prev = std::mem::replace(window_res.prev_state_mut(), window_state_next.clone());

    WindowState::apply_difference(&window_state_prev, &window_state_next, &window);
}