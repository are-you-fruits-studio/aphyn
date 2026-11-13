use aphyn_ecs::{
    TypesRegistryAccessFfi, TypesRegistryCache, WorldBehaviorBuilderFfi, WorldBehaviorBuilderMut, WorldDataMut, WorldDataUnsafeFfi,
};
use aphyn_render_features::RenderFeatureRegistry;
use aphyn_window::WindowState;

#[repr(C)]
pub struct AppInitCtxFfi {
    pub types_ref: *const TypesRegistryAccessFfi,
    pub data_mut: *mut WorldDataUnsafeFfi,
    pub behavior_mut: *mut WorldBehaviorBuilderFfi,
    pub window_mut: *mut WindowState,
    pub render_mut: *mut RenderFeatureRegistry,
    pub threading_mut: *mut ThreadingMode,
}

pub struct AppInitCtx<'a> {
    pub data: WorldDataMut<'a>,
    pub behavior: WorldBehaviorBuilderMut<'a>,
    pub window: &'a mut WindowState,
    pub render: &'a mut RenderFeatureRegistry,
    pub threading: &'a mut ThreadingMode,
}

impl<'a> AppInitCtx<'a> {
    pub unsafe fn into_ffi(self) -> AppInitCtxFfi {
        let AppInitCtx {
            data,
            behavior,
            window,
            render,
            threading,
        } = self;

        unsafe {
            let (behavior_ffi, types) = behavior.into_raw_parts();

            AppInitCtxFfi {
                types_ref: &raw const *types.registry(),
                data_mut: data.ffi(),
                behavior_mut: &raw mut *behavior_ffi,
                window_mut: &raw mut *window,
                render_mut: &raw mut *render,
                threading_mut: &raw mut *threading,
            }
        }
    }
}

impl AppInitCtxFfi {
    pub fn init_app(self, f: fn(AppInitCtx)) {
        let behavior = unsafe { &mut *self.behavior_mut };
        let types_ref = unsafe { &*self.types_ref };
        let window = unsafe { &mut *self.window_mut };
        let render = unsafe { &mut *self.render_mut };
        let threading = unsafe { &mut *self.threading_mut };

        let types = TypesRegistryCache::new(types_ref.clone());

        {
            let data = unsafe { WorldDataMut::new(self.data_mut, &types) };
            let behavior = WorldBehaviorBuilderMut::new(behavior, &types);

            let ctx = AppInitCtx {
                data,
                behavior,
                window,
                render,
                threading,
            };

            let _init_result: () = { f(ctx) };
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ThreadingMode {
    #[default]
    ThreadPool,
    SingleThreaded,
}
