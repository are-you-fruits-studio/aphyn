use std::{collections::HashMap, ffi::c_void, hash::Hash};

use crate::{FfiDroppable, FfiOption, FfiStaticRef, FfiStrSliceRef, FfiString};

#[repr(C)]
struct FfiHashMapVTable<K: 'static + Eq + Hash, V: 'static> {
    insert_fn: unsafe extern "C-unwind" fn(*mut c_void, k: K, v: V) -> FfiOption<V>,
    remove_fn: unsafe extern "C-unwind" fn(*mut c_void, k: *const K) -> FfiOption<V>,
    get_fn: unsafe extern "C-unwind" fn(*const c_void, k: *const K) -> FfiOption<*const V>,
    get_mut_fn: unsafe extern "C-unwind" fn(*mut c_void, k: *const K) -> FfiOption<*mut V>,
    get_by_str_fn: unsafe extern "C-unwind" fn(*const c_void, k: FfiStrSliceRef) -> FfiOption<*const V>,
    get_by_str_mut_fn: unsafe extern "C-unwind" fn(*mut c_void, k: FfiStrSliceRef) -> FfiOption<*mut V>,
    remove_by_str_fn: unsafe extern "C-unwind" fn(*mut c_void, k: FfiStrSliceRef) -> FfiOption<V>,
}

#[repr(C)]
pub struct FfiHashMap<K: 'static + Eq + Hash, V: 'static> {
    data: FfiDroppable,
    vtable: FfiStaticRef<FfiHashMapVTable<K, V>>,
}

unsafe impl<K: Eq + Hash + Send, V: Send> Send for FfiHashMap<K, V> {}
unsafe impl<K: Eq + Hash + Sync, V: Sync> Sync for FfiHashMap<K, V> {}

unsafe extern "C-unwind" fn ffi_insert<K: 'static + Eq + Hash, V: 'static>(this: *mut c_void, k: K, v: V) -> FfiOption<V> {
    unsafe {
        let this = &mut *(this as *mut HashMap<K, V>);

        let result = this.insert(k, v);

        FfiOption::from_option(result)
    }
}
unsafe extern "C-unwind" fn ffi_remove<K: 'static + Eq + Hash, V: 'static>(this: *mut c_void, k: *const K) -> FfiOption<V> {
    unsafe {
        let this = &mut *(this as *mut HashMap<K, V>);

        let result = this.remove(&*k);

        FfiOption::from_option(result)
    }
}
unsafe extern "C-unwind" fn ffi_get<K: 'static + Eq + Hash, V: 'static>(this: *const c_void, k: *const K) -> FfiOption<*const V> {
    unsafe {
        let this = &*(this as *const HashMap<K, V>);

        let result = this.get(&*k);

        FfiOption::from_option(result.map(|v| &raw const *v))
    }
}
unsafe extern "C-unwind" fn ffi_get_mut<K: 'static + Eq + Hash, V: 'static>(this: *mut c_void, k: *const K) -> FfiOption<*mut V> {
    unsafe {
        let this = &mut *(this as *mut HashMap<K, V>);

        let result = this.get_mut(&*k);

        FfiOption::from_option(result.map(|v| &raw mut *v))
    }
}
unsafe extern "C-unwind" fn ffi_get_by_str<V: 'static>(this: *const c_void, k: FfiStrSliceRef) -> FfiOption<*const V> {
    unsafe {
        let this = &*(this as *const HashMap<FfiString, V>);

        let result = this.get(k.into_slice());

        FfiOption::from_option(result.map(|v| &raw const *v))
    }
}
unsafe extern "C-unwind" fn ffi_get_by_str_mut<V: 'static>(this: *mut c_void, k: FfiStrSliceRef) -> FfiOption<*mut V> {
    unsafe {
        let this = &mut *(this as *mut HashMap<FfiString, V>);

        let result = this.get_mut(k.into_slice());

        FfiOption::from_option(result.map(|v| &raw mut *v))
    }
}
unsafe extern "C-unwind" fn ffi_remove_by_str<V: 'static>(this: *mut c_void, k: FfiStrSliceRef) -> FfiOption<V> {
    unsafe {
        let this = &mut *(this as *mut HashMap<FfiString, V>);

        let result = this.remove(k.into_slice());

        FfiOption::from_option(result)
    }
}

impl<K: 'static + Eq + Hash, V: 'static> FfiHashMap<K, V> {
    pub fn new() -> Self {
        Self {
            data: FfiDroppable::new(HashMap::<K, V>::new()),
            vtable: FfiStaticRef::new(&FfiHashMapVTable::<K, V> {
                insert_fn: ffi_insert::<K, V>,
                remove_fn: ffi_remove::<K, V>,
                get_fn: ffi_get::<K, V>,
                get_mut_fn: ffi_get_mut::<K, V>,
                get_by_str_fn: ffi_get_by_str::<V>,
                get_by_str_mut_fn: ffi_get_by_str_mut::<V>,
                remove_by_str_fn: ffi_remove_by_str::<V>,
            }),
        }
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        unsafe { (self.vtable.insert_fn)(self.data.get(), k, v).into_option() }
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        unsafe { (self.vtable.remove_fn)(self.data.get(), k).into_option() }
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        unsafe { (self.vtable.get_fn)(self.data.get(), k).into_option().map(|p| &*p) }
    }
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        unsafe { (self.vtable.get_mut_fn)(self.data.get(), k).into_option().map(|p| &mut *p) }
    }
}

impl<V: 'static> FfiHashMap<FfiString, V> {
    pub fn get_by_str(&self, k: &str) -> Option<&V> {
        unsafe {
            (self.vtable.get_by_str_fn)(self.data.get(), FfiStrSliceRef::from_slice(k))
                .into_option()
                .map(|p| &*p)
        }
    }
    pub fn get_by_str_mut(&mut self, k: &str) -> Option<&mut V> {
        unsafe {
            (self.vtable.get_by_str_mut_fn)(self.data.get(), FfiStrSliceRef::from_slice(k))
                .into_option()
                .map(|p| &mut *p)
        }
    }
    pub fn remove_by_str(&mut self, k: &str) -> Option<V> {
        unsafe { (self.vtable.remove_by_str_fn)(self.data.get(), FfiStrSliceRef::from_slice(k)).into_option() }
    }
}

// todo: other methods and impls
