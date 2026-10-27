use fruits_ffi::{FfiIndexMap, FfiIndexSet, FfiVec};

use crate::*;

// todo: merge with the ffi version (keep it ffi-friendly)
#[repr(C)]
pub struct ArchetypesHolderFfi {
    types: TypesRegistryAccessFfi,
    archetype_id_by_components: FfiIndexMap<UniqueComponentsSet, u64>,
    archetype_ids_by_component: FfiIndexMap<u64, (FfiIndexSet<u64>, FfiVec<u64>)>,
    archetypes: FfiVec<ArchetypeUnsafeFfi>,
}

impl ArchetypesHolderFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            types,
            archetype_id_by_components: Default::default(),
            archetype_ids_by_component: Default::default(),
            archetypes: Default::default(),
        }
    }

    pub fn len(&self) -> u64 {
        self.archetypes.len() as u64
    }

    pub fn by_id_ref(&self, id: u64) -> Option<&ArchetypeUnsafeFfi> {
        self.archetypes.get(id)
    }
    pub fn by_id_mut(&mut self, id: u64) -> Option<&mut ArchetypeUnsafeFfi> {
        self.archetypes.get_mut(id)
    }
    pub fn by_2_ids_ref(&self, mut id: [u64; 2]) -> Option<[&ArchetypeUnsafeFfi; 2]> {
        if id[0] >= self.archetypes.len() || id[1] >= self.archetypes.len() {
            return None;
        }

        let should_swap = id[0] > id[1];

        if should_swap {
            id = [id[1], id[0]];
        }

        let mut slices = self.archetypes.as_slice()[id[0] as usize..].split_at((id[1] - id[0]) as usize);

        if should_swap {
            (slices.0, slices.1) = (slices.1, slices.0);
        }

        Some([&slices.0[0], &slices.1[0]])
    }
    pub fn by_2_ids_mut(&mut self, mut id: [u64; 2]) -> Option<[&mut ArchetypeUnsafeFfi; 2]> {
        if id[0] >= self.archetypes.len() || id[1] >= self.archetypes.len() {
            return None;
        }

        let should_swap = id[0] > id[1];

        if should_swap {
            id = [id[1], id[0]];
        }

        let mut slices = self.archetypes.as_mut_slice()[id[0] as usize..].split_at_mut((id[1] - id[0]) as usize);

        if should_swap {
            (slices.0, slices.1) = (slices.1, slices.0);
        }

        Some([&mut slices.0[0], &mut slices.1[0]])
    }
    pub fn by_components_ref(&self, components: &UniqueComponentsSet) -> Option<&ArchetypeUnsafeFfi> {
        let id = self.id_by_components(components)?;
        self.by_id_ref(id)
    }
    pub fn by_components_mut(&mut self, components: &UniqueComponentsSet) -> Option<&mut ArchetypeUnsafeFfi> {
        let id = self.id_by_components(components)?;
        self.by_id_mut(id)
    }
    pub fn id_by_components(&self, components: &UniqueComponentsSet) -> Option<u64> {
        self.archetype_id_by_components.get(components).copied()
    }
    pub fn ids_by_component(&self, component: u64) -> Option<&FfiVec<u64>> {
        self.archetype_ids_by_component.get(&component).map(|t| &t.1)
    }
    fn create(&mut self, components: UniqueComponentsSet) -> Result<u64, UniqueComponentsSet> {
        if self.archetype_id_by_components.contains_key(&components) {
            return Err(components);
        }

        let id = self.archetype_id_by_components.len() as u64;

        for &component_id in components.components() {
            // todo: optimize with "entry()" api.
            // let (set, vec) = self.archetype_ids_by_component.entry(component_id).or_default();
            if !self.archetype_ids_by_component.contains_key(&component_id) {
                self.archetype_ids_by_component.insert(component_id, Default::default());
            }
            let (set, vec) = self.archetype_ids_by_component.get_mut(&component_id).unwrap();

            if set.insert(id) {
                vec.push(id);
            }
        }

        self.archetypes
            .push(ArchetypeUnsafeFfi::new(self.types.clone(), components.clone()));
        self.archetype_id_by_components.insert(components, id);

        Ok(id)
    }
    pub fn id_by_components_or_create(&mut self, components: UniqueComponentsSet) -> u64 {
        match self.id_by_components(&components) {
            Some(id) => id,
            None => self.create(components).ok().unwrap(),
        }
    }
}
