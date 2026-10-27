use std::{cell::UnsafeCell, collections::HashMap, sync::RwLock};

use fruits_ffi::FfiOpaqueVec;

use crate::*;

pub struct EventsHolderNative {
    events: RwLock<HashMap<u64, UnsafeCell<FfiOpaqueVec>>>,
    types: TypesRegistryAccessFfi,
}
impl EventsHolderNative {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
            types,
        }
    }

    pub fn get(&self, type_id: u64) -> *mut FfiOpaqueVec {
        let events = &*self.events.read().unwrap();

        match events.get(&type_id) {
            Some(vec) => vec.get(),
            None => std::ptr::null_mut(),
        }
    }

    pub fn get_or_create(&self, type_id: u64) -> *mut FfiOpaqueVec {
        let type_info = self.types.get(type_id).unwrap();

        let events = &mut *self.events.write().unwrap();

        events
            .entry(type_id)
            .or_insert_with(|| UnsafeCell::new(unsafe {
                FfiOpaqueVec::new(type_info.short().size(), type_info.short().align(), Some(type_info.raw_drop_fn()))
            }))
            .get()
    }

    pub fn clear(&self) {
        let events = &mut *self.events.write().unwrap();

        for values in events.values_mut() {
            values.get_mut().clear();
        }
    }
}

// todo?
// Safety. Is safe itself. Ptr usage is managed by caller
unsafe impl Send for EventsHolderNative {}
unsafe impl Sync for EventsHolderNative {}
