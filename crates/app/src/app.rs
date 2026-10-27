use aphyn_ecs::WorldBuilder;
use aphyn_window::WindowState;
use winit::event_loop::EventLoop;

use crate::{AppInitCtx, event_loop_handler::EventLoopHandler};

pub struct App {
    ecs_world: WorldBuilder,
    window_state: WindowState,
}

impl App {
    pub fn new() -> Self {
        Self {
            ecs_world: WorldBuilder::new(),
            window_state: WindowState::default(),
        }
    }

    pub fn ecs(&self) -> &WorldBuilder {
        &self.ecs_world
    }

    pub fn ecs_mut(&mut self) -> &mut WorldBuilder {
        &mut self.ecs_world
    }

    pub fn init_ctx_mut(&mut self) -> AppInitCtx<'_> {
        AppInitCtx {
            world: self.ecs_world.as_mut(),
            window: &mut self.window_state,
        }
    }

    pub fn run(self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        event_loop.run_app(&mut EventLoopHandler::new(self.ecs_world, self.window_state)).unwrap();
    }
}
