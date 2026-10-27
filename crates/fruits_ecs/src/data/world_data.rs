use fruits_ffi::FfiBox;

use crate::*;

// todo: access modifiers
// todo: unsafe modifiers

#[repr(C)]
pub struct WorldDataUnsafeFfi {
    pub res: ResourcesHolderUnsafeFfi,
    pub evt: EventsHolderUnsafeFfi,
    pub ent: EntitiesHolderUnsafeFfi,
}

impl WorldDataUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            res: ResourcesHolderUnsafeFfi::new(types.clone()),
            evt: EventsHolderUnsafeFfi::new(types.clone()),
            ent: EntitiesHolderUnsafeFfi::new(types),
        }
    }
}

//

pub struct WorldData {
    world: FfiBox<WorldDataUnsafeFfi>,
    types: TypesRegistryCache,
}

impl WorldData {
    pub fn new(types: TypesRegistryCache) -> Self {
        unsafe {
            Self {
                world: FfiBox::new(WorldDataUnsafeFfi::new(types.registry().clone())),
                types,
            }
        }
    }

    pub fn resources<'r>(&'r self) -> ResourcesHolderRef<'r> {
        unsafe { ResourcesHolderRef::new(&raw const self.world.res, &self.types) }
    }
    pub fn resources_mut<'r>(&'r mut self) -> ResourcesHolderMut<'r> {
        unsafe { ResourcesHolderMut::new(&raw mut self.world.res, &self.types) }
    }
    pub fn entities<'r>(&'r self) -> EntitiesHolderRef<'r> {
        unsafe { EntitiesHolderRef::new(&raw const self.world.ent, &self.types) }
    }
    pub fn entities_mut<'r>(&'r mut self) -> EntitiesHolderMut<'r> {
        unsafe { EntitiesHolderMut::new(&raw mut self.world.ent, &self.types) }
    }
    pub fn events<'r>(&'r self) -> EventsHolderRef<'r> {
        unsafe { EventsHolderRef::new(&raw const self.world.evt, &self.types) }
    }
    pub fn events_mut<'r>(&'r mut self) -> EventsHolderMut<'r> {
        unsafe { EventsHolderMut::new(&raw mut self.world.evt, &self.types) }
    }

    // todo
    // pub unsafe fn ffi(&self) -> *mut WorldDataUnsafeFfi {
    //     self.world
    // }

    pub fn as_tuple_mut<'r>(&'r mut self) -> (ResourcesHolderMut<'r>, EntitiesHolderMut<'r>, EventsHolderMut<'r>) {
        unsafe {
            let res = &raw mut self.world.res;
            let world = &mut *self.world;
            (
                ResourcesHolderMut::new(res, &self.types),
                EntitiesHolderMut::new(&mut world.ent, &self.types),
                EventsHolderMut::new(&mut world.evt, &self.types),
            )
        }
    }

    pub fn as_mut<'r>(&'r mut self) -> WorldDataMut<'r> {
        WorldDataMut {
            world: self.world.as_raw(),
            types: &self.types,
        }
    }

    pub fn as_ref<'r>(&'r self) -> WorldDataRef<'r> {
        WorldDataRef {
            world: self.world.as_raw(),
            types: &self.types,
        }
    }
}

unsafe impl<'w> Send for WorldData {}
unsafe impl<'w> Sync for WorldData {}

//

pub struct WorldDataMut<'w> {
    world: *mut WorldDataUnsafeFfi,
    types: &'w TypesRegistryCache,
}

impl<'w> WorldDataMut<'w> {
    pub unsafe fn new(world: *mut WorldDataUnsafeFfi, types: &'w TypesRegistryCache) -> Self {
        Self { world, types }
    }

    pub fn into_resources(self) -> ResourcesHolderRef<'w> {
        unsafe { ResourcesHolderRef::new(&raw mut (*self.world).res, self.types) }
    }
    pub fn into_resources_mut(self) -> ResourcesHolderMut<'w> {
        unsafe { ResourcesHolderMut::new(&raw mut (*self.world).res, self.types) }
    }
    pub fn into_entities(self) -> EntitiesHolderRef<'w> {
        unsafe { EntitiesHolderRef::new(&raw mut (*self.world).ent, self.types) }
    }
    pub fn into_entities_mut(self) -> EntitiesHolderMut<'w> {
        unsafe { EntitiesHolderMut::new(&raw mut (*self.world).ent, self.types) }
    }
    pub fn into_events(self) -> EventsHolderRef<'w> {
        unsafe { EventsHolderRef::new(&raw mut (*self.world).evt, self.types) }
    }
    pub fn into_events_mut(self) -> EventsHolderMut<'w> {
        unsafe { EventsHolderMut::new(&raw mut (*self.world).evt, self.types) }
    }

    pub unsafe fn ffi(&self) -> *mut WorldDataUnsafeFfi {
        self.world
    }

    pub fn into_tuple_mut(self) -> (ResourcesHolderMut<'w>, EntitiesHolderMut<'w>, EventsHolderMut<'w>) {
        unsafe {
            (
                ResourcesHolderMut::new(&raw mut (*self.world).res, self.types),
                EntitiesHolderMut::new(&raw mut (*self.world).ent, self.types),
                EventsHolderMut::new(&raw mut (*self.world).evt, self.types),
            )
        }
    }
}

impl<'w: 'r, 'r> WorldDataMut<'w> {
    pub fn resources(&'r self) -> ResourcesHolderRef<'r> {
        self.as_ref().resources()
    }
    pub fn resources_mut(&'r mut self) -> ResourcesHolderMut<'r> {
        self.as_mut().into_resources_mut()
    }
    pub fn entities(&'r self) -> EntitiesHolderRef<'r> {
        self.as_ref().entities()
    }
    pub fn entities_mut(&'r mut self) -> EntitiesHolderMut<'r> {
        self.as_mut().into_entities_mut()
    }
    pub fn events(&'r self) -> EventsHolderRef<'r> {
        self.as_ref().events()
    }
    pub fn events_mut(&'r mut self) -> EventsHolderMut<'r> {
        self.as_mut().into_events_mut()
    }

    pub fn as_tuple_mut(&'r mut self) -> (ResourcesHolderMut<'r>, EntitiesHolderMut<'r>, EventsHolderMut<'r>) {
        self.as_mut().into_tuple_mut()
    }
    
    pub fn as_mut(&'r mut self) -> WorldDataMut<'r> {
        WorldDataMut {
            world: self.world,
            types: self.types,
        }
    }

    pub fn as_ref(&'r self) -> WorldDataRef<'r> {
        WorldDataRef {
            world: self.world,
            types: self.types,
        }
    }
}

unsafe impl<'w> Send for WorldDataMut<'w> {}
unsafe impl<'w> Sync for WorldDataMut<'w> {}

//

#[derive(Copy, Clone)]
pub struct WorldDataRef<'w> {
    world: *mut WorldDataUnsafeFfi,
    types: &'w TypesRegistryCache,
}

impl<'w> WorldDataRef<'w> {
    pub unsafe fn new(world: *mut WorldDataUnsafeFfi, types: &'w TypesRegistryCache) -> Self {
        Self { world, types }
    }

    pub fn resources(self) -> ResourcesHolderRef<'w> {
        unsafe { ResourcesHolderRef::new(&raw mut (*self.world).res, self.types) }
    }
    pub fn entities(self) -> EntitiesHolderRef<'w> {
        unsafe { EntitiesHolderRef::new(&raw mut (*self.world).ent, self.types) }
    }
    pub fn events(self) -> EventsHolderRef<'w> {
        unsafe { EventsHolderRef::new(&raw mut (*self.world).evt, self.types) }
    }

    pub unsafe fn ffi(&self) -> *mut WorldDataUnsafeFfi {
        self.world
    }

    pub unsafe fn into_mut(self) -> WorldDataMut<'w> {
        WorldDataMut {
            world: self.world,
            types: self.types,
        }
    }
}
