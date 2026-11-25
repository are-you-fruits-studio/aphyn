use aphyn_ecs::{
    TypesRegistryAccessFfi, TypesRegistryCache, WorldBehaviorBuilderFfi, WorldBehaviorBuilderMut, WorldDataMut, WorldDataUnsafeFfi,
};
use aphyn_reflection::ReflectionRegistryBuilder;
use aphyn_render_features::RenderFeatureRegistry;
use aphyn_serialization::TransSerializerRegistryBuilder;
use aphyn_window::WindowState;

#[repr(C)]
pub struct AppInitCtxFfi {
    pub types_ref: *const TypesRegistryAccessFfi,
    pub data_mut: *mut WorldDataUnsafeFfi,
    pub behavior_mut: *mut WorldBehaviorBuilderFfi,
    pub window_mut: *mut WindowState,
    pub render_mut: *mut RenderFeatureRegistry,
    pub threading_mut: *mut ThreadingMode,
    pub serializers_mut: *mut TransSerializerRegistryBuilder,
    pub reflection_mut: *mut ReflectionRegistryBuilder,
}

pub struct AppInitCtx<'a> {
    pub data: WorldDataMut<'a>,
    pub types: &'a TypesRegistryCache,
    pub behavior: WorldBehaviorBuilderMut<'a>,
    pub window: &'a mut WindowState,
    pub render: &'a mut RenderFeatureRegistry,
    pub threading: &'a mut ThreadingMode,
    pub serializers: &'a mut TransSerializerRegistryBuilder,
    pub reflection: &'a mut ReflectionRegistryBuilder,
}

pub struct FeatureInitCtx<'a> {
    pub data: WorldDataMut<'a>,
    pub types: &'a TypesRegistryCache,
    pub behavior: WorldBehaviorBuilderMut<'a>,
    pub render: &'a mut RenderFeatureRegistry,
    pub serializers: &'a mut TransSerializerRegistryBuilder,
    pub reflection: &'a mut ReflectionRegistryBuilder,
}

impl<'a> FeatureInitCtx<'a> {
    pub fn as_mut(&mut self) -> FeatureInitCtx<'_> {
        FeatureInitCtx {
            data: self.data.as_mut(),
            types: self.types,
            behavior: self.behavior.as_mut(),
            render: self.render,
            serializers: self.serializers,
            reflection: self.reflection,
        }
    }
}

impl<'a> AppInitCtx<'a> {
    pub fn feature_ctx(&mut self) -> FeatureInitCtx<'_> {
        FeatureInitCtx {
            data: self.data.as_mut(),
            types: self.types,
            behavior: self.behavior.as_mut(),
            render: self.render,
            serializers: self.serializers,
            reflection: self.reflection,
        }
    }

    pub unsafe fn into_ffi(self) -> AppInitCtxFfi {
        let AppInitCtx {
            data,
            types: _,
            behavior,
            window,
            render,
            threading,
            serializers,
            reflection,
        } = self;

        unsafe {
            let (behavior_ffi, types) = behavior.into_raw_parts();

            AppInitCtxFfi {
                types_ref: &raw const *types.ffi(),
                data_mut: data.ffi(),
                behavior_mut: &raw mut *behavior_ffi,
                window_mut: &raw mut *window,
                render_mut: &raw mut *render,
                threading_mut: &raw mut *threading,
                serializers_mut: &raw mut *serializers,
                reflection_mut: &raw mut *reflection,
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
        let serializers = unsafe { &mut *self.serializers_mut };
        let reflection = unsafe { &mut *self.reflection_mut };

        let types = TypesRegistryCache::new(types_ref.clone());

        {
            let data = unsafe { WorldDataMut::new(self.data_mut, &types) };
            let behavior = WorldBehaviorBuilderMut::new(behavior, &types);

            let ctx = AppInitCtx {
                data,
                types: &types,
                behavior,
                window,
                render,
                threading,
                serializers,
                reflection,
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
