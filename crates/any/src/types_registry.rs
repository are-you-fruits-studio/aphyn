use std::{
    any::TypeId,
    collections::HashMap,
    ffi::c_void,
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use aphyn_ffi::{FfiExtendedTypeInfo, FfiOption, FfiStrSliceRef};

use crate::AnyTypeId;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StoredTypeInfo {
    pub id: u64,
    pub data: &'static FfiExtendedTypeInfo,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct TypesRegistryTag(*const c_void);

impl TypesRegistryTag {
    pub const NONE: Self = Self(std::ptr::null());

    pub fn is_none(&self) -> bool {
        self.0.is_null()
    }
}

unsafe impl Send for TypesRegistryTag {}
unsafe impl Sync for TypesRegistryTag {}

//

#[repr(C)]
struct TypesRegistryAccessVtable {
    pub clone_fn: unsafe extern "C-unwind" fn(*const c_void),
    pub len_fn: unsafe extern "C-unwind" fn(*const c_void) -> u64,
    pub get_fn: unsafe extern "C-unwind" fn(*const c_void, id: u64) -> FfiOption<&'static FfiExtendedTypeInfo>,
    pub get_by_name_fn: unsafe extern "C-unwind" fn(*const c_void, name: FfiStrSliceRef) -> FfiOption<StoredTypeInfo>,
    pub try_register_fn: unsafe extern "C-unwind" fn(*const c_void, data: &'static FfiExtendedTypeInfo) -> FfiOption<u64>,
    pub drop_fn: unsafe extern "C-unwind" fn(*const c_void),
}

// a refcounted handle to the shared registry state; the state's address is the registry's tag
#[repr(C)]
pub struct TypesRegistryAccessFfi {
    data: *const c_void,
    vtable: &'static TypesRegistryAccessVtable,
}

type TypesRegistryShared = RwLock<TypesRegistryData>;

impl TypesRegistryAccessFfi {
    pub fn new() -> Self {
        let shared: Arc<TypesRegistryShared> = Arc::new(RwLock::new(TypesRegistryData {
            types_by_string: HashMap::new(),
            types: Vec::new(),
        }));

        Self {
            data: Arc::into_raw(shared) as *const c_void,
            vtable: &TypesRegistryAccessVtable {
                clone_fn: Self::ffi_clone,
                len_fn: Self::ffi_len,
                get_fn: Self::ffi_get,
                get_by_name_fn: Self::ffi_get_by_name,
                try_register_fn: Self::ffi_try_register,
                drop_fn: Self::ffi_drop,
            },
        }
    }

    pub fn tag(&self) -> TypesRegistryTag {
        TypesRegistryTag(self.data)
    }

    pub fn len(&self) -> u64 {
        unsafe { (self.vtable.len_fn)(self.data) }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, id: u64) -> Option<&'static FfiExtendedTypeInfo> {
        unsafe { (self.vtable.get_fn)(self.data, id).into_option() }
    }

    pub fn get_by_name(&self, name: &str) -> Option<StoredTypeInfo> {
        unsafe { (self.vtable.get_by_name_fn)(self.data, FfiStrSliceRef::from_slice(name)).into_option() }
    }

    pub fn try_register(&self, data: &'static FfiExtendedTypeInfo) -> Option<u64> {
        unsafe { (self.vtable.try_register_fn)(self.data, data).into_option() }
    }

    pub fn get_or_register(&self, data: &'static FfiExtendedTypeInfo) -> u64 {
        if let Some(id) = self.get_by_name(data.short().name()) {
            return id.id;
        }

        self.try_register(data)
            .or_else(|| self.get_by_name(data.short().name()).map(|t| t.id))
            .unwrap()
    }

    pub fn any_type_id(&self, data: &'static FfiExtendedTypeInfo) -> AnyTypeId {
        AnyTypeId::new(self.tag(), self.get_or_register(data))
    }

    unsafe fn shared<'a>(this: *const c_void) -> &'a TypesRegistryShared {
        unsafe { &*(this as *const TypesRegistryShared) }
    }

    unsafe extern "C-unwind" fn ffi_clone(this: *const c_void) {
        unsafe { Arc::increment_strong_count(this as *const TypesRegistryShared) }
    }
    unsafe extern "C-unwind" fn ffi_len(this: *const c_void) -> u64 {
        unsafe { Self::shared(this).read().unwrap().types.len() as u64 }
    }
    unsafe extern "C-unwind" fn ffi_get(this: *const c_void, id: u64) -> FfiOption<&'static FfiExtendedTypeInfo> {
        unsafe { FfiOption::from_option(Self::shared(this).read().unwrap().get(id)) }
    }
    unsafe extern "C-unwind" fn ffi_get_by_name(this: *const c_void, name: FfiStrSliceRef) -> FfiOption<StoredTypeInfo> {
        unsafe { FfiOption::from_option(Self::shared(this).read().unwrap().get_by_name(name.into_slice())) }
    }
    unsafe extern "C-unwind" fn ffi_try_register(this: *const c_void, data: &'static FfiExtendedTypeInfo) -> FfiOption<u64> {
        unsafe { Self::shared(this).write().unwrap().try_register(data).into() }
    }
    unsafe extern "C-unwind" fn ffi_drop(this: *const c_void) {
        unsafe { drop(Arc::from_raw(this as *const TypesRegistryShared)) }
    }
}

impl Drop for TypesRegistryAccessFfi {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop_fn)(self.data) }
    }
}

impl Clone for TypesRegistryAccessFfi {
    fn clone(&self) -> Self {
        unsafe { (self.vtable.clone_fn)(self.data) };

        Self {
            data: self.data,
            vtable: self.vtable,
        }
    }
}

unsafe impl Send for TypesRegistryAccessFfi {}
unsafe impl Sync for TypesRegistryAccessFfi {}

//

struct TypesRegistryData {
    types_by_string: HashMap<String, u64>,
    types: Vec<StoredTypeInfo>,
}

impl TypesRegistryData {
    fn get(&self, id: u64) -> Option<&'static FfiExtendedTypeInfo> {
        self.types.get(id as usize).map(|t| t.data)
    }

    fn get_by_name(&self, name: &str) -> Option<StoredTypeInfo> {
        self.types.get(*self.types_by_string.get(name)? as usize).copied()
    }

    fn try_register(&mut self, data: &'static FfiExtendedTypeInfo) -> Option<u64> {
        let name = data.short().name();

        if self.types_by_string.contains_key(name) {
            return None;
        }

        let id = self.types.len() as u64;
        self.types_by_string.insert(name.to_string(), id);
        self.types.push(StoredTypeInfo { id, data });

        Some(id)
    }
}

//

pub struct RegistrySpecificTypeId<T: 'static> {
    id: AnyTypeId,
    _phantom: PhantomData<fn(T) -> T>,
}

impl<T: 'static> RegistrySpecificTypeId<T> {
    pub fn try_new(types: &TypesRegistryCache) -> Option<Self> {
        Some(Self {
            id: AnyTypeId::new(types.tag(), types.get::<T>()?),
            _phantom: PhantomData,
        })
    }

    pub fn new(types: &TypesRegistryCache) -> Self {
        Self {
            id: AnyTypeId::new(types.tag(), types.get_or_register::<T>()),
            _phantom: PhantomData,
        }
    }

    pub fn id(&self) -> u64 {
        self.id.id()
    }

    pub fn tag(&self) -> TypesRegistryTag {
        self.id.tag()
    }

    pub fn any_type_id(&self) -> AnyTypeId {
        self.id
    }
}

impl<T: 'static> Clone for RegistrySpecificTypeId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static> Copy for RegistrySpecificTypeId<T> {}

//

pub trait TypeCache: 'static + Copy {
    fn new(types: &TypesRegistryCache) -> Self;
}

impl<T: 'static> TypeCache for RegistrySpecificTypeId<T> {
    fn new(types: &TypesRegistryCache) -> Self {
        Self::new(types)
    }
}

macro_rules! type_cache_tuple {
    ($($c: ident),*) => {
        impl<
        $($c: TypeCache),*
        > TypeCache for (
        $($c,)*
        ) {
            fn new(_types: &TypesRegistryCache) -> Self {
                (
                    $($c::new(_types),)*
                )
            }
        }
    };
}

type_cache_tuple!();
type_cache_tuple!(C0);
type_cache_tuple!(C0, C1);
type_cache_tuple!(C0, C1, C2);
type_cache_tuple!(C0, C1, C2, C3);
type_cache_tuple!(C0, C1, C2, C3, C4);
type_cache_tuple!(C0, C1, C2, C3, C4, C5);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7, C8);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13);
type_cache_tuple!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13, C14);

//

#[derive(Clone)]
pub struct TypesRegistryCache {
    cache: Arc<RwLock<HashMap<TypeId, u64>>>,
    registry: TypesRegistryAccessFfi,
}

impl TypesRegistryCache {
    pub fn new(registry: TypesRegistryAccessFfi) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            registry,
        }
    }

    pub fn tag(&self) -> TypesRegistryTag {
        self.registry.tag()
    }

    pub fn get_with_type_id(&self, type_id: &TypeId) -> Option<u64> {
        self.cache.read().unwrap().get(type_id).copied()
    }

    pub fn get<T: 'static>(&self) -> Option<u64> {
        self.get_with_type_id(&TypeId::of::<T>())
    }

    pub fn try_register<T: 'static>(&self) -> Option<u64> {
        let mut cache = self.cache.write().unwrap();

        if cache.contains_key(&TypeId::of::<T>()) {
            return None;
        }

        let id = self.registry.try_register(&const { FfiExtendedTypeInfo::of::<T>() });

        if let Some(id) = id {
            cache.insert(TypeId::of::<T>(), id);

            return Some(id);
        }

        if let Some(id) = self.registry.get_by_name(std::any::type_name::<T>()).map(|t| t.id) {
            cache.insert(TypeId::of::<T>(), id);

            return Some(id);
        }

        None
    }

    pub fn get_or_register<T: 'static>(&self) -> u64 {
        self.get::<T>().unwrap_or_else(|| self.try_register::<T>().unwrap())
    }

    pub fn type_id_of<T: 'static>(&self) -> RegistrySpecificTypeId<T> {
        RegistrySpecificTypeId::new(self)
    }

    pub fn any_type_id(&self, data: &'static FfiExtendedTypeInfo) -> AnyTypeId {
        self.registry.any_type_id(data)
    }

    pub fn ffi(&self) -> &TypesRegistryAccessFfi {
        &self.registry
    }
}
