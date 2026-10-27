use std::{
    any::TypeId, collections::HashMap, ffi::c_void, marker::PhantomData, sync::{Arc, RwLock}
};

use fruits_ffi::{FfiExtendedTypeInfo, FfiOption, FfiStrSliceRef};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct StoredTypeInfo {
    pub id: u64,
    pub data: &'static FfiExtendedTypeInfo,
}

//

#[repr(C)]
struct TypesRegistryAccessVtable {
    pub clone_fn: unsafe extern "C-unwind" fn(*const c_void) -> *mut c_void,
    pub len_fn: unsafe extern "C-unwind" fn(*const c_void) -> u64,
    pub get_fn: unsafe extern "C-unwind" fn(*const c_void, id: u64) -> FfiOption<&'static FfiExtendedTypeInfo>,
    pub get_by_name_fn: unsafe extern "C-unwind" fn(*const c_void, name: FfiStrSliceRef) -> FfiOption<StoredTypeInfo>,
    pub try_register_fn: unsafe extern "C-unwind" fn(*const c_void, data: &'static FfiExtendedTypeInfo) -> FfiOption<u64>,
    pub drop_fn: unsafe extern "C-unwind" fn(*mut c_void),
}

#[repr(C)]
pub struct TypesRegistryAccessFfi {
    data: *mut c_void,
    vtable: &'static TypesRegistryAccessVtable,
}

//

impl TypesRegistryAccessFfi {
    pub fn new() -> Self {
        let types = TypesRegistryAccessNative::new();

        Self {
            data: Box::into_raw(Box::new(types)) as *mut c_void,
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

    pub fn len(&self) -> u64 {
        unsafe { (self.vtable.len_fn)((&raw const *self) as *const c_void) }
    }

    pub fn get(&self, id: u64) -> Option<&'static FfiExtendedTypeInfo> {
        unsafe {
            let this = self.data;

            let data = (self.vtable.get_fn)(this, id);

            data.into_option()
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<StoredTypeInfo> {
        unsafe {
            let this = self.data;
            let name = FfiStrSliceRef::from_slice(name);

            let data = (self.vtable.get_by_name_fn)(this, name);

            data.into_option()
        }
    }

    pub fn try_register(&self, data: &'static FfiExtendedTypeInfo) -> Option<u64> {
        unsafe {
            let this = self.data;

            let data = (self.vtable.try_register_fn)(this, data);

            data.into_option()
        }
    }

    pub fn get_or_register(&self, data: &'static FfiExtendedTypeInfo) -> u64 {
        if let Some(id) = self.get_by_name(data.short().name()) {
            return id.id;
        }

        self.try_register(data).unwrap()
    }

    unsafe extern "C-unwind" fn ffi_clone(this: *const c_void) -> *mut c_void {
        unsafe {
            let this = &*(this as *const TypesRegistryAccessNative);

            let cloned = this.clone();

            Box::into_raw(Box::new(cloned)) as *mut c_void
        }
    }
    unsafe extern "C-unwind" fn ffi_len(this: *const c_void) -> u64 {
        unsafe {
            let this = &*(this as *const TypesRegistryAccessNative);

            let result = this.len();

            result as u64
        }
    }
    unsafe extern "C-unwind" fn ffi_get(this: *const c_void, id: u64) -> FfiOption<&'static FfiExtendedTypeInfo> {
        unsafe {
            let this = &*(this as *const TypesRegistryAccessNative);

            let result = this.get(id);

            FfiOption::from_option(result)
        }
    }
    unsafe extern "C-unwind" fn ffi_get_by_name(this: *const c_void, name: FfiStrSliceRef) -> FfiOption<StoredTypeInfo> {
        unsafe {
            let this = &*(this as *const TypesRegistryAccessNative);
            let name = name.into_slice();

            let result = this.get_by_name(name);

            FfiOption::from_option(result)
        }
    }
    unsafe extern "C-unwind" fn ffi_try_register(this: *const c_void, data: &'static FfiExtendedTypeInfo) -> FfiOption<u64> {
        unsafe {
            let this = &*(this as *const TypesRegistryAccessNative);

            let result = this.try_register(data);

            result.into()
        }
    }
    pub unsafe extern "C-unwind" fn ffi_drop(this_ref: *mut c_void) {
        // todo
        unsafe {
            drop(Box::from_raw(this_ref as *mut TypesRegistryAccessNative));
        }
    }
    // todo
}

impl Drop for TypesRegistryAccessFfi {
    fn drop(&mut self) {
        // todo
        unsafe {
            (self.vtable.drop_fn)(self.data);
        }
    }
}

impl Clone for TypesRegistryAccessFfi {
    fn clone(&self) -> Self {
        unsafe {
            let data = (self.vtable.clone_fn)(self.data);

            Self {
                data,
                vtable: self.vtable,
            }
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

struct TypesRegistryAccessNative {
    data: Arc<RwLock<TypesRegistryData>>,
}

impl Clone for TypesRegistryAccessNative {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

impl TypesRegistryAccessNative {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(TypesRegistryData {
                types_by_string: HashMap::new(),
                types: Vec::new(),
            })),
        }
    }

    pub fn len(&self) -> usize {
        let self_data = self.data.read().unwrap();

        let result = self_data.types.len();

        result
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let self_data = self.data.read().unwrap();

        let result = self_data.types.is_empty();

        result
    }

    pub fn get(&self, id: u64) -> Option<&'static FfiExtendedTypeInfo> {
        let self_data = self.data.read().unwrap();

        let result = self_data.types.get(id as usize).copied();

        result.map(|t| t.data)
    }

    pub fn get_by_name(&self, name: &str) -> Option<StoredTypeInfo> {
        let self_data = self.data.read().unwrap();

        let result = self_data.types.get(*self_data.types_by_string.get(name)? as usize).copied();

        result
    }

    pub fn try_register(&self, data: &'static FfiExtendedTypeInfo) -> Option<u64> {
        let mut self_data = self.data.write().unwrap();
        let name = data.short().name();

        if self_data.types_by_string.get(name).is_some() {
            return None;
        }

        let id = self_data.types.len() as u64;
        self_data.types_by_string.insert(name.to_string(), id);
        self_data.types.push(StoredTypeInfo { id, data });

        Some(id)
    }
}

//

pub struct RegistrySpecificTypeId<T: 'static> {
    id: u64,
    _phantom: PhantomData<fn(T) -> T>,
}

impl<T: 'static> RegistrySpecificTypeId<T> {
    pub fn try_new(types: &TypesRegistryCache) -> Option<Self> {
        Some(Self {
            id: types.get::<T>()?,
            _phantom: PhantomData,
        })
    }

    pub fn new(types: &TypesRegistryCache) -> Self {
        Self {
            id: types.get_or_register::<T>(),
            _phantom: PhantomData,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

impl<T: 'static> Clone for RegistrySpecificTypeId<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _phantom: PhantomData,
        }
    }
}
impl<T: 'static> Copy for RegistrySpecificTypeId<T> {}

//

pub trait TypeCache : 'static + Copy {
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

// impl Clone for TypesRegistryCache {
//     fn clone(&self) -> Self {
//         Self {
//             cache: Arc::clone(&self.cache),
//             registry: self.registry.clone(),
//         }
//     }
// }

impl TypesRegistryCache {
    pub fn new(registry: TypesRegistryAccessFfi) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            registry,
        }
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

    pub unsafe fn registry(&self) -> &TypesRegistryAccessFfi {
        &self.registry
    }

    // todo
    // pub unsafe fn raw_registry(&self) -> TypesRegistryAccessRefFfi {
    //     self.registry.clone()
    // }
}
