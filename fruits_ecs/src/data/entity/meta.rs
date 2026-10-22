use std::fmt::Debug;

use fruits_utils::index_version_collection::{VersionCollection, VersionIndex};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(VersionIndex);

impl EntityId {
    pub const EMPTY: Self = Self(VersionIndex::EMPTY);

    pub fn version_index(&self) -> VersionIndex {
        self.0
    }

    pub fn from_version_index(vi: VersionIndex) -> Self {
        Self(vi)
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Debug for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityId")
            .field("i", &self.0.index)
            .field("v", &self.0.version)
            .finish()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EntityLocation {
    pub archetype_id: u64,
    pub entity_archetype_index: u64,
}

#[repr(transparent)]
#[derive(Default)]
pub struct EntitiesMetadata(VersionCollection<EntityLocation>);

impl EntitiesMetadata {
    pub fn new() -> Self {
        Self(VersionCollection::new())
    }

    pub fn insert(&mut self, location: EntityLocation) -> EntityId {
        EntityId(self.0.insert(location))
    }

    pub fn remove(&mut self, entity: EntityId) -> Option<EntityLocation> {
        self.0.remove(entity.0)
    }

    pub fn get_by_idx_only(&self, idx: u64) -> Option<EntityId> {
        self.0.get_by_idx_only(idx).map(EntityId::from_version_index)
    }

    pub fn get(&self, entity: EntityId) -> Option<&EntityLocation> {
        self.0.get(entity.0)
    }

    pub fn get_mut(&mut self, entity: EntityId) -> Option<&mut EntityLocation> {
        self.0.get_mut(entity.0)
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.0.contains_index(entity.0)
    }

    pub fn len(&self) -> u64 {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
