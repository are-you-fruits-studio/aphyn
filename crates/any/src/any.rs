use std::{ffi::c_void, marker::PhantomData, mem::ManuallyDrop};

use aphyn_ffi::{FfiBox, FfiExtendedTypeInfo, FfiOption};

use crate::{RegistrySpecificTypeId, TypesRegistryAccessFfi, TypesRegistryCache, TypesRegistryTag};

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct AnyTypeId {
    tag: TypesRegistryTag,
    id: u64,
}

impl Default for AnyTypeId {
    fn default() -> Self {
        Self::UNRESOLVED
    }
}

impl AnyTypeId {
    // carries no registry: values with it are always compared by type info
    pub const UNRESOLVED: Self = Self {
        tag: TypesRegistryTag::NONE,
        id: 0,
    };

    pub const fn new(tag: TypesRegistryTag, id: u64) -> Self {
        Self { tag, id }
    }

    pub const fn tag(&self) -> TypesRegistryTag {
        self.tag
    }

    pub const fn id(&self) -> u64 {
        self.id
    }

    pub fn is_resolved(&self) -> bool {
        !self.tag.is_none()
    }

    pub fn same_registry(&self, other: &AnyTypeId) -> bool {
        self.is_resolved() && self.tag == other.tag
    }

    pub fn matches(&self, info: &FfiExtendedTypeInfo, other: &AnyTypeId, other_info: &FfiExtendedTypeInfo) -> bool {
        if self.same_registry(other) {
            return self.id == other.id;
        }

        info.short().matches(other_info.short())
    }

    pub fn matches_type<T: 'static>(&self, info: &FfiExtendedTypeInfo, id: RegistrySpecificTypeId<T>) -> bool {
        let other = id.any_type_id();

        // todo: should we allow multiple types registries to be mixed?
        if self.same_registry(&other) {
            return self.id == other.id;
        }

        info.short().does_match::<T>()
    }
}

// what a type-erased constructor needs to box a value: its id and the registry the id belongs to
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AnyBoxCtx<'r> {
    pub id: AnyTypeId,
    pub registry: FfiOption<&'r TypesRegistryAccessFfi>,
}

impl<'r> AnyBoxCtx<'r> {
    pub const UNRESOLVED: AnyBoxCtx<'static> = AnyBoxCtx {
        id: AnyTypeId::UNRESOLVED,
        registry: FfiOption::None,
    };

    pub fn new(id: AnyTypeId, registry: &'r TypesRegistryAccessFfi) -> Self {
        Self {
            id,
            registry: FfiOption::Some(registry),
        }
    }

    pub fn of(types: &'r TypesRegistryCache, info: &'static FfiExtendedTypeInfo) -> Self {
        Self::new(types.any_type_id(info), types.ffi())
    }
}

//

#[repr(C)]
pub struct AnyBox {
    ptr: *mut c_void,
    type_info: &'static FfiExtendedTypeInfo,
    id: AnyTypeId,
    registry: FfiOption<TypesRegistryAccessFfi>,
}

impl AnyBox {
    pub fn new<T: 'static>(value: T, types: &TypesRegistryCache) -> Self {
        let id = types.type_id_of::<T>();
        unsafe { Self::from_typed(value, AnyBoxCtx::new(id.any_type_id(), types.ffi())) }
    }

    // safety: `ctx.id` is `T`'s id in `ctx.registry`, or unresolved
    pub unsafe fn from_typed<T: 'static>(value: T, ctx: AnyBoxCtx) -> Self {
        Self {
            ptr: FfiBox::new(value).into_raw() as *mut c_void,
            type_info: const { &FfiExtendedTypeInfo::of::<T>() },
            id: ctx.id,
            registry: ctx.registry.as_ref().map(|r| (*r).clone()).into(),
        }
    }

    // moves the value out of `ptr` (the caller must not drop it afterwards)
    // safety: `ptr` holds a valid value of `type_info`'s type; `ctx.id` is its id or unresolved
    pub unsafe fn from_moved(ptr: *const c_void, type_info: &'static FfiExtendedTypeInfo, ctx: AnyBoxCtx) -> Self {
        Self {
            ptr: unsafe { type_info.move_to_box(ptr) },
            type_info,
            id: ctx.id,
            registry: ctx.registry.as_ref().map(|r| (*r).clone()).into(),
        }
    }

    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.type_info
    }

    pub const fn type_id(&self) -> AnyTypeId {
        self.id
    }

    pub fn registry(&self) -> Option<&TypesRegistryAccessFfi> {
        self.registry.as_ref()
    }

    pub fn box_ctx(&self) -> AnyBoxCtx<'_> {
        AnyBoxCtx {
            id: self.id,
            registry: self.registry.as_ref().into(),
        }
    }

    pub fn as_any_ref(&self) -> AnyRef<'_> {
        AnyRef {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
            _phantom: PhantomData,
        }
    }
    pub fn as_any_mut(&mut self) -> AnyMut<'_> {
        AnyMut {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
            _phantom: PhantomData,
        }
    }
    pub fn as_any_ptr(&self) -> AnyPtr {
        AnyPtr {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
        }
    }

    pub fn is<T: 'static>(&self, id: RegistrySpecificTypeId<T>) -> bool {
        self.id.matches_type(self.type_info, id)
    }

    pub unsafe fn dealloc_without_drop(self) {
        let mut this = ManuallyDrop::new(self);
        unsafe {
            this.type_info.dealloc_box(this.ptr);
            std::ptr::drop_in_place(&mut this.registry);
        }
    }

    pub fn downcast<T: 'static>(self, id: RegistrySpecificTypeId<T>) -> Result<T, AnyBox> {
        if !self.is(id) {
            return Err(self);
        }

        unsafe {
            let value = (self.ptr as *const T).read();
            self.dealloc_without_drop();
            Ok(value)
        }
    }
    pub fn downcast_ref<T: 'static>(&self, id: RegistrySpecificTypeId<T>) -> Option<&T> {
        self.is(id).then(|| unsafe { &*(self.ptr as *const T) })
    }
    pub fn downcast_mut<T: 'static>(&mut self, id: RegistrySpecificTypeId<T>) -> Option<&mut T> {
        self.is(id).then(|| unsafe { &mut *(self.ptr as *mut T) })
    }

    // for type-erased callers that know the expected id but have no `RegistrySpecificTypeId`
    // safety: `id` is `T`'s id in its registry, or unresolved
    pub unsafe fn is_by_id<T: 'static>(&self, id: AnyTypeId) -> bool {
        if self.id.same_registry(&id) {
            return self.id.id == id.id;
        }

        self.type_info.short().does_match::<T>()
    }

    // safety: `id` is `T`'s id in its registry, or unresolved
    pub unsafe fn downcast_by_id<T: 'static>(self, id: AnyTypeId) -> Result<T, AnyBox> {
        if !unsafe { self.is_by_id::<T>(id) } {
            return Err(self);
        }

        unsafe {
            let value = (self.ptr as *const T).read();
            self.dealloc_without_drop();
            Ok(value)
        }
    }
}

impl Drop for AnyBox {
    fn drop(&mut self) {
        struct DeallocOnDrop {
            ptr: *mut c_void,
            type_info: &'static FfiExtendedTypeInfo,
        }

        impl Drop for DeallocOnDrop {
            fn drop(&mut self) {
                unsafe { self.type_info.dealloc_box(self.ptr) };
            }
        }

        let to_dealloc = DeallocOnDrop {
            ptr: self.ptr,
            type_info: self.type_info,
        };

        unsafe { self.type_info.drop(self.ptr) };

        drop(to_dealloc);
    }
}

unsafe impl Send for AnyBox {}
unsafe impl Sync for AnyBox {}

//

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AnyRef<'a> {
    ptr: *mut c_void,
    type_info: &'static FfiExtendedTypeInfo,
    id: AnyTypeId,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> AnyRef<'a> {
    pub fn new<T: 'static>(value: &'a T, id: RegistrySpecificTypeId<T>) -> Self {
        unsafe { Self::from_typed(value, id.any_type_id()) }
    }

    // safety: `id` is `T`'s id in its registry, or unresolved
    pub unsafe fn from_typed<T: 'static>(value: &'a T, id: AnyTypeId) -> Self {
        Self {
            ptr: value as *const T as *mut c_void,
            type_info: const { &FfiExtendedTypeInfo::of::<T>() },
            id,
            _phantom: PhantomData,
        }
    }

    // safety: `ptr` points to a value of `type_info`'s type, valid for `'a`; `id` is its id or unresolved
    pub unsafe fn from_raw(ptr: *const c_void, type_info: &'static FfiExtendedTypeInfo, id: AnyTypeId) -> Self {
        Self {
            ptr: ptr as *mut c_void,
            type_info,
            id,
            _phantom: PhantomData,
        }
    }

    pub const fn ptr(&self) -> *const c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.type_info
    }

    pub const fn type_id(&self) -> AnyTypeId {
        self.id
    }

    pub fn as_any_ptr(&self) -> AnyPtr {
        AnyPtr {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
        }
    }

    pub fn is<T: 'static>(&self, id: RegistrySpecificTypeId<T>) -> bool {
        self.id.matches_type(self.type_info, id)
    }

    pub fn is_same_type(&self, other_id: &AnyTypeId, other_info: &FfiExtendedTypeInfo) -> bool {
        self.id.matches(self.type_info, other_id, other_info)
    }

    pub fn downcast_ref<T: 'static>(&self, id: RegistrySpecificTypeId<T>) -> Option<&'a T> {
        self.is(id).then(|| unsafe { &*(self.ptr as *const T) })
    }

    // for type-erased callers that know the expected id but have no `RegistrySpecificTypeId`
    // safety: `id` is `T`'s id in its registry, or unresolved
    pub unsafe fn downcast_ref_by_id<T: 'static>(&self, id: AnyTypeId) -> Option<&'a T> {
        let matches = if self.id.same_registry(&id) {
            self.id.id == id.id
        } else {
            self.type_info.short().does_match::<T>()
        };

        matches.then(|| unsafe { &*(self.ptr as *const T) })
    }
}

#[repr(C)]
pub struct AnyMut<'a> {
    ptr: *mut c_void,
    type_info: &'static FfiExtendedTypeInfo,
    id: AnyTypeId,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> AnyMut<'a> {
    pub fn new<T: 'static>(value: &'a mut T, id: RegistrySpecificTypeId<T>) -> Self {
        unsafe { Self::from_typed(value, id.any_type_id()) }
    }

    // safety: `id` is `T`'s id in its registry, or unresolved
    pub unsafe fn from_typed<T: 'static>(value: &'a mut T, id: AnyTypeId) -> Self {
        Self {
            ptr: value as *mut T as *mut c_void,
            type_info: const { &FfiExtendedTypeInfo::of::<T>() },
            id,
            _phantom: PhantomData,
        }
    }

    // safety: `ptr` points to a value of `type_info`'s type, uniquely borrowed for `'a`; `id` is its id or unresolved
    pub unsafe fn from_raw(ptr: *mut c_void, type_info: &'static FfiExtendedTypeInfo, id: AnyTypeId) -> Self {
        Self {
            ptr,
            type_info,
            id,
            _phantom: PhantomData,
        }
    }

    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.type_info
    }

    pub const fn type_id(&self) -> AnyTypeId {
        self.id
    }

    pub fn as_any_ref<'r>(&'r self) -> AnyRef<'r>
    where
        'a: 'r,
    {
        AnyRef {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
            _phantom: PhantomData,
        }
    }
    pub fn as_any_ptr(&self) -> AnyPtr {
        AnyPtr {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
        }
    }

    pub fn reborrow<'r>(&'r mut self) -> AnyMut<'r>
    where
        'a: 'r,
    {
        AnyMut {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
            _phantom: PhantomData,
        }
    }

    pub fn into_any_ref(self) -> AnyRef<'a> {
        AnyRef {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
            _phantom: PhantomData,
        }
    }

    // Err gives the value back when its type differs; Ok holds the previous value
    pub fn replace(&mut self, value: AnyBox) -> Result<AnyBox, AnyBox> {
        if !self.id.matches(self.type_info, &value.id, value.type_info) {
            return Err(value);
        }

        unsafe {
            std::ptr::swap_nonoverlapping(
                self.ptr as *mut u8,
                value.ptr as *mut u8,
                self.type_info.short().size() as usize,
            );
        }

        Ok(value)
    }

    pub fn is<T: 'static>(&self, id: RegistrySpecificTypeId<T>) -> bool {
        self.id.matches_type(self.type_info, id)
    }

    pub fn downcast_ref<T: 'static>(&self, id: RegistrySpecificTypeId<T>) -> Option<&T> {
        self.is(id).then(|| unsafe { &*(self.ptr as *const T) })
    }
    pub fn downcast_mut<T: 'static>(&mut self, id: RegistrySpecificTypeId<T>) -> Option<&mut T> {
        self.is(id).then(|| unsafe { &mut *(self.ptr as *mut T) })
    }
    pub fn into_downcast_mut<T: 'static>(self, id: RegistrySpecificTypeId<T>) -> Option<&'a mut T> {
        self.is(id).then(|| unsafe { &mut *(self.ptr as *mut T) })
    }

    // safety: `id` is `T`'s id in its registry, or unresolved
    pub unsafe fn into_downcast_mut_by_id<T: 'static>(self, id: AnyTypeId) -> Option<&'a mut T> {
        let matches = if self.id.same_registry(&id) {
            self.id.id == id.id
        } else {
            self.type_info.short().does_match::<T>()
        };

        matches.then(|| unsafe { &mut *(self.ptr as *mut T) })
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AnyPtr {
    ptr: *mut c_void,
    type_info: &'static FfiExtendedTypeInfo,
    id: AnyTypeId,
}

impl AnyPtr {
    pub fn new<T: 'static>(value: *mut T, id: RegistrySpecificTypeId<T>) -> Self {
        Self {
            ptr: value as *mut c_void,
            type_info: const { &FfiExtendedTypeInfo::of::<T>() },
            id: id.any_type_id(),
        }
    }

    // safety: `ptr` points to a value of `type_info`'s type, uniquely borrowed for `'a`; `id` is its id or unresolved
    pub unsafe fn from_raw(ptr: *mut c_void, type_info: &'static FfiExtendedTypeInfo, id: AnyTypeId) -> Self {
        Self { ptr, type_info, id }
    }

    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.type_info
    }

    pub const fn type_id(&self) -> AnyTypeId {
        self.id
    }

    pub unsafe fn as_any_ref<'r>(&self) -> AnyRef<'r> {
        AnyRef {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
            _phantom: PhantomData,
        }
    }
    pub unsafe fn as_any_mut<'r>(&self) -> AnyMut<'r> {
        AnyMut {
            ptr: self.ptr,
            type_info: self.type_info,
            id: self.id,
            _phantom: PhantomData,
        }
    }

    pub fn downcast_ptr<T: 'static>(&self, id: RegistrySpecificTypeId<T>) -> Option<*mut T> {
        self.id.matches_type(self.type_info, id).then(|| self.ptr as *mut T)
    }
}
