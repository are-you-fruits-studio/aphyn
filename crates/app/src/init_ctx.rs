use aphyn_ecs::{TypesRegistryAccessFfi, TypesRegistryCache, WorldBuilderMut, WorldBuilderUnsafeFfi};
use aphyn_window::WindowState;

#[repr(C)]
pub struct AppInitCtxFfi {
    pub types_ref: *const TypesRegistryAccessFfi,
    pub world_mut: *mut WorldBuilderUnsafeFfi,
    pub window_mut: *mut WindowState,
}

pub struct AppInitCtx<'a> {
    pub world: WorldBuilderMut<'a>,
    pub window: &'a mut WindowState,
}

impl<'a> AppInitCtx<'a> {
    pub unsafe fn into_ffi(self) -> AppInitCtxFfi {
        let AppInitCtx { mut world, window } = self;

        unsafe {
            let (world_builder_ffi, types) = world.into_raw_parts();

            AppInitCtxFfi {
                types_ref: &raw const *types.registry(),
                world_mut: &raw mut *world_builder_ffi,
                window_mut: &raw mut *window,
            }
        }
    }
}

impl AppInitCtxFfi {
    pub fn init_app(self, f: fn(AppInitCtx)) {
        let world = unsafe { &mut *self.world_mut };
        let types_ref = unsafe { &*self.types_ref };
        let window = unsafe { &mut *self.window_mut };

        let types = TypesRegistryCache::new(types_ref.clone());

        {
            let world = WorldBuilderMut::new(world, &types);

            let ctx = AppInitCtx {
                world,
                window,
            };

            let _init_result: () = { f(ctx) };
        }
    }
}
