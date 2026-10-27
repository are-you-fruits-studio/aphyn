use crate::{
    Schedule, TypesRegistryAccessFfi, TypesRegistryCache, WorldBehaviorBuilderFfi, WorldBehaviorBuilderMut, WorldBehaviorFfi,
    WorldDataMut, WorldDataRef, WorldDataUnsafeFfi,
};

#[repr(C)]
pub struct WorldBuilderUnsafeFfi {
    data: WorldDataUnsafeFfi,
    behavior: WorldBehaviorBuilderFfi,
}

impl WorldBuilderUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            data: WorldDataUnsafeFfi::new(types.clone()),
            behavior: WorldBehaviorBuilderFfi::new(types),
        }
    }

    pub fn build(self) -> WorldUnsafeFfi {
        WorldUnsafeFfi {
            data: self.data,
            behavior: self.behavior.build(),
        }
    }
}

#[repr(C)]
pub struct WorldUnsafeFfi {
    data: WorldDataUnsafeFfi,
    behavior: WorldBehaviorFfi,
}

impl WorldUnsafeFfi {
    pub fn execute_iteration(&mut self, schedule: Schedule) {
        self.behavior.get_mut(schedule).execute_iteration(&raw mut self.data);
    }
}

//

pub struct World {
    world: WorldUnsafeFfi,
    types: TypesRegistryCache,
}

impl World {
    pub fn data<'r>(&'r self) -> WorldDataRef<'r> {
        unsafe { WorldDataRef::new(&raw const self.world.data as *mut WorldDataUnsafeFfi, &self.types) }
    }
    pub fn data_mut<'r>(&'r mut self) -> WorldDataMut<'r> {
        unsafe { WorldDataMut::new(&raw mut self.world.data, &self.types) }
    }
    pub fn execute_iteration(&mut self, schedule: Schedule) {
        self.world.execute_iteration(schedule)
    }
    pub fn as_mut<'r>(&'r mut self) -> WorldMut<'r> {
        WorldMut {
            world: &mut self.world,
            types: &self.types,
        }
    }
}

pub struct WorldMut<'w> {
    world: &'w mut WorldUnsafeFfi,
    types: &'w TypesRegistryCache,
}

impl<'w> WorldMut<'w> {
    pub fn data<'r>(&'r self) -> WorldDataRef<'r>
    where
        'w: 'r,
    {
        unsafe { WorldDataRef::new(&raw const self.world.data as *mut WorldDataUnsafeFfi, self.types) }
    }
    pub fn data_mut<'r>(&'r mut self) -> WorldDataMut<'r>
    where
        'w: 'r,
    {
        unsafe { WorldDataMut::new(&raw mut self.world.data, self.types) }
    }
    pub fn execute_iteration(&mut self, schedule: Schedule) {
        self.world.execute_iteration(schedule)
    }
}

pub struct WorldBuilder {
    world: WorldBuilderUnsafeFfi,
    types: TypesRegistryCache,
}

impl WorldBuilder {
    pub fn new() -> Self {
        let types = TypesRegistryAccessFfi::new();

        Self {
            world: WorldBuilderUnsafeFfi::new(types.clone()),
            types: TypesRegistryCache::new(types),
        }
    }

    pub fn data_mut<'r>(&'r mut self) -> WorldDataMut<'r> {
        unsafe { WorldDataMut::new(&raw mut self.world.data, &self.types) }
    }

    pub fn behavior_mut<'r>(&'r mut self) -> WorldBehaviorBuilderMut<'r> {
        WorldBehaviorBuilderMut::new(&mut self.world.behavior, &self.types)
    }

    pub fn build(self) -> World {
        World {
            world: self.world.build(),
            types: self.types,
        }
    }

    pub fn as_mut<'r>(&'r mut self) -> WorldBuilderMut<'r> {
        WorldBuilderMut {
            world: &mut self.world,
            types: &self.types,
        }
    }

    pub unsafe fn into_raw_parts(&mut self) -> (&mut WorldBuilderUnsafeFfi, &TypesRegistryCache) {
        (&mut self.world, &self.types)
    }
}

pub struct WorldBuilderMut<'w> {
    world: &'w mut WorldBuilderUnsafeFfi,
    types: &'w TypesRegistryCache,
}

impl<'w> WorldBuilderMut<'w> {
    pub fn new(world: &'w mut WorldBuilderUnsafeFfi, types: &'w TypesRegistryCache) -> Self {
        Self { world, types }
    }

    pub fn data_mut<'r>(&'r mut self) -> WorldDataMut<'r>
    where
        'w: 'r,
    {
        unsafe { WorldDataMut::new(&raw mut self.world.data, self.types) }
    }

    pub fn behavior_mut<'r>(&'r mut self) -> WorldBehaviorBuilderMut<'r>
    where
        'w: 'r,
    {
        WorldBehaviorBuilderMut::new(&mut self.world.behavior, self.types)
    }

    pub fn as_mut<'r>(&'r mut self) -> WorldBuilderMut<'r>
    where
        'w: 'r,
    {
        WorldBuilderMut {
            world: self.world,
            types: self.types,
        }
    }

    pub unsafe fn into_raw_parts(&mut self) -> (&mut WorldBuilderUnsafeFfi, &TypesRegistryCache) {
        (&mut self.world, &self.types)
    }
}
