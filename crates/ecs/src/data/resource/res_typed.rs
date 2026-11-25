use aphyn_any::{AnyBox, AnyMut, AnyPtr, AnyRef};
use aphyn_ffi::{FfiExtendedTypeInfo, FfiFnMutMut};

use crate::*;

pub trait Resource: 'static + Send + Sync {}

//

fn resources_holder_get_ptr<T: Resource>(res: &ResourcesHolderUnsafeFfi, types: &TypesRegistryCache) -> Option<*mut T> {
    let id = types.type_id_of::<T>();
    res.get(id.id()).map(|p| p.downcast_ptr(id).unwrap())
}
fn resources_holder_get_ptr_any(
    res: &ResourcesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    type_info: &'static FfiExtendedTypeInfo,
) -> Option<AnyPtr> {
    res.get(types.ffi().get_or_register(type_info))
}
fn resources_holder_get_all(res: &ResourcesHolderUnsafeFfi, mut handler: impl FnMut(AnyPtr)) {
    res.get_all(FfiFnMutMut::new(&mut handler))
}

fn resources_holder_get<'r, T: Resource>(res: &'r ResourcesHolderUnsafeFfi, types: &TypesRegistryCache) -> Option<&'r T> {
    unsafe { resources_holder_get_ptr(res, types).map(|p| &*p) }
}
fn resources_holder_get_any<'r>(
    res: &'r ResourcesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    type_info: &'static FfiExtendedTypeInfo,
) -> Option<AnyRef<'r>> {
    unsafe { resources_holder_get_ptr_any(res, types, type_info).map(|p| p.as_any_ref()) }
}

fn resources_holder_get_mut<'r, T: Resource>(
    res: &'r mut ResourcesHolderUnsafeFfi,
    types: &TypesRegistryCache,
) -> Option<&'r mut T> {
    unsafe { resources_holder_get_ptr(res, types).map(|p| &mut *p) }
}
fn resources_holder_get_mut_any<'r>(
    res: &'r mut ResourcesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    type_info: &'static FfiExtendedTypeInfo,
) -> Option<AnyMut<'r>> {
    unsafe { resources_holder_get_ptr_any(res, types, type_info).map(|p| p.as_any_mut()) }
}

fn resources_holder_insert<T: Resource>(res: &mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, data: T) -> Option<T> {
    let id = types.type_id_of::<T>();
    res.insert(id.id(), AnyBox::new(data, types))
        .map(|a| a.downcast(id).ok().unwrap())
}
fn resources_holder_insert_any(res: &mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache, data: AnyBox) -> Option<AnyBox> {
    res.insert(types.ffi().get_or_register(data.type_info()), data)
}

fn resources_holder_remove<T: Resource>(res: &mut ResourcesHolderUnsafeFfi, types: &TypesRegistryCache) -> Option<T> {
    let id = types.type_id_of::<T>();
    res.remove(id.id()).map(|a| a.downcast(id).ok().unwrap())
}
fn resources_holder_remove_any(
    res: &mut ResourcesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    type_info: &'static FfiExtendedTypeInfo,
) -> Option<AnyBox> {
    res.remove(types.ffi().get_or_register(type_info))
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

    pub fn types(&self) -> &'r TypesRegistryCache {
        self.types
    }

    pub fn insert<T: Resource>(&mut self, data: T) -> Option<T> {
        unsafe { resources_holder_insert::<T>(&mut *self.res, self.types, data) }
    }
    pub fn insert_any(&mut self, data: AnyBox) -> Option<AnyBox> {
        unsafe { resources_holder_insert_any(&mut *self.res, self.types, data) }
    }

    pub fn remove<T: Resource>(&mut self) -> Option<T> {
        unsafe { resources_holder_remove::<T>(&mut *self.res, self.types) }
    }
    pub fn remove_any(&mut self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyBox> {
        unsafe { resources_holder_remove_any(&mut *self.res, self.types, type_info) }
    }

    pub fn get_ptr<T: Resource>(&self) -> Option<*mut T> {
        unsafe { resources_holder_get_ptr::<T>(&*self.res, self.types) }
    }
    pub fn get_ptr_any(&self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyPtr> {
        unsafe { resources_holder_get_ptr_any(&*self.res, self.types, type_info) }
    }
    pub fn get_all(&self, handler: impl FnMut(AnyPtr)) {
        unsafe { resources_holder_get_all(&*self.res, handler) }
    }

    pub fn into_get<T: Resource>(self) -> Option<&'r T> {
        unsafe { resources_holder_get::<T>(&*self.res, self.types) }
    }
    pub fn into_get_any(self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyRef<'r>> {
        unsafe { resources_holder_get_any(&*self.res, self.types, type_info) }
    }

    pub fn into_get_mut<T: Resource>(self) -> Option<&'r mut T> {
        unsafe { resources_holder_get_mut::<T>(&mut *self.res, self.types) }
    }
    pub fn into_get_mut_any(self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyMut<'r>> {
        unsafe { resources_holder_get_mut_any(&mut *self.res, self.types, type_info) }
    }
}

impl<'r: 'p, 'p> ResourcesHolderMut<'r> {
    pub fn get<T: Resource>(&'p self) -> Option<&'p T> {
        self.as_ref().get::<T>()
    }
    pub fn get_any(&'p self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyRef<'p>> {
        self.as_ref().get_any(type_info)
    }

    pub fn get_mut<T: Resource>(&'p mut self) -> Option<&'p mut T> {
        self.as_mut().into_get_mut::<T>()
    }
    pub fn get_mut_any(&'p mut self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyMut<'p>> {
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

unsafe impl<'r> Send for ResourcesHolderMut<'r> {}
unsafe impl<'r> Sync for ResourcesHolderMut<'r> {}

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

    pub fn types(self) -> &'r TypesRegistryCache {
        self.types
    }

    pub fn get_ptr<T: Resource>(self) -> Option<*mut T> {
        unsafe { resources_holder_get_ptr::<T>(&*self.res, self.types) }
    }
    pub fn get_ptr_any(&self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyPtr> {
        unsafe { resources_holder_get_ptr_any(&*self.res, self.types, type_info) }
    }
    pub fn get_all(&self, handler: impl FnMut(AnyPtr)) {
        unsafe { resources_holder_get_all(&*self.res, handler) }
    }

    pub fn get<T: Resource>(self) -> Option<&'r T> {
        unsafe { resources_holder_get::<T>(&*self.res, self.types) }
    }
    pub fn get_any(self, type_info: &'static FfiExtendedTypeInfo) -> Option<AnyRef<'r>> {
        unsafe { resources_holder_get_any(&*self.res, self.types, type_info) }
    }
}

unsafe impl<'r> Send for ResourcesHolderRef<'r> {}
unsafe impl<'r> Sync for ResourcesHolderRef<'r> {}
