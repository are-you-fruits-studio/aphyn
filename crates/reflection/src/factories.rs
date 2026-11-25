use std::marker::PhantomData;

use aphyn_any::{AnyBox, AnyBoxCtx, AnyRef, AnyTypeId};
use aphyn_ffi::FfiOption;

use crate::any_ref_into;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ErasedFn<F> {
    pub(crate) func: *const (),
    pub(crate) trampoline: F,
}

type FnDefaultErased = unsafe extern "C-unwind" fn(*const (), AnyBoxCtx) -> AnyBox;
type FnCloneErased = unsafe extern "C-unwind" fn(*const (), AnyRef, AnyBoxCtx) -> FfiOption<AnyBox>;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ReflectFactories {
    default: FfiOption<ErasedFn<FnDefaultErased>>,
    clone: FfiOption<ErasedFn<FnCloneErased>>,
}

unsafe impl Send for ReflectFactories {}
unsafe impl Sync for ReflectFactories {}

impl ReflectFactories {
    pub(crate) fn none() -> Self {
        Self {
            default: FfiOption::None,
            clone: FfiOption::None,
        }
    }

    pub(crate) fn set_default<T: 'static>(&mut self, default: fn() -> T) {
        unsafe extern "C-unwind" fn ffi_default<T: 'static>(default: *const (), ctx: AnyBoxCtx) -> AnyBox {
            let default: fn() -> T = unsafe { std::mem::transmute(default) };

            unsafe { AnyBox::from_typed(default(), ctx) }
        }

        self.default = FfiOption::Some(ErasedFn {
            func: default as *const (),
            trampoline: ffi_default::<T>,
        });
    }

    pub(crate) fn set_clone<T: 'static>(&mut self, clone: fn(&T) -> T) {
        unsafe extern "C-unwind" fn ffi_clone<T: 'static>(clone: *const (), value: AnyRef, ctx: AnyBoxCtx) -> FfiOption<AnyBox> {
            let clone: fn(&T) -> T = unsafe { std::mem::transmute(clone) };

            unsafe { any_ref_into::<T>(value, ctx.id) }
                .map(|v| unsafe { AnyBox::from_typed(clone(v), ctx) })
                .into()
        }

        self.clone = FfiOption::Some(ErasedFn {
            func: clone as *const (),
            trampoline: ffi_clone::<T>,
        });
    }

    pub(crate) fn set_optional<T: 'static>(&mut self, default: Option<fn() -> T>, clone: Option<fn(&T) -> T>) {
        if let Some(default) = default {
            self.set_default(default);
        }
        if let Some(clone) = clone {
            self.set_clone(clone);
        }
    }

    pub(crate) fn has_default(&self) -> bool {
        self.default.is_some()
    }

    pub(crate) fn has_clone(&self) -> bool {
        self.clone.is_some()
    }

    // `ctx` must be the reflected type's own ctx
    pub(crate) fn default_value(&self, ctx: AnyBoxCtx) -> Option<AnyBox> {
        match self.default {
            FfiOption::Some(f) => Some(unsafe { (f.trampoline)(f.func, ctx) }),
            FfiOption::None => None,
        }
    }

    // `ctx` must be the reflected type's own ctx
    pub(crate) fn clone_value(&self, value: AnyRef, ctx: AnyBoxCtx) -> Option<AnyBox> {
        match self.clone {
            FfiOption::Some(f) => unsafe { (f.trampoline)(f.func, value, ctx) }.into_option(),
            FfiOption::None => None,
        }
    }
}

// autoref specialization used by the derive (through the `__reflect_*` macros); in generic code the bounds
// are unprovable, so it resolves to `None`
#[doc(hidden)]
pub struct ReflectProbe<T>(PhantomData<fn() -> T>);

impl<T> ReflectProbe<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

#[doc(hidden)]
pub trait ReflectProbeDefault<T> {
    fn probe_default(&self) -> Option<fn() -> T>;
}

impl<T: Default> ReflectProbeDefault<T> for &ReflectProbe<T> {
    fn probe_default(&self) -> Option<fn() -> T> {
        Some(T::default)
    }
}

#[doc(hidden)]
pub trait ReflectProbeDefaultFallback<T> {
    fn probe_default(&self) -> Option<fn() -> T>;
}

impl<T> ReflectProbeDefaultFallback<T> for ReflectProbe<T> {
    fn probe_default(&self) -> Option<fn() -> T> {
        None
    }
}

#[doc(hidden)]
pub trait ReflectProbeClone<T> {
    fn probe_clone(&self) -> Option<fn(&T) -> T>;
}

impl<T: Clone> ReflectProbeClone<T> for &ReflectProbe<T> {
    fn probe_clone(&self) -> Option<fn(&T) -> T> {
        Some(T::clone)
    }
}

#[doc(hidden)]
pub trait ReflectProbeCloneFallback<T> {
    fn probe_clone(&self) -> Option<fn(&T) -> T>;
}

impl<T> ReflectProbeCloneFallback<T> for ReflectProbe<T> {
    fn probe_clone(&self) -> Option<fn(&T) -> T> {
        None
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __reflect_default_fn {
    ($t: ty) => {{
        #[allow(unused_imports)]
        use $crate::{ReflectProbeDefault as _, ReflectProbeDefaultFallback as _};
        (&&$crate::ReflectProbe::<$t>::new()).probe_default()
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __reflect_clone_fn {
    ($t: ty) => {{
        #[allow(unused_imports)]
        use $crate::{ReflectProbeClone as _, ReflectProbeCloneFallback as _};
        (&&$crate::ReflectProbe::<$t>::new()).probe_clone()
    }};
}

// the field values handed to an enum variant constructor, in field order, with each field's resolved id
pub struct ReflectVariantFields<'a> {
    values: std::vec::IntoIter<AnyBox>,
    ids: std::slice::Iter<'a, AnyTypeId>,
}

impl<'a> ReflectVariantFields<'a> {
    pub(crate) fn new(values: Vec<AnyBox>, ids: &'a [AnyTypeId]) -> Self {
        Self {
            values: values.into_iter(),
            ids: ids.iter(),
        }
    }

    // None when the values ran out or the next one is not a `T`
    // safety: `T` is the type the variant declared for the next field
    pub unsafe fn take<T: 'static>(&mut self) -> Option<T> {
        let value = self.values.next()?;
        let id = *self.ids.next()?;

        unsafe { value.downcast_by_id::<T>(id) }.ok()
    }
}
