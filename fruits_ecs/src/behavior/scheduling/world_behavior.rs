use crate::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Schedule {
    Start = 0,
    Update = 1,
    RenderCamera = 2,
}

impl Schedule {
    pub const COUNT: usize = 3;
    pub const fn index(self) -> usize {
        self as usize
    }
}

//

#[repr(C)]
pub struct WorldBehaviorBuilderFfi {
    schedules: [SystemsHolderBuilderFfi; Schedule::COUNT],
}

impl WorldBehaviorBuilderFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            schedules: core::array::from_fn(|_| SystemsHolderBuilderFfi::new(types.clone())),
        }
    }

    pub fn get_mut(&mut self, schedule: Schedule) -> &mut SystemsHolderBuilderFfi {
        &mut self.schedules[schedule.index()]
    }

    pub fn clear(&mut self) {
        for schedule in &mut self.schedules {
            schedule.clear();
        }
    }

    pub fn build(self) -> WorldBehaviorFfi {
        WorldBehaviorFfi {
            schedules: self.schedules.map(|mut b| b.build()),
        }
    }
}

//

#[repr(C)]
pub struct WorldBehaviorFfi {
    schedules: [SystemsHolderFfi; Schedule::COUNT],
}

impl WorldBehaviorFfi {
    pub fn get_mut(&mut self, schedule: Schedule) -> &mut SystemsHolderFfi {
        &mut self.schedules[schedule.index()]
    }
}

//

pub struct WorldBehaviorBuilderMut<'w> {
    world: &'w mut WorldBehaviorBuilderFfi,
    types: &'w TypesRegistryCache,
}

impl<'w> WorldBehaviorBuilderMut<'w> {
    pub fn new(world: &'w mut WorldBehaviorBuilderFfi, types: &'w TypesRegistryCache) -> Self {
        Self { world, types }
    }

    pub fn get_mut<'r>(&'r mut self, schedule: Schedule) -> SystemsHolderBuilderMut<'r>
    where
        'w: 'r,
    {
        SystemsHolderBuilderMut::new(self.world.get_mut(schedule), self.types)
    }

    pub fn clear(&mut self) {
        self.world.clear();
    }
}
