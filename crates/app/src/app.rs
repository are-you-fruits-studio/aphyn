use aphyn_ecs::{WorldBehaviorBuilder, WorldData};
use aphyn_render_features::RenderFeatureRegistry;
use aphyn_window::WindowState;
use winit::event_loop::EventLoop;

use crate::{AppInitCtx, event_loop_handler::EventLoopHandler};

pub struct App {
    data: WorldData,
    behavior: WorldBehaviorBuilder,
    window_state: WindowState,
    render: RenderFeatureRegistry,
}

impl App {
    pub fn new() -> Self {
        let data = WorldData::default();
        let behavior = WorldBehaviorBuilder::new(data.types().clone());

        Self {
            data,
            behavior,
            window_state: WindowState::default(),
            render: RenderFeatureRegistry::default(),
        }
    }

    pub fn data_mut(&mut self) -> &mut WorldData {
        &mut self.data
    }

    pub fn behavior_mut(&mut self) -> &mut WorldBehaviorBuilder {
        &mut self.behavior
    }

    pub fn render_mut(&mut self) -> &mut RenderFeatureRegistry {
        &mut self.render
    }

    pub fn init_ctx_mut(&mut self) -> AppInitCtx<'_> {
        AppInitCtx {
            data: self.data.as_mut(),
            behavior: self.behavior.as_mut(),
            window: &mut self.window_state,
            render: &mut self.render,
        }
    }

    pub fn run(self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        event_loop
            .run_app(&mut EventLoopHandler::new(
                self.data,
                self.behavior,
                self.window_state,
                self.render,
            ))
            .unwrap();
    }
}
