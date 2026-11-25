use aphyn_ecs::{ThreadPool, TypesRegistryAccessFfi, TypesRegistryCache, WorldBehaviorBuilder, WorldData};
use aphyn_reflection::ReflectionRegistryBuilder;
use aphyn_reflection_ecs::ReflectionRegistryResource;
use aphyn_render_features::RenderFeatureRegistry;
use aphyn_serialization::TransSerializerRegistryBuilder;
use aphyn_serialization_ecs::SerializersResource;
use aphyn_window::WindowState;
use winit::event_loop::EventLoop;

use crate::{AppInitCtx, ThreadingMode, event_loop_handler::EventLoopHandler};

pub struct App {
    types: TypesRegistryCache,
    data: WorldData,
    behavior: WorldBehaviorBuilder,
    window_state: WindowState,
    render: RenderFeatureRegistry,
    threading: ThreadingMode,
    serializers: TransSerializerRegistryBuilder,
    reflection: ReflectionRegistryBuilder,
}

impl App {
    pub fn new() -> Self {
        let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        let data = WorldData::new(types.clone());
        let behavior = WorldBehaviorBuilder::new(types.clone());

        Self {
            types,
            data,
            behavior,
            window_state: WindowState::default(),
            render: RenderFeatureRegistry::default(),
            threading: ThreadingMode::default(),
            serializers: TransSerializerRegistryBuilder::new(),
            reflection: ReflectionRegistryBuilder::new(),
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
            types: &self.types,
            behavior: self.behavior.as_mut(),
            window: &mut self.window_state,
            render: &mut self.render,
            threading: &mut self.threading,
            serializers: &mut self.serializers,
            reflection: &mut self.reflection,
        }
    }

    pub fn run(mut self) {
        {
            let serializers = std::mem::take(&mut self.serializers).build();
            let reflection = std::mem::take(&mut self.reflection).build();

            let mut res = self.data.resources_mut();
            res.insert(SerializersResource(serializers));
            res.insert(ReflectionRegistryResource(reflection));
        }

        let thread_pool = match self.threading {
            ThreadingMode::ThreadPool => Some(ThreadPool::default()),
            ThreadingMode::SingleThreaded => None,
        };

        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        event_loop
            .run_app(&mut EventLoopHandler::new(
                self.data,
                self.behavior,
                self.window_state,
                self.render,
                thread_pool,
            ))
            .unwrap();
    }
}
