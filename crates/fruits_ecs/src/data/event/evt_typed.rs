use fruits_ffi::{FfiOpaqueVec, FfiVec};

use crate::*;

pub trait Event: 'static {}

//

fn events_holder_get<'e, E: Event>(evt: &'e EventsHolderUnsafeFfi, types: &TypesRegistryCache) -> &'e [E] {
    unsafe {
        let result = evt.get(types.get_or_register::<E>());

        if result.is_null() {
            &[]
        } else {
            &*FfiOpaqueVec::as_vec_ptr(result)
        }
    }
}

fn events_holder_get_ptr<'e, E: Event>(evt: &'e EventsHolderUnsafeFfi, types: &TypesRegistryCache) -> *mut FfiVec<E> {
    unsafe {
        let result = evt.get_or_create(types.get_or_register::<E>());

        FfiOpaqueVec::as_vec_ptr(result)
    }
}

fn events_holder_get_mut<'e, E: Event>(evt: &'e mut EventsHolderUnsafeFfi, types: &TypesRegistryCache) -> &'e mut FfiVec<E> {
    unsafe { &mut *events_holder_get_ptr(evt, types) }
}

fn events_holder_clear<'e>(evt: &'e mut EventsHolderUnsafeFfi) {
    evt.clear();
}

//

// todo

pub struct EventsHolderMut<'e> {
    evt: *mut EventsHolderUnsafeFfi,
    types: &'e TypesRegistryCache,
}

impl<'e> EventsHolderMut<'e> {
    pub unsafe fn new(evt: *mut EventsHolderUnsafeFfi, types: &'e TypesRegistryCache) -> Self {
        Self { evt, types }
    }

    pub fn get<'r, E: Event>(&'r self) -> &'r [E]
    where
        'e: 'r,
    {
        unsafe { events_holder_get(&*self.evt, self.types) }
    }

    pub fn get_ptr<'r, E: Event>(&'r self) -> *mut FfiVec<E>
    where
        'e: 'r,
    {
        unsafe { events_holder_get_ptr(&*self.evt, self.types) }
    }

    pub fn get_mut<'r, E: Event>(&'r mut self) -> &'r mut FfiVec<E>
    where
        'e: 'r,
    {
        unsafe { events_holder_get_mut(&mut *self.evt, self.types) }
    }

    pub fn clear(&mut self) {
        unsafe { events_holder_clear(&mut *self.evt) }
    }
}

//

#[derive(Copy, Clone)]
pub struct EventsHolderRef<'e> {
    evt: *const EventsHolderUnsafeFfi,
    types: &'e TypesRegistryCache,
}

impl<'e> EventsHolderRef<'e> {
    pub unsafe fn new(evt: *const EventsHolderUnsafeFfi, types: &'e TypesRegistryCache) -> Self {
        Self { evt, types }
    }

    pub fn get<E: Event>(self) -> &'e [E] {
        unsafe { events_holder_get(&*self.evt, self.types) }
    }

    pub fn get_ptr<E: Event>(self) -> *mut FfiVec<E> {
        unsafe { events_holder_get_ptr(&*self.evt, self.types) }
    }
}
