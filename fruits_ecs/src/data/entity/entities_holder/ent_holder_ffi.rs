use std::ffi::c_void;

use fruits_ffi::{FfiAny, FfiAnyRef, FfiExtendedTypeInfo, FfiFnMutMut, FfiOpaqueRef, FfiVec};

use crate::*;


#[repr(C)]
pub struct EntitiesHolderUnsafeFfi {
    archetypes: ArchetypesHolderFfi,
    entities_meta: EntitiesMetadata,
}

impl EntitiesHolderUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            archetypes: ArchetypesHolderFfi::new(types),
            entities_meta: EntitiesMetadata::new(),
        }
    }

    pub fn archetypes(&self) -> &ArchetypesHolderFfi {
        &self.archetypes
    }

    pub fn entities_meta(&self) -> &EntitiesMetadata {
        &self.entities_meta
    }

    // todo
    // pub unsafe fn query<'a, A: ArchetypeIteratorItem, F: QueryFilter>(&'a self) -> EntitiesHolderQuery<'a, A, F> {
    //     unsafe { EntitiesHolderQuery::<'a, A, F>::new(self.as_ref()) }
    // }

    pub fn entities_count(&self) -> u64 {
        self.entities_meta.len()
    }

    pub fn contains_entity(&self, entity: EntityId) -> bool {
        self.entities_meta.contains(entity)
    }

    pub fn create_entity(&mut self) -> EntityId {
        let archetype_id = self.archetypes.id_by_components_or_create(UniqueComponentsSet::default());

        let archetype = self.archetypes.by_id_mut(archetype_id).unwrap();

        let entity_archetype_index = archetype.entities_count();

        let entity = self.entities_meta.insert(EntityLocation {
            archetype_id,
            entity_archetype_index,
        });

        archetype.create_entity(entity);

        entity
    }

    pub fn destroy_entity(&mut self, entity: EntityId) -> bool {
        let Some(entity_location) = self.entities_meta.remove(entity) else {
            return false;
        };

        let archetype = self.archetypes.by_id_mut(entity_location.archetype_id).unwrap();

        let last_entity = archetype.destroy_entity(entity_location.entity_archetype_index).unwrap();

        if last_entity != entity {
            *self.entities_meta.get_mut(last_entity).unwrap() = entity_location;
        }

        true
    }

    pub unsafe fn add_component(&mut self, entity: EntityId, component_ptr: *mut u8, component_id: u64) -> bool {
        let Some(entity_location) = self.entities_meta.get(entity) else {
            return false;
        };

        let src_archetype_id = entity_location.archetype_id;

        let mut dst_components_set = {
            let src_archetype = self.archetypes.by_id_ref(src_archetype_id).unwrap();

            UniqueComponentsSetBuilder::from_set(src_archetype.layout().components_set())
        };

        if !dst_components_set.insert(component_id) {
            return false;
        }

        let dst_archetype_id = self.archetypes.id_by_components_or_create(dst_components_set.build());

        let [src_archetype, dst_archetype] = self.archetypes.by_2_ids_mut([src_archetype_id, dst_archetype_id]).unwrap();

        let entity_with_added_component_new_location = EntityLocation {
            archetype_id: dst_archetype_id,
            entity_archetype_index: dst_archetype.entities_count(),
        };

        // Safety. Access is unique and is only used in the method scope.
        let last_entity = unsafe {
            ArchetypeUnsafeFfi::add_component(
                src_archetype,
                dst_archetype,
                entity_location.entity_archetype_index,
                component_ptr,
                component_id,
            )
            .unwrap()
        };

        if last_entity != entity {
            *self.entities_meta.get_mut(last_entity).unwrap() = *entity_location;
        }

        *self.entities_meta.get_mut(entity).unwrap() = entity_with_added_component_new_location;

        true
    }

    pub unsafe fn remove_component(
        &mut self,
        entity: EntityId,
        component_mover: impl FnOnce(*mut u8, &'static FfiExtendedTypeInfo),
        component_id: u64,
    ) -> bool {
        let Some(entity_location) = self.entities_meta.get(entity) else {
            return false;
        };

        let src_archetype_id = entity_location.archetype_id;

        let mut dst_components_set = {
            let src_archetype = self.archetypes.by_id_ref(src_archetype_id).unwrap();

            UniqueComponentsSetBuilder::from_set(src_archetype.layout().components_set())
        };

        if !dst_components_set.remove(component_id) {
            return false;
        }

        let dst_archetype_id = self.archetypes.id_by_components_or_create(dst_components_set.build());

        let [src_archetype, dst_archetype] = self.archetypes.by_2_ids_mut([src_archetype_id, dst_archetype_id]).unwrap();

        let entity_with_removed_component_new_location = EntityLocation {
            archetype_id: dst_archetype_id,
            entity_archetype_index: dst_archetype.entities_count(),
        };

        // Safety. Access is unique and is only used in the method scope.
        let last_entity = unsafe {
            ArchetypeUnsafeFfi::remove_component(
                src_archetype,
                dst_archetype,
                entity_location.entity_archetype_index,
                component_mover,
                component_id,
            )
            .unwrap()
        };

        if last_entity != entity {
            *self.entities_meta.get_mut(last_entity).unwrap() = *entity_location;
        }

        *self.entities_meta.get_mut(entity).unwrap() = entity_with_removed_component_new_location;

        true
    }

    pub fn get_component_ptr(&self, entity: EntityId, component_id: u64) -> Option<*mut u8> {
        let entity_location = self.entities_meta.get(entity)?;

        self.archetypes
            .by_id_ref(entity_location.archetype_id)
            .unwrap()
            .get_component_ptr(entity_location.entity_archetype_index, component_id)
    }

    // todo: to iterator
    pub fn get_all_components_ptrs(&self, entity: EntityId, mut handler: FfiFnMutMut<(*mut u8, &'static FfiExtendedTypeInfo), ()>) {
        let Some(entity_location) = self.entities_meta.get(entity) else {
            return;
        };

        let arch = self.archetypes
            .by_id_ref(entity_location.archetype_id)
            .unwrap();

        arch.layout().components().iter().map(|c| {
            let ptr = arch.get_component_ptr(entity_location.entity_archetype_index, c.type_info.id).unwrap();

            handler.execute((ptr, c.type_info.data))
        }).collect()
    }
}
