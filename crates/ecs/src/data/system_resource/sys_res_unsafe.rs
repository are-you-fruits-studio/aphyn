use crate::*;

fn system_resources_holder_typed_get_or_insert<'a, T: 'static + Default>(
    res: &'a SystemResourcesHolderUnsafeFfi,
    types: &TypesRegistryCache,
) -> *mut T {
    let type_id = types.get_or_register::<T>();

    unsafe {
        let result = res.get_or_insert(type_id);

        let ptr = result.ptr as *mut T;

        if result.is_new {
            ptr.write(T::default());
        }

        ptr
    }
}

// todo: refactor to use real references instead of lifetimed structs.
// todo: check types registry compatibility.

pub struct SystemResourcesHolder {
    res: SystemResourcesHolderUnsafeFfi,
    types: TypesRegistryCache,
}

impl SystemResourcesHolder {
    pub fn new(types: TypesRegistryCache) -> Self {
        Self {
            res: SystemResourcesHolderUnsafeFfi::new(unsafe { types.registry().clone() }),
            types,
        }
    }

    pub fn get_or_insert<T: 'static + Default>(&self) -> *mut T {
        system_resources_holder_typed_get_or_insert(&self.res, &self.types)
    }

    pub unsafe fn ffi(&mut self) -> *mut SystemResourcesHolderUnsafeFfi {
        &raw mut self.res
    }

    pub fn as_mut<'r>(&'r mut self) -> SystemResourcesHolderMut<'r> {
        SystemResourcesHolderMut {
            res: &mut self.res,
            types: &self.types,
        }
    }

    pub fn as_ref<'r>(&'r self) -> SystemResourcesHolderRef<'r> {
        SystemResourcesHolderRef {
            res: &self.res,
            types: &self.types,
        }
    }
}

//

pub struct SystemResourcesHolderMut<'r> {
    res: &'r mut SystemResourcesHolderUnsafeFfi,
    types: &'r TypesRegistryCache,
}

impl<'r> SystemResourcesHolderMut<'r> {
    pub fn new(res: &'r mut SystemResourcesHolderUnsafeFfi, types: &'r TypesRegistryCache) -> Self {
        Self { res, types }
    }

    pub fn get_or_insert<T: 'static + Default>(&self) -> *mut T {
        system_resources_holder_typed_get_or_insert(self.res, &self.types)
    }

    pub fn as_mut(&'r mut self) -> SystemResourcesHolderMut<'r> {
        SystemResourcesHolderMut {
            res: &mut self.res,
            types: self.types,
        }
    }

    pub fn as_ref(&'r self) -> SystemResourcesHolderRef<'r> {
        SystemResourcesHolderRef {
            res: self.res,
            types: self.types,
        }
    }
}

//

#[derive(Copy, Clone)]
pub struct SystemResourcesHolderRef<'r> {
    res: &'r SystemResourcesHolderUnsafeFfi,
    types: &'r TypesRegistryCache,
}

impl<'r> SystemResourcesHolderRef<'r> {
    pub fn new(res: &'r SystemResourcesHolderUnsafeFfi, types: &'r TypesRegistryCache) -> Self {
        Self { res, types }
    }

    pub fn get_or_insert<T: 'static + Default>(self) -> *mut T {
        system_resources_holder_typed_get_or_insert(self.res, &self.types)
    }
}
