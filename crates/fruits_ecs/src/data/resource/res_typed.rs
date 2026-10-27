use fruits_ffi::{FfiAny, FfiAnyMut, FfiAnyPtr, FfiAnyRef, FfiExtendedTypeInfo, FfiFnMutMut};

use crate::*;

pub trait Resource: 'static + Send + Sync {}

//

fn resources_holder_get_ptr<T: Resource>(res: &ResourcesHolderUnsafeFfi, types: &TypesRegistryCache) -> Option<*mut T> {
    res.get(types.get_or_register::<T>()).map(|p| p.downcast_ptr().unwrap())
}
fn resources_holder_get_ptr_any(res: &ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyPtr> {
    unsafe { res.get(types.registry().get_or_register(type_info)) }
}
fn resources_holder_get_all(res: &ResourcesHolderUnsafeFfi, mut handler: impl FnMut(FfiAnyPtr)) {
    res.get_all(FfiFnMutMut::new(&mut handler))
}

fn resources_holder_get<'r, T: Resource>(res: &'r ResourcesHolderUnsafeFfi, types: &TypesRegistryCache) -> Option<&'r T> {
    unsafe { resources_holder_get_ptr(res, types).map(|p| &*p) }
}
fn resources_holder_get_any<'r>(res: &'r ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyRef<'r>> {
    unsafe { resources_holder_get_ptr_any(res, types, type_info).map(|p| p.as_any_ref()) }
}

fn resources_holder_get_mut<'r, T: Resource>(res: &'r mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache) -> Option<&'r mut T> {
    unsafe { resources_holder_get_ptr(res, types).map(|p| &mut *p) }
}
fn resources_holder_get_mut_any<'r>(res: &'r mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyMut<'r>> {
    unsafe { resources_holder_get_ptr_any(res, types, type_info).map(|p| p.as_any_mut()) }
}

fn resources_holder_insert<T: Resource>(res: &mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, data: T) -> Option<T> {
    res.insert(types.get_or_register::<T>(), FfiAny::new(data)).map(|a| a.downcast::<T>().unwrap().into_inner())
}
fn resources_holder_insert_any(res: &mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, data: FfiAny) -> Option<FfiAny> {
    unsafe { res.insert(types.registry().get_or_register(data.type_info()), FfiAny::new(data)) }
}

fn resources_holder_remove<T: Resource>(res: &mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache) -> Option<T> {
    res.remove(types.get_or_register::<T>()).map(|a| a.downcast::<T>().unwrap().into_inner())
}
fn resources_holder_remove_any(res: &mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAny> {
    unsafe { res.remove(types.registry().get_or_register(type_info)) }
}

//

pub struct ResourcesHolderMut<'r> {
    res: *mut ResourcesHolderUnsafeFfi,
    types: &'r TypesRegistryCache,
}

impl<'r> ResourcesHolderMut<'r> {
    pub unsafe fn new(res: *mut ResourcesHolderUnsafeFfi, types: &'r TypesRegistryCache) -> Self {
        Self { res, types }
    }

    pub fn insert<T: Resource>(&mut self, data: T) -> Option<T> {
        unsafe { resources_holder_insert::<T>(&mut *self.res, self.types, data) }
    }
    pub fn insert_any(&mut self, data: FfiAny) -> Option<FfiAny> {
        unsafe { resources_holder_insert_any(&mut *self.res, self.types, data) }
    }

    pub fn remove<T: Resource>(&mut self) -> Option<T> {
        unsafe { resources_holder_remove::<T>(&mut *self.res, self.types) }
    }
    pub fn remove_any(&mut self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAny> {
        unsafe { resources_holder_remove_any(&mut *self.res, self.types, type_info) }
    }

    pub fn get_ptr<T: Resource>(&self) -> Option<*mut T> {
        unsafe { resources_holder_get_ptr::<T>(&*self.res, self.types) }
    }
    pub fn get_ptr_any(&self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyPtr> {
        unsafe { resources_holder_get_ptr_any(&*self.res, self.types, type_info) }
    }
    pub fn get_all(&self, handler: impl FnMut(FfiAnyPtr)) {
        unsafe { resources_holder_get_all(&*self.res, handler) }
    }

    pub fn into_get<T: Resource>(self) -> Option<&'r T> {
        unsafe { resources_holder_get::<T>(&*self.res, self.types) }
    }
    pub fn into_get_any(self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyRef<'r>> {
        unsafe { resources_holder_get_any(&*self.res, self.types, type_info) }
    }

    pub fn into_get_mut<T: Resource>(self) -> Option<&'r mut T> {
        unsafe { resources_holder_get_mut::<T>(&mut *self.res, self.types) }
    }
    pub fn into_get_mut_any(self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyMut<'r>> {
        unsafe { resources_holder_get_mut_any(&mut *self.res, self.types, type_info) }
    }
}

impl<'r: 'p, 'p> ResourcesHolderMut<'r> {
    pub fn get<T: Resource>(&'p self) -> Option<&'p T> {
        self.as_ref().get::<T>()
    }
    pub fn get_any(&'p self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyRef<'p>> {
        self.as_ref().get_any(type_info)
    }

    pub fn get_mut<T: Resource>(&'p mut self) -> Option<&'p mut T> {
        self.as_mut().into_get_mut::<T>()
    }
    pub fn get_mut_any(&'p mut self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyMut<'p>> {
        self.as_mut().into_get_mut_any(type_info)
    }

    pub fn as_mut(&'p mut self) -> ResourcesHolderMut<'p> {
        ResourcesHolderMut {
            res: self.res,
            types: self.types,
        }
    }

    pub fn as_ref(&'p self) -> ResourcesHolderRef<'p> {
        ResourcesHolderRef {
            res: self.res,
            types: self.types,
        }
    }
}

unsafe impl<'r> Send for ResourcesHolderMut<'r> { }
unsafe impl<'r> Sync for ResourcesHolderMut<'r> { }

//

#[derive(Copy, Clone)]
pub struct ResourcesHolderRef<'r> {
    res: *const ResourcesHolderUnsafeFfi,
    types: &'r TypesRegistryCache,
}

impl<'r> ResourcesHolderRef<'r> {
    pub unsafe fn new(res: *const ResourcesHolderUnsafeFfi, types: &'r TypesRegistryCache) -> Self {
        Self { res, types }
    }

    pub fn get_ptr<T: Resource>(self) -> Option<*mut T> {
        unsafe { resources_holder_get_ptr::<T>(&*self.res, self.types) }
    }
    pub fn get_ptr_any(&self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyPtr> {
        unsafe { resources_holder_get_ptr_any(&*self.res, self.types, type_info) }
    }
    pub fn get_all(&self, handler: impl FnMut(FfiAnyPtr)) {
        unsafe { resources_holder_get_all(&*self.res, handler) }
    }

    pub fn get<T: Resource>(self) -> Option<&'r T> {
        unsafe { resources_holder_get::<T>(&*self.res, self.types) }
    }
    pub fn get_any(self, type_info: &'static FfiExtendedTypeInfo) -> Option<FfiAnyRef<'r>> {
        unsafe { resources_holder_get_any(&*self.res, self.types, type_info) }
    }
}

unsafe impl<'r> Send for ResourcesHolderRef<'r> { }
unsafe impl<'r> Sync for ResourcesHolderRef<'r> { }