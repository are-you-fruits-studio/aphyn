use std::{ffi::c_void, fmt::Debug, marker::PhantomData};

use fruits_ffi::{FfiDroppable, FfiString};

use crate::*;

pub struct SystemInput<'a> {
    pub world_data: WorldDataRef<'a>,
    pub system_data: SystemResourcesHolderRef<'a>,
}

//

/// # Safety
///
/// Implemented automatically.
pub unsafe trait SystemParam {
    type Item<'e>: 'e + SystemParam;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache);
    /// # Safety
    ///
    /// Should be managed by system scheduler and data usage.
    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str>;
}

/// # Safety
///
/// Implemented automatically.
pub unsafe trait SystemWithMarker<M: 'static>: 'static + Send + Sync {
    fn fill_data_usage(&self, usage: &mut DataUsageBuilder, types: &TypesRegistryCache);
    /// # Safety
    ///
    /// Should be managed by system scheduler and data usage.
    unsafe fn execute<'e>(&self, data: SystemInput<'e>);
    fn system_name(&self) -> &'static str;
}

//

macro_rules! system_with_marker_impl {
    ($($P: ident),*) => {
        unsafe impl<F, R, $($P),*> SystemWithMarker<fn($($P),*)> for F
        where
            F: 'static + Send + Sync + Fn($($P),*) -> R + for<'a> Fn($($P::Item<'a>),*) -> R,
            fn($($P),*): 'static,
            $($P: SystemParam),*
        {
            #[allow(redundant_semicolons)]
            fn fill_data_usage(&self, _usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
                $($P::fill_data_usage(_usage, _types));*;
            }

            unsafe fn execute<'e>(&self, _data: SystemInput<'e>) {
                self(
                    // Safety. Managed by caller.
                    $(unsafe { get_param_or_panic::<F, $P>(&_data) },)*
                );
            }

            fn system_name(&self) -> &'static str {
                std::any::type_name::<F>()
            }
        }
    };
}

fn panic_cannot_obtain_param<F, P>(msg: &'static str) -> ! {
    panic!(
        "System cannot obtain its parameters. System: {}. Parameter: {}. Message: {}",
        std::any::type_name::<F>(),
        std::any::type_name::<P>(),
        msg,
    )
}

unsafe fn get_param_or_panic<'e, F, P: SystemParam>(data: &'e SystemInput<'e>) -> P::Item<'e> {
    // Safety. Managed by caller.
    unsafe { P::new(data) }.unwrap_or_else(|m| panic_cannot_obtain_param::<F, P>(m))
}

system_with_marker_impl!();
system_with_marker_impl!(P0);
system_with_marker_impl!(P0, P1);
system_with_marker_impl!(P0, P1, P2);
system_with_marker_impl!(P0, P1, P2, P3);
system_with_marker_impl!(P0, P1, P2, P3, P4);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13);
system_with_marker_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14);

//

struct SystemNativeData<S: SystemWithMarker<M>, M: 'static> {
    system: S,
    types: TypesRegistryCache,
    _phantom: PhantomData<fn(M) -> M>,
}

#[repr(C)]
pub struct SystemFfi {
    system_name: FfiString,
    data_usage: DataUsage,
    system_native_data: FfiDroppable,
    execute_fn: unsafe extern "C-unwind" fn(*const c_void, SystemCtxFfi),
}

impl SystemFfi {
    pub fn new<S: SystemWithMarker<M>, M: 'static>(system: S, types: TypesRegistryCache) -> Self {
        unsafe extern "C-unwind" fn ffi_execute<S: SystemWithMarker<M>, M: 'static>(system_native_data: *const c_void, ctx: SystemCtxFfi) {
            unsafe {
                let system_native_data = &*(system_native_data as *const SystemNativeData<S, M>);

                let system_data = &mut *ctx.system_data;
                let types = &system_native_data.types;

                let system_input = SystemInput {
                    world_data: WorldDataRef::new(ctx.world_mut, types),
                    system_data: SystemResourcesHolderRef::new(system_data, types),
                };

                system_native_data.system.execute(system_input);
            }
        }

        let mut data_usage = DataUsageBuilder::new();

        system.fill_data_usage(&mut data_usage, &types);
        
        let Some(data_usage) = data_usage.build() else {
            panic!("fruits: Invalid system DataUsage: {}. {data_usage:?}", std::any::type_name::<S>());
        };

        Self {
            data_usage,
            execute_fn: ffi_execute::<S, M>,
            system_native_data: FfiDroppable::new(SystemNativeData::<S, M> {
                system,
                types,
                _phantom: PhantomData,
            }),
            system_name: FfiString::from_string(String::from(std::any::type_name::<S>())),
        }
    }

    pub fn data_usage(&self) -> &DataUsage {
        &self.data_usage
    }
    // todo:

    // /// # Safety
    // ///
    // /// Should be managed by system scheduler and data usage.
    // pub unsafe fn execute<'e>(&self, data: &SystemInput<'e>) {

    // }

    pub unsafe fn execute(&self, ctx: SystemCtxFfi) {
        unsafe { (self.execute_fn)(self.system_native_data.get(), ctx) }
    }

    pub fn system_name(&self) -> &FfiString {
        &self.system_name
    }
}

impl Debug for SystemFfi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemFfi")
            .field("system_name", &self.system_name)
            .field("data_usage", &self.data_usage)
            .finish()
    }
}

unsafe impl Send for SystemFfi {}
unsafe impl Sync for SystemFfi {}

// todo
#[repr(C)]
#[derive(Copy, Clone)]
pub struct SystemCtxFfi {
    pub world_mut: *mut WorldDataUnsafeFfi,
    pub system_data: *mut SystemResourcesHolderUnsafeFfi,
}