use std::{collections::{HashMap, HashSet}, fmt::Debug};

use fruits_ffi::FfiVec;

// todo: ffi for builders?

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DataUsageDetails {
    pub is_mut: bool,
    pub is_required: bool,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DataUsageEntry {
    pub type_id: u64,
    pub details: DataUsageDetails,
}

#[repr(C)]
#[derive(Debug)]
pub enum WorldPartDataUsage {
    ByType(FfiVec<DataUsageEntry>),
    Global { is_mut: bool },
}

#[repr(C)]
#[derive(Debug)]
pub struct WorldDataUsage {
    resources: WorldPartDataUsage,
    entities: WorldPartDataUsage,
    events: WorldPartDataUsage,
}

impl WorldDataUsage {
    pub fn resources(&self) -> &WorldPartDataUsage {
        &self.resources
    }
    pub fn entities(&self) -> &WorldPartDataUsage {
        &self.entities
    }
    pub fn events(&self) -> &WorldPartDataUsage {
        &self.events
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct DataUsage {
    world: WorldDataUsage,
    system_types: FfiVec<u64>,
}

impl DataUsage {
    pub fn world(&self) -> &WorldDataUsage {
        &self.world
    }
    pub fn system_types(&self) -> &FfiVec<u64> {
        &self.system_types
    }
}

//

#[derive(Debug)]
pub struct DataUsageDetailsBuilder {
    pub writes: usize,
    pub reads: usize,
    pub is_required: bool,
}

impl DataUsageDetailsBuilder {
    pub fn new() -> Self {
        Self {
            is_required: false,
            reads: 0,
            writes: 0,
        }
    }

    pub fn add(&mut self, usage: DataUsageDetails) {
        self.is_required |= usage.is_required;

        if usage.is_mut {
            self.writes += 1;
        } else {
            self.reads += 1;
        }
    }

    pub fn build(&self) -> Option<DataUsageDetails> {
        if self.writes == 0 && self.reads > 0 {
            Some(DataUsageDetails {
                is_mut: false,
                is_required: self.is_required,
            })
        } else if self.writes == 1 && self.reads == 0 {
            Some(DataUsageDetails {
                is_mut: true,
                is_required: self.is_required,
            })
        } else {
            None
        }
    }
}

pub struct ByTypeWorldPartDataUsageBuilder {
    usage: HashMap<u64, DataUsageDetailsBuilder>,
}

impl Debug for ByTypeWorldPartDataUsageBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(&self.usage).finish()
    }
}

impl ByTypeWorldPartDataUsageBuilder {
    pub fn new() -> Self {
        Self {
            usage: HashMap::new(),
        }
    }

    pub fn add_type(&mut self, usage: DataUsageEntry) {
        self.usage.entry(usage.type_id).or_insert_with(|| DataUsageDetailsBuilder::new()).add(usage.details);
    }

    pub fn build(&self) -> Option<FfiVec<DataUsageEntry>> {
        self.usage.iter()
            .map(|(id, usage)| Some(DataUsageEntry { type_id: *id, details: usage.build()? }))
            .collect()
    }
}

#[derive(Debug)]
pub struct WorldPartDataUsageBuilder {
    by_type: ByTypeWorldPartDataUsageBuilder,
    global_reads: usize,
    global_writes: usize,
}

impl WorldPartDataUsageBuilder {
    pub fn new() -> Self {
        Self {
            by_type: ByTypeWorldPartDataUsageBuilder::new(),
            global_reads: 0,
            global_writes: 0,
        }
    }

    pub fn add_type(&mut self, usage: DataUsageEntry) {
        self.by_type.add_type(usage);
    }

    pub fn add_global(&mut self, is_mut: bool) {
        if is_mut {
            self.global_writes += 1;
        } else {
            self.global_reads += 1;
        }
    }

    pub fn as_by_type(&mut self) -> &mut ByTypeWorldPartDataUsageBuilder {
        &mut self.by_type
    }

    pub fn build(&self) -> Option<WorldPartDataUsage> {
        if self.global_writes == 0 && self.global_reads == 0 {
            Some(WorldPartDataUsage::ByType(self.by_type.build()?))
        } else if self.by_type.usage.is_empty() && self.global_writes == 1 && self.global_reads == 0 {
            Some(WorldPartDataUsage::Global { is_mut: true })
        } else if self.global_writes == 0 && self.global_reads > 0 && self.by_type.usage.values().all(|v| v.writes == 0 && v.reads > 0) {
            Some(WorldPartDataUsage::Global { is_mut: false })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct WorldDataUsageBuilder {
    resources: WorldPartDataUsageBuilder,
    entities: WorldPartDataUsageBuilder,
    events: WorldPartDataUsageBuilder,
}

impl WorldDataUsageBuilder {
    pub fn new() -> Self {
        Self {
            resources: WorldPartDataUsageBuilder::new(),
            entities: WorldPartDataUsageBuilder::new(),
            events: WorldPartDataUsageBuilder::new(),
        }
    }

    pub fn resources(&mut self) -> &mut WorldPartDataUsageBuilder {
        &mut self.resources
    }
    pub fn entities(&mut self) -> &mut WorldPartDataUsageBuilder {
        &mut self.entities
    }
    pub fn events(&mut self) -> &mut WorldPartDataUsageBuilder {
        &mut self.events
    }
    pub fn add_global(&mut self, is_mut: bool) {
        self.resources.add_global(is_mut);
        self.entities.add_global(is_mut);
        self.events.add_global(is_mut);
    }

    pub fn build(&self) -> Option<WorldDataUsage> {
        Some(WorldDataUsage {
            resources: self.resources.build()?,
            entities: self.entities.build()?,
            events: self.events.build()?,
        })
    }
}

#[derive(Debug)]
pub struct DataUsageBuilder {
    world: WorldDataUsageBuilder,
    system_types: HashSet<u64>,
    system_types_duplicates: HashSet<u64>,
}
impl DataUsageBuilder {
    pub fn new() -> Self {
        Self {
            world: WorldDataUsageBuilder::new(),
            system_types: HashSet::new(),
            system_types_duplicates: HashSet::new(),
        }
    }

    pub fn world(&mut self) -> &mut WorldDataUsageBuilder {
        &mut self.world
    }

    pub fn add_system(&mut self, type_id: u64) {
        if !self.system_types.insert(type_id) {
            self.system_types_duplicates.insert(type_id);
        }
    }

    pub fn build(&self) -> Option<DataUsage> {
        if !self.system_types_duplicates.is_empty() {
            return None;
        }

        Some(DataUsage {
            world: self.world.build()?,
            system_types: self.system_types.iter().copied().collect(),
        })
    }
}
