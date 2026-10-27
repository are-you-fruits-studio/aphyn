use fruits_ffi::{FfiAny, FfiAnyPtr, FfiFnMutMut, FfiIndexMap};

use crate::*;

#[repr(C)]
pub struct ResourcesHolderUnsafeFfi {
    types: TypesRegistryAccessFfi,
    resources: FfiIndexMap<u64, FfiAny>,
}
impl ResourcesHolderUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            types,
            resources: FfiIndexMap::new(),
        }
    }

    pub fn insert(&mut self, id: u64, item: FfiAny) -> Option<FfiAny> {
        self.resources.insert(id, item)
    }

    pub fn remove(&mut self, id: u64) -> Option<FfiAny> {
        self.resources.remove_swap(&id)
    }

    pub fn get(&self, id: u64) -> Option<FfiAnyPtr> {
        self.resources.get(&id).map(|a| a.as_any_ptr())
    }

    pub fn get_all(&self, mut handler: FfiFnMutMut<FfiAnyPtr, ()>) {
        for resource in &self.resources {
            handler.execute(resource.1.as_any_ptr())
        }
    }
}

// todo?
// Safety. It is safe itself. Ptr usage is managed by caller
unsafe impl Send for ResourcesHolderUnsafeFfi {}
unsafe impl Sync for ResourcesHolderUnsafeFfi {}
