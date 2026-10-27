use std::{collections::BTreeSet, hash::Hash};

use fruits_ffi::FfiVec;

#[repr(transparent)]
#[derive(Clone, Default, Debug)]
pub struct UniqueComponentsSet {
    components: FfiVec<u64>,
}

impl UniqueComponentsSet {
    pub fn components(&self) -> &[u64] {
        &self.components
    }
}

impl Hash for UniqueComponentsSet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for component_id in &self.components {
            component_id.hash(state);
        }
    }
}

impl PartialEq for UniqueComponentsSet {
    fn eq(&self, other: &Self) -> bool {
        if self.components.len() != other.components.len() {
            return false;
        }

        for i in 0..(self.components.len()) {
            if self.components[i] != other.components[i] {
                return false;
            }
        }

        return true;
    }
}

impl Eq for UniqueComponentsSet {}

//

#[derive(Clone, Default, Debug)]
pub struct UniqueComponentsSetBuilder {
    components: BTreeSet<u64>,
}

impl UniqueComponentsSetBuilder {
    pub fn new() -> Self {
        Self {
            components: BTreeSet::new(),
        }
    }

    pub fn from_set(set: &UniqueComponentsSet) -> Self {
        Self {
            components: set.components.iter().copied().collect(),
        }
    }

    pub fn components(&self) -> &BTreeSet<u64> {
        &self.components
    }

    pub fn insert(&mut self, type_id: u64) -> bool {
        self.components.insert(type_id)
    }

    pub fn remove(&mut self, type_id: u64) -> bool {
        self.components.remove(&type_id)
    }

    pub fn build(&self) -> UniqueComponentsSet {
        UniqueComponentsSet {
            components: self.components.iter().copied().collect(),
        }
    }
}

impl Hash for UniqueComponentsSetBuilder {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for component_id in &self.components {
            component_id.hash(state);
        }
    }
}

impl PartialEq for UniqueComponentsSetBuilder {
    fn eq(&self, other: &Self) -> bool {
        if self.components.len() != other.components.len() {
            return false;
        }

        self.components.iter().all(|c| other.components.contains(c))
    }
}

impl Eq for UniqueComponentsSetBuilder {}
