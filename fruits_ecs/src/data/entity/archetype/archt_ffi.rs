use std::ffi::c_void;

use fruits_ffi::FfiExtendedTypeInfo;

use crate::*;

#[repr(C)]
pub struct ArchetypeUnsafeFfiIteratorItem {
    pub chunk_ptr: *mut u8,
    pub chunk_entity_idx: u64,
}

pub struct ArchetypeUnsafeFfiIterator<'a> {
    archetype: &'a ArchetypeUnsafeFfi,
    entity_archetype_idx: u64,
}

impl<'a> ArchetypeUnsafeFfiIterator<'a> {
    pub fn new(archetype: &'a ArchetypeUnsafeFfi) -> Self {
        Self {
            archetype,
            entity_archetype_idx: 0,
        }
    }
}

impl<'a> Iterator for ArchetypeUnsafeFfiIterator<'a> {
    type Item = ArchetypeUnsafeFfiIteratorItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.entity_archetype_idx >= self.archetype.alive_entities_count {
            return None;
        }

        let chunk_entity = self.archetype.layout.entity_in_chunk_index(self.entity_archetype_idx);
        let chunk_ptr = self
            .archetype
            .archetype
            .get_chunk(self.archetype.layout.chunk_index(self.entity_archetype_idx));

        self.entity_archetype_idx += 1;

        Some(ArchetypeUnsafeFfiIteratorItem {
            chunk_ptr,
            chunk_entity_idx: chunk_entity,
        })
    }
}

//

// todo: drop components on Archetype drop?
#[repr(C)]
pub struct ArchetypeUnsafeFfi {
    layout: ArchetypeLayout,
    archetype: ArchetypeRaw,
    alive_entities_count: u64,
}

impl ArchetypeUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi, components_set: UniqueComponentsSet) -> Self {
        Self {
            layout: ArchetypeLayout::new(types, components_set),
            archetype: ArchetypeRaw::new(),
            alive_entities_count: 0,
        }
    }

    pub fn contains_component_type(&self, type_id: u64) -> bool {
        self.layout.get_component(type_id).is_some()
    }

    pub fn iter<'a>(&'a self) -> ArchetypeUnsafeFfiIterator<'a> {
        ArchetypeUnsafeFfiIterator::new(&self)
    }

    pub fn entities_count(&self) -> u64 {
        self.alive_entities_count
    }

    pub fn get_entity(&self, entity_index: u64) -> Option<EntityId> {
        if entity_index >= self.alive_entities_count {
            return None;
        }

        Some(unsafe {
            let physical_location = self.layout.entity_memory_physical_location(entity_index);

            let memory = self.archetype.get_memory(&physical_location);

            (memory as *const EntityId).read()
        })
    }

    pub fn get_component_ptr(&self, entity_index: u64, component_id: u64) -> Option<*mut u8> {
        if entity_index >= self.alive_entities_count {
            return None;
        }

        if self.layout.get_component(component_id).is_none() {
            return None;
        }

        let physical_location = self.layout.component_memory_physical_location(entity_index, component_id);

        Some(self.archetype.get_memory(&physical_location))
    }

    fn create_place_for_entity(&mut self) -> u64 {
        // todo: initialize components
        let entity_in_archetype_index = self.alive_entities_count;

        if self.layout.chunk_index(entity_in_archetype_index) as u64 >= self.archetype.chunks_count() {
            self.archetype.push_chunk();
        }

        self.alive_entities_count += 1;

        entity_in_archetype_index
    }

    pub fn create_entity(&mut self, entity: EntityId) {
        let entity_in_archetype_index = self.create_place_for_entity();

        unsafe {
            let entity_location = self.layout.entity_memory_physical_location(entity_in_archetype_index);

            let memory = self.archetype.get_memory(&entity_location);

            (memory as *mut EntityId).write(entity);
        }
    }

    /// Returns the last entity before the destroy.
    pub fn destroy_entity(&mut self, entity_index: u64) -> Option<EntityId> {
        if entity_index >= self.alive_entities_count {
            return None;
        }

        for item_layout in self.layout.components() {
            let item_layout = item_layout.into_item_layout();

            unsafe {
                let location = self.layout.memory_physical_location(entity_index, &item_layout);

                let memory = self.archetype.get_memory(&location);

                item_layout.type_info.drop(memory as *mut c_void);
            }
        }

        self.erase_entity(entity_index)
    }

    fn erase_entity(&mut self, entity_index: u64) -> Option<EntityId> {
        // todo: Release the unneded chunks?
        if entity_index >= self.alive_entities_count {
            return None;
        }

        let last_index = self.alive_entities_count - 1;

        if entity_index != last_index {
            let last_components_locations = Self::get_items_locations_iter(&self.layout, last_index, &self.layout);
            let entity_components_locations = Self::get_items_locations_iter(&self.layout, entity_index, &self.layout);

            for ((src_location, size), (dst_location, _)) in last_components_locations.zip(entity_components_locations) {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.archetype.get_memory(&src_location),
                        self.archetype.get_memory(&dst_location),
                        size,
                    )
                }
            }
        }

        let last_entity_location = self.layout.entity_memory_physical_location(last_index);

        let last_entity = unsafe {
            let last_enity_memory = self.archetype.get_memory(&last_entity_location);

            (last_enity_memory as *const EntityId).read()
        };

        self.alive_entities_count -= 1;

        Some(last_entity)
    }

    /// Returns the last entity from src archetype before the movement.
    ///
    /// # Safety
    ///
    /// Memory is automatically deallocated. Needs manual lifetime-management.
    pub unsafe fn add_component(
        src: &mut Self,
        dst: &mut Self,
        src_entity_index: u64,
        component_ptr: *mut u8,
        component_id: u64,
    ) -> Option<EntityId> {
        if !ArchetypeLayout::is_component_the_only_difference(&dst.layout, &src.layout, component_id) {
            return None;
        }

        let dst_entity_index = dst.create_place_for_entity();

        let src_components_locations = Self::get_items_locations_iter(&src.layout, src_entity_index, &src.layout);
        let dst_components_locations = Self::get_items_locations_iter(&src.layout, dst_entity_index, &dst.layout);

        for ((src_location, size), (dst_location, _)) in src_components_locations.zip(dst_components_locations) {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src.archetype.get_memory(&src_location),
                    dst.archetype.get_memory(&dst_location),
                    size,
                );
            }
        }

        let item_layout = dst.layout.get_component(component_id).unwrap();
        let added_component_location = dst.layout.memory_physical_location(dst_entity_index, item_layout);

        unsafe {
            let added_mem = dst.archetype.get_memory(&added_component_location);

            std::ptr::copy_nonoverlapping(component_ptr, added_mem, item_layout.type_info.short().size() as usize);
        }

        Some(src.erase_entity(src_entity_index).unwrap())
    }

    pub unsafe fn replace_components(&mut self, entity_index: u64, components: &[ComponentPtr]) -> bool {
        if entity_index >= self.alive_entities_count {
            return false;
        }

        if !components.iter().all(|c| self.layout.get_component(c.id).is_some()) {
            return false;
        }

        for &ComponentPtr { ptr: component_ptr, id: component_id } in components {
            let item_layout = self.layout.get_component(component_id).unwrap();
            let location = self.layout.memory_physical_location(entity_index, item_layout);

            unsafe {
                let memory = self.archetype.get_memory(&location);

                item_layout.type_info.drop(memory as *mut c_void);

                std::ptr::copy_nonoverlapping(component_ptr, memory, item_layout.type_info.short().size() as usize);
            }
        }

        true
    }

    pub unsafe fn set_components(
        src: &mut Self,
        dst: &mut Self,
        src_entity_index: u64,
        components: &[ComponentPtr],
    ) -> Option<EntityId> {
        if src_entity_index >= src.alive_entities_count {
            return None;
        }

        let is_in_components = |id: u64| components.iter().any(|c| c.id == id);

        let new_types_count = components.iter().filter(|c| src.layout.get_component(c.id).is_none()).count();

        if dst.layout.components().len() != src.layout.components().len() + new_types_count
            || !src.layout.components().iter().all(|c| dst.layout.get_component(c.type_info.id).is_some())
            || !components.iter().all(|c| dst.layout.get_component(c.id).is_some())
        {
            return None;
        }

        let dst_entity_index = dst.create_place_for_entity();

        for component in src.layout.components() {
            let src_item_layout = src.layout.get_component(component.type_info.id).unwrap();
            let src_location = src.layout.memory_physical_location(src_entity_index, src_item_layout);
            let src_memory = src.archetype.get_memory(&src_location);

            unsafe {
                if is_in_components(component.type_info.id) {
                    src_item_layout.type_info.drop(src_memory as *mut c_void);
                } else {
                    let dst_item_layout = dst.layout.get_component(component.type_info.id).unwrap();
                    let dst_location = dst.layout.memory_physical_location(dst_entity_index, dst_item_layout);

                    std::ptr::copy_nonoverlapping(
                        src_memory,
                        dst.archetype.get_memory(&dst_location),
                        src_item_layout.type_info.short().size() as usize,
                    );
                }
            }
        }

        unsafe {
            std::ptr::copy_nonoverlapping(
                src.archetype.get_memory(&src.layout.entity_memory_physical_location(src_entity_index)),
                dst.archetype.get_memory(&dst.layout.entity_memory_physical_location(dst_entity_index)),
                src.layout.entity_item_layout().type_info.short().size() as usize,
            );
        }

        for &ComponentPtr { ptr: component_ptr, id: component_id } in components {
            let item_layout = dst.layout.get_component(component_id).unwrap();
            let location = dst.layout.memory_physical_location(dst_entity_index, item_layout);

            unsafe {
                std::ptr::copy_nonoverlapping(component_ptr, dst.archetype.get_memory(&location), item_layout.type_info.short().size() as usize);
            }
        }

        Some(src.erase_entity(src_entity_index).unwrap())
    }

    fn get_items_locations_iter<'l>(
        componens_layout: &'l ArchetypeLayout,
        entity_in_archetype_index: u64,
        memory_layout: &'l ArchetypeLayout,
    ) -> impl Iterator<Item = (ArchetypeItemPhysicalLocation, usize)> + 'l {
        componens_layout
            .components()
            .iter()
            .map(move |t| {
                let item_layout = memory_layout.get_component(t.type_info.id).unwrap();
                (
                    memory_layout.memory_physical_location(entity_in_archetype_index, item_layout),
                    item_layout.type_info.short().size() as usize,
                )
            })
            .chain(std::iter::once({
                let item_layout = memory_layout.entity_item_layout();
                (
                    memory_layout.entity_memory_physical_location(entity_in_archetype_index),
                    item_layout.type_info.short().size() as usize,
                )
            }))
    }

    /// Returns the last entity from src archetype before the movement.
    pub unsafe fn remove_component(
        src: &mut Self,
        dst: &mut Self,
        src_entity_index: u64,
        component_mover: impl FnOnce(*mut u8, &'static FfiExtendedTypeInfo),
        component_id: u64,
    ) -> Option<EntityId> {
        if !ArchetypeLayout::is_component_the_only_difference(&src.layout, &dst.layout, component_id) {
            return None;
        }

        let dst_entity_index = dst.create_place_for_entity();

        let src_components_locations = Self::get_items_locations_iter(&dst.layout, src_entity_index, &src.layout);
        let dst_components_locations = Self::get_items_locations_iter(&dst.layout, dst_entity_index, &dst.layout);

        for ((src_location, size), (dst_location, _)) in src_components_locations.zip(dst_components_locations) {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src.archetype.get_memory(&src_location),
                    dst.archetype.get_memory(&dst_location),
                    size,
                )
            }
        }

        // Safety: Managed by caller.
        unsafe {
            let item_layout = src.layout.get_component(component_id).unwrap();
            let removed_component_location = src.layout.memory_physical_location(src_entity_index, item_layout);
            let removed_ptr = src.archetype.get_memory(&removed_component_location);

            component_mover(removed_ptr, item_layout.type_info);
        };

        Some(src.erase_entity(src_entity_index).unwrap())
    }

    pub fn layout(&self) -> &ArchetypeLayout {
        &self.layout
    }

    pub unsafe fn raw_archetype(&self) -> &ArchetypeRaw {
        &self.archetype
    }
}
