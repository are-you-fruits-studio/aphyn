use std::{collections::HashMap, ffi::c_void, sync::Mutex};

use crate::*;

#[repr(C)]
pub struct SystemResourceGetOrInsertResult {
    pub ptr: *mut u8,
    pub is_new: bool,
}

pub struct SystemResourcesHolderNative {
    types: TypesRegistryAccessFfi,
    resources: Mutex<HashMap<u64, *mut u8>>,
}
impl SystemResourcesHolderNative {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            types,
            resources: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_or_insert(&self, id: u64) -> SystemResourceGetOrInsertResult {
        let mut resources = self.resources.lock().unwrap();

        if let Some(res) = resources.get(&id) {
            return SystemResourceGetOrInsertResult {
                ptr: *res,
                is_new: false,
            };
        }

        let Some(type_info) = self.types.get(id) else {
            panic!("Type data missing");
        };

        // todo
        let mem = unsafe {
            let Some(layout) = std::alloc::Layout::from_size_align(type_info.short().size() as usize, type_info.short().align() as usize).ok() else {
                panic!("Invalid type layout");
            };

            std::alloc::alloc(layout)
        };

        resources.insert(id, mem);

        SystemResourceGetOrInsertResult { ptr: mem, is_new: true }
    }
}

impl Drop for SystemResourcesHolderNative {
    fn drop(&mut self) {
        let resources = self.resources.lock().unwrap();

        for (&id, &mem) in resources.iter() {
            let type_info = self.types.get(id).unwrap();

            // todo
            unsafe {
                type_info.drop(mem as *mut c_void);

                let layout = std::alloc::Layout::from_size_align(type_info.short().size() as usize, type_info.short().align() as usize)
                    .ok()
                    .unwrap();

                std::alloc::dealloc(mem, layout)
            }
        }
    }
}

// todo?
// Safety. It is safe itself. Ptr usage is managed by caller
unsafe impl Send for SystemResourcesHolderNative {}
unsafe impl Sync for SystemResourcesHolderNative {}
