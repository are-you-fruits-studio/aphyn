use aphyn_any::{AnyBoxCtx, AnyMut, AnyRef, AnyTypeId, TypesRegistryAccessFfi, TypesRegistryCache};
use aphyn_ffi::{FfiExtendedTypeInfo, FfiOption, FfiShortTypeInfo, FfiString};

type FnGetErased = for<'a> unsafe extern "C-unwind" fn(*const (), AnyRef<'a>, AnyTypeId, AnyTypeId) -> FfiOption<AnyRef<'a>>;
type FnGetMutErased = for<'a> unsafe extern "C-unwind" fn(*const (), AnyMut<'a>, AnyTypeId, AnyTypeId) -> FfiOption<AnyMut<'a>>;

// a reflected type: its info, and its id and registry once the reflection registry is built
#[repr(C)]
#[derive(Clone)]
pub(crate) struct ReflectType {
    pub(crate) info: &'static FfiExtendedTypeInfo,
    pub(crate) id: AnyTypeId,
    pub(crate) registry: FfiOption<TypesRegistryAccessFfi>,
}

impl ReflectType {
    pub(crate) fn of<T: 'static>() -> Self {
        Self {
            info: const { &FfiExtendedTypeInfo::of::<T>() },
            id: AnyTypeId::UNRESOLVED,
            registry: FfiOption::None,
        }
    }

    pub(crate) fn resolve(&mut self, types: &TypesRegistryCache) {
        self.id = types.any_type_id(self.info);
        self.registry = FfiOption::Some(types.ffi().clone());
    }

    pub(crate) fn box_ctx(&self) -> AnyBoxCtx<'_> {
        self.box_ctx_for(self.id)
    }

    // a ctx for a related type (element, key, value) whose id was resolved in the same registry
    pub(crate) fn box_ctx_for(&self, id: AnyTypeId) -> AnyBoxCtx<'_> {
        AnyBoxCtx {
            id,
            registry: self.registry.as_ref().into(),
        }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct ReflectField {
    name: FfiString,
    owner: &'static FfiExtendedTypeInfo,
    dst: &'static FfiExtendedTypeInfo,
    owner_id: AnyTypeId,
    dst_id: AnyTypeId,
    getter: *const (),
    getter_mut: *const (),
    fn_get: FnGetErased,
    fn_get_mut: FnGetMutErased,
}

unsafe impl Send for ReflectField {}
unsafe impl Sync for ReflectField {}

impl ReflectField {
    pub fn new<T: 'static, F: 'static>(name: &str, getter: fn(&T) -> Option<&F>, getter_mut: fn(&mut T) -> Option<&mut F>) -> Self {
        unsafe extern "C-unwind" fn ffi_get<'a, T: 'static, F: 'static>(
            getter: *const (),
            value: AnyRef<'a>,
            owner_id: AnyTypeId,
            dst_id: AnyTypeId,
        ) -> FfiOption<AnyRef<'a>> {
            let getter: fn(&T) -> Option<&F> = unsafe { std::mem::transmute(getter) };

            match unsafe { any_ref_into::<T>(value, owner_id) } {
                Some(value) => getter(value).map(|f| unsafe { AnyRef::from_typed(f, dst_id) }).into(),
                None => FfiOption::None,
            }
        }
        unsafe extern "C-unwind" fn ffi_get_mut<'a, T: 'static, F: 'static>(
            getter_mut: *const (),
            value: AnyMut<'a>,
            owner_id: AnyTypeId,
            dst_id: AnyTypeId,
        ) -> FfiOption<AnyMut<'a>> {
            let getter_mut: fn(&mut T) -> Option<&mut F> = unsafe { std::mem::transmute(getter_mut) };

            match unsafe { any_mut_into::<T>(value, owner_id) } {
                Some(value) => getter_mut(value).map(|f| unsafe { AnyMut::from_typed(f, dst_id) }).into(),
                None => FfiOption::None,
            }
        }

        Self {
            name: name.into(),
            owner: const { &FfiExtendedTypeInfo::of::<T>() },
            dst: const { &FfiExtendedTypeInfo::of::<F>() },
            owner_id: AnyTypeId::UNRESOLVED,
            dst_id: AnyTypeId::UNRESOLVED,
            getter: getter as *const (),
            getter_mut: getter_mut as *const (),
            fn_get: ffi_get::<T, F>,
            fn_get_mut: ffi_get_mut::<T, F>,
        }
    }

    pub(crate) fn resolved(&self, types: &TypesRegistryCache) -> Self {
        Self {
            owner_id: types.any_type_id(self.owner),
            dst_id: types.any_type_id(self.dst),
            ..self.clone()
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn owner_type_info(&self) -> &FfiShortTypeInfo {
        self.owner.short()
    }

    pub fn owner_extended_type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.owner
    }

    pub fn owner_type_name(&self) -> &'static str {
        self.owner.short().name()
    }

    pub fn owner_type_id(&self) -> AnyTypeId {
        self.owner_id
    }

    pub fn dst_type_info(&self) -> &FfiShortTypeInfo {
        self.dst.short()
    }

    pub fn dst_extended_type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.dst
    }

    pub fn dst_type_name(&self) -> &'static str {
        self.dst.short().name()
    }

    pub fn dst_type_id(&self) -> AnyTypeId {
        self.dst_id
    }

    pub fn get<'a>(&self, value: AnyRef<'a>) -> Option<AnyRef<'a>> {
        unsafe { (self.fn_get)(self.getter, value, self.owner_id, self.dst_id) }.into_option()
    }

    pub fn get_mut<'a>(&self, value: AnyMut<'a>) -> Option<AnyMut<'a>> {
        unsafe { (self.fn_get_mut)(self.getter_mut, value, self.owner_id, self.dst_id) }.into_option()
    }
}

impl std::fmt::Debug for ReflectField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReflectField")
            .field("name", &self.name())
            .field("owner", &self.owner_type_name())
            .field("dst", &self.dst_type_name())
            .finish()
    }
}

// safety: `id` is `T`'s id in its registry, or unresolved
pub(crate) unsafe fn any_ref_into<'a, T: 'static>(value: AnyRef<'a>, id: AnyTypeId) -> Option<&'a T> {
    unsafe { value.downcast_ref_by_id::<T>(id) }
}

// safety: `id` is `T`'s id in its registry, or unresolved
pub(crate) unsafe fn any_mut_into<'a, T: 'static>(value: AnyMut<'a>, id: AnyTypeId) -> Option<&'a mut T> {
    unsafe { value.into_downcast_mut_by_id::<T>(id) }
}
