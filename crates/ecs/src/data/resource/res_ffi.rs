use aphyn_any::{AnyBox, AnyPtr};
use aphyn_ffi::{FfiFnMutMut, FfiIndexMap};

use crate::*;

#[repr(C)]
pub struct ResourcesHolderUnsafeFfi {
    types: TypesRegistryAccessFfi,
    resources: FfiIndexMap<u64, AnyBox>,
}
impl ResourcesHolderUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            types,
            resources: FfiIndexMap::new(),
        }
    }

    pub fn insert(&mut self, id: u64, item: AnyBox) -> Option<AnyBox> {
        self.resources.insert(id, item)
    }

    pub fn remove(&mut self, id: u64) -> Option<AnyBox> {
        self.resources.remove_swap(&id)
    }

    pub fn get(&self, id: u64) -> Option<AnyPtr> {
        self.resources.get(&id).map(|a| a.as_any_ptr())
    }

    pub fn get_all(&self, mut handler: FfiFnMutMut<AnyPtr, ()>) {
        for resource in &self.resources {
            handler.execute(resource.1.as_any_ptr())
        }
    }
}

// todo?
// Safety. It is safe itself. Ptr usage is managed by caller
unsafe impl Send for ResourcesHolderUnsafeFfi {}
unsafe impl Sync for ResourcesHolderUnsafeFfi {}
