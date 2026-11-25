use std::{ffi::c_void, mem::MaybeUninit};

use aphyn_any::{AnyBox, AnyBoxCtx, AnyMut, AnyRef, AnyTypeId};

use aphyn_ffi::{FfiFnMutMut, FfiSliceRef, FfiVec};

use crate::*;

fn entities_holder_query<'e, A: ArchetypeIteratorItem, F: QueryFilter>(
    entities: &'e EntitiesHolderUnsafeFfi,
    types: &'e TypesRegistryCache,
) -> EntitiesHolderQuery<'e, A, F> {
    unsafe { EntitiesHolderQuery::<'e, A, F>::new(entities, types) }
}

fn entities_holder_add_component<C: 'static>(
    entities: &mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    component: C,
) -> Result<(), C> {
    unsafe {
        let mut component = MaybeUninit::new(component);

        if entities.add_component(entity, component.as_mut_ptr() as *mut u8, types.get_or_register::<C>()) {
            Ok(())
        } else {
            Err(component.assume_init())
        }
    }
}

fn entities_holder_set_component<C: 'static>(
    entities: &mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    component: C,
) -> bool {
    unsafe {
        let mut component = MaybeUninit::new(component);

        match entities.set_component(entity, component.as_mut_ptr() as *mut u8, types.get_or_register::<C>()) {
            PtrState::Idle => {
                component.assume_init_drop();
                false
            }
            PtrState::Uninitialized => true,
            PtrState::Overwritten => {
                component.assume_init_drop();
                true
            }
        }
    }
}

fn entities_holder_component_any_type_id(types: &TypesRegistryCache, component: &AnyBox) -> u64 {
    if component.type_id().tag() == types.tag() {
        return component.type_id().id();
    }

    types.ffi().get_or_register(component.type_info())
}

fn entities_holder_add_component_any(
    entities: &mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    component: AnyBox,
) -> Result<(), AnyBox> {
    unsafe {
        let type_id = entities_holder_component_any_type_id(types, &component);

        if entities.add_component(entity, component.ptr() as *mut u8, type_id) {
            component.dealloc_without_drop();
            Ok(())
        } else {
            Err(component)
        }
    }
}

fn entities_holder_set_components_any(
    entities: &mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    components: FfiVec<AnyBox>,
) -> bool {
    unsafe {
        let components_ptrs = components
            .iter()
            .map(|c| ComponentPtr {
                ptr: c.ptr() as *mut u8,
                id: entities_holder_component_any_type_id(types, c),
            })
            .collect::<FfiVec<_>>();

        let is_set = entities.set_components(entity, FfiSliceRef::from_slice(&components_ptrs));

        if is_set {
            for component in components {
                component.dealloc_without_drop();
            }
        }

        is_set
    }
}

fn entities_holder_remove_component<C: 'static>(
    entities: &mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
) -> Option<C> {
    unsafe {
        let mut component = Option::<C>::None;

        entities.remove_component(
            entity,
            |ptr, _info| component = Some((ptr as *const C).read()),
            types.get_or_register::<C>(),
        );

        component
    }
}

fn entities_holder_remove_component_any(
    entities: &mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    id: AnyTypeId,
) -> Option<AnyBox> {
    if id.tag() != types.tag() {
        return None;
    }

    unsafe {
        let mut component = Option::<AnyBox>::None;

        entities.remove_component(
            entity,
            |ptr, info| {
                component = Some(AnyBox::from_moved(
                    ptr as *const c_void,
                    info,
                    AnyBoxCtx::new(id, types.ffi()),
                ))
            },
            id.id(),
        );

        component
    }
}

fn entities_holder_get_component_ptr<C: 'static>(
    entities: &EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
) -> Option<*mut C> {
    entities
        .get_component_ptr(entity, types.get_or_register::<C>())
        .map(|p| p as *mut C)
}

fn entities_holder_get_component<'r, C: 'static>(
    entities: &'r EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
) -> Option<&'r C> {
    unsafe { entities_holder_get_component_ptr::<C>(entities, types, entity).map(|p| &*p) }
}

fn entities_holder_get_component_any_ptr(
    entities: &EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    id: AnyTypeId,
) -> Option<AnyPtr> {
    if id.tag() != types.tag() {
        return None;
    }

    let info = types.ffi().get(id.id())?;
    let ptr = entities.get_component_ptr(entity, id.id())?;
    let any_ptr = unsafe { AnyPtr::from_raw(ptr as *mut c_void, info, id) };
    Some(any_ptr)
}

fn entities_holder_get_all_components<'r>(
    entities: &'r EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    mut handler: impl FnMut(AnyRef<'r>),
) {
    unsafe {
        let mut handler = |ptr: AnyPtr| handler(ptr.as_any_ref());
        let handler = FfiFnMutMut::new(&mut handler);
        entities.get_all_components_ptrs(entity, handler)
    }
}

fn entities_holder_get_all_components_mut<'r>(
    entities: &'r mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
    mut handler: impl FnMut(AnyMut<'r>),
) {
    unsafe {
        let mut handler = |ptr: AnyPtr| handler(ptr.as_any_mut());
        let handler = FfiFnMutMut::new(&mut handler);
        entities.get_all_components_ptrs(entity, handler)
    }
}

fn entities_holder_get_component_mut<'r, C: 'static>(
    entities: &'r mut EntitiesHolderUnsafeFfi,
    types: &TypesRegistryCache,
    entity: EntityId,
) -> Option<&'r mut C> {
    unsafe { entities_holder_get_component_ptr::<C>(entities, types, entity).map(|p| &mut *p) }
}

fn entities_holder_entities_count(entities: &EntitiesHolderUnsafeFfi) -> u64 {
    entities.entities_count()
}

fn entities_holder_contains_entity(entities: &EntitiesHolderUnsafeFfi, entity: EntityId) -> bool {
    entities.contains_entity(entity)
}

fn entities_holder_get_entity_by_idx_only(entities: &EntitiesHolderUnsafeFfi, idx: u64) -> Option<EntityId> {
    entities.get_entity_by_idx_only(idx)
}

fn entities_holder_create_entity(entities: &mut EntitiesHolderUnsafeFfi) -> EntityId {
    entities.create_entity()
}

fn entities_holder_destroy_entity(entities: &mut EntitiesHolderUnsafeFfi, entity: EntityId) -> bool {
    entities.destroy_entity(entity)
}

//

pub struct EntitiesHolderMut<'e> {
    entities: *mut EntitiesHolderUnsafeFfi,
    types: &'e TypesRegistryCache,
}

impl<'e> EntitiesHolderMut<'e> {
    pub unsafe fn new(entities: *mut EntitiesHolderUnsafeFfi, types: &'e TypesRegistryCache) -> Self {
        Self { entities, types }
    }

    pub fn types(&self) -> &'e TypesRegistryCache {
        self.types
    }

    pub fn query<'r, A: ArchetypeIteratorItem>(&'r self) -> EntitiesHolderQuery<'r, A::ReadOnlyItem<'r>>
    where
        'e: 'r,
    {
        unsafe { entities_holder_query::<A::ReadOnlyItem<'r>, ()>(&*self.entities, self.types) }
    }

    pub fn query_filtered<'r, A: ArchetypeIteratorItem, F: QueryFilter>(&'r self) -> EntitiesHolderQuery<'r, A::ReadOnlyItem<'r>, F>
    where
        'e: 'r,
    {
        unsafe { entities_holder_query::<A::ReadOnlyItem<'r>, F>(&*self.entities, self.types) }
    }

    pub fn query_mut<'r, A: ArchetypeIteratorItem>(&'r mut self) -> EntitiesHolderQuery<'r, A::Item<'r>>
    where
        'e: 'r,
    {
        unsafe { entities_holder_query::<A::Item<'r>, ()>(&mut *self.entities, self.types) }
    }

    pub fn query_filtered_mut<'r, A: ArchetypeIteratorItem, F: QueryFilter>(&'r mut self) -> EntitiesHolderQuery<'r, A::Item<'r>, F>
    where
        'e: 'r,
    {
        unsafe { entities_holder_query::<A::Item<'r>, F>(&mut *self.entities, self.types) }
    }

    pub fn entities_count(&self) -> u64 {
        unsafe { entities_holder_entities_count(&*self.entities) }
    }

    pub fn contains_entity(&self, entity: EntityId) -> bool {
        unsafe { entities_holder_contains_entity(&*self.entities, entity) }
    }

    pub fn get_entity_by_idx_only(&self, idx: u64) -> Option<EntityId> {
        unsafe { entities_holder_get_entity_by_idx_only(&*self.entities, idx) }
    }

    pub fn encode_entity_as_id(&self, entity: EntityId) -> u64 {
        if self.contains_entity(entity) {
            entity.version_index().index + 1
        } else {
            0
        }
    }

    pub fn decode_entity_from_id(&self, id: u64) -> EntityId {
        if id != 0
            && let Some(entity) = self.get_entity_by_idx_only(id)
        {
            entity
        } else {
            EntityId::EMPTY
        }
    }

    pub fn create_entity(&mut self) -> EntityId {
        unsafe { entities_holder_create_entity(&mut *self.entities) }
    }

    pub fn destroy_entity(&mut self, entity: EntityId) -> bool {
        unsafe { entities_holder_destroy_entity(&mut *self.entities, entity) }
    }

    pub fn add_component<C: 'static>(&mut self, entity: EntityId, component: C) -> Result<(), C> {
        // todo: maybe don't use result and have option instead or/and have different variants for add-or-replace, replace-if-present, trying-to-add, add-only.
        // todo: maybe an hashmap-like "entry" api
        unsafe { entities_holder_add_component(&mut *self.entities, self.types, entity, component) }
    }

    pub fn set_component<C: 'static>(&mut self, entity: EntityId, component: C) -> bool {
        unsafe { entities_holder_set_component(&mut *self.entities, self.types, entity, component) }
    }

    pub fn set_components<T: ComponentsTuple>(&mut self, entity: EntityId, components: T) -> bool {
        T::set_as_components_into_entities_holder(self.as_mut(), entity, components)
    }

    pub fn add_component_any(&mut self, entity: EntityId, component: AnyBox) -> Result<(), AnyBox> {
        unsafe { entities_holder_add_component_any(&mut *self.entities, self.types, entity, component) }
    }

    pub fn set_components_any(&mut self, entity: EntityId, components: FfiVec<AnyBox>) -> bool {
        unsafe { entities_holder_set_components_any(&mut *self.entities, self.types, entity, components) }
    }

    pub fn remove_component<C: 'static>(&mut self, entity: EntityId) -> Option<C> {
        unsafe { entities_holder_remove_component(&mut *self.entities, self.types, entity) }
    }

    pub fn remove_component_any(&mut self, entity: EntityId, id: AnyTypeId) -> Option<AnyBox> {
        unsafe { entities_holder_remove_component_any(&mut *self.entities, self.types, entity, id) }
    }

    pub fn get_component_ptr<C: 'static>(&self, entity: EntityId) -> Option<*mut C> {
        unsafe { entities_holder_get_component_ptr(&*self.entities, self.types, entity) }
    }

    pub fn get_component<'r, C: 'static>(&'r self, entity: EntityId) -> Option<&'r C>
    where
        'e: 'r,
    {
        unsafe { entities_holder_get_component(&*self.entities, self.types, entity) }
    }

    pub fn get_component_mut<'r, C: 'static>(&'r mut self, entity: EntityId) -> Option<&'r mut C>
    where
        'e: 'r,
    {
        unsafe { entities_holder_get_component_mut(&mut *self.entities, self.types, entity) }
    }

    pub fn get_component_any<'r>(&'r self, entity: EntityId, id: AnyTypeId) -> Option<AnyRef<'r>>
    where
        'e: 'r,
    {
        unsafe { entities_holder_get_component_any_ptr(&*self.entities, self.types, entity, id).map(|p| p.as_any_ref()) }
    }

    pub fn get_component_any_mut<'r>(&'r mut self, entity: EntityId, id: AnyTypeId) -> Option<AnyMut<'r>>
    where
        'e: 'r,
    {
        unsafe { entities_holder_get_component_any_ptr(&*self.entities, self.types, entity, id).map(|p| p.as_any_mut()) }
    }

    pub fn get_all_components<'r>(&'r self, entity: EntityId, handler: impl FnMut(AnyRef<'r>))
    where
        'e: 'r,
    {
        unsafe { entities_holder_get_all_components(&*self.entities, self.types, entity, handler) }
    }

    pub fn get_all_components_mut<'r>(&'r mut self, entity: EntityId, handler: impl FnMut(AnyMut<'r>))
    where
        'e: 'r,
    {
        unsafe { entities_holder_get_all_components_mut(&mut *self.entities, self.types, entity, handler) }
    }

    pub fn as_mut<'r>(&'r mut self) -> EntitiesHolderMut<'r>
    where
        'e: 'r,
    {
        EntitiesHolderMut {
            entities: self.entities,
            types: self.types,
        }
    }

    pub fn as_ref<'r>(&'r self) -> EntitiesHolderRef<'r>
    where
        'e: 'r,
    {
        EntitiesHolderRef {
            entities: self.entities,
            types: self.types,
        }
    }
}

unsafe impl<'e> Send for EntitiesHolderMut<'e> {}
unsafe impl<'e> Sync for EntitiesHolderMut<'e> {}

//

#[derive(Copy, Clone)]
pub struct EntitiesHolderRef<'e> {
    entities: *const EntitiesHolderUnsafeFfi,
    types: &'e TypesRegistryCache,
}

impl<'e> EntitiesHolderRef<'e> {
    pub unsafe fn new(entities: *const EntitiesHolderUnsafeFfi, types: &'e TypesRegistryCache) -> Self {
        Self { entities, types }
    }

    pub fn types(self) -> &'e TypesRegistryCache {
        self.types
    }

    pub unsafe fn unsafe_query<A: ArchetypeIteratorItem, F: QueryFilter>(self) -> EntitiesHolderQuery<'e, A, F> {
        unsafe { entities_holder_query::<A, F>(&*self.entities, self.types) }
    }

    pub fn query<A: ArchetypeIteratorItem>(self) -> EntitiesHolderQuery<'e, A::ReadOnlyItem<'e>> {
        unsafe { entities_holder_query::<A::ReadOnlyItem<'e>, ()>(&*self.entities, self.types) }
    }

    pub fn query_filtered<A: ArchetypeIteratorItem, F: QueryFilter>(self) -> EntitiesHolderQuery<'e, A::ReadOnlyItem<'e>, F> {
        unsafe { entities_holder_query::<A::ReadOnlyItem<'e>, F>(&*self.entities, self.types) }
    }

    pub fn entities_count(self) -> u64 {
        unsafe { entities_holder_entities_count(&*self.entities) }
    }

    pub fn contains_entity(self, entity: EntityId) -> bool {
        unsafe { entities_holder_contains_entity(&*self.entities, entity) }
    }

    pub fn get_entity_by_idx_only(&self, idx: u64) -> Option<EntityId> {
        unsafe { entities_holder_get_entity_by_idx_only(&*self.entities, idx) }
    }

    pub fn encode_entity_as_id(&self, entity: EntityId) -> u64 {
        if self.contains_entity(entity) {
            entity.version_index().index + 1
        } else {
            0
        }
    }

    pub fn decode_entity_from_id(&self, id: u64) -> EntityId {
        if id != 0
            && let Some(entity) = self.get_entity_by_idx_only(id)
        {
            entity
        } else {
            EntityId::EMPTY
        }
    }

    pub fn get_component_ptr<C: 'static>(self, entity: EntityId) -> Option<*mut C> {
        unsafe { entities_holder_get_component_ptr(&*self.entities, self.types, entity) }
    }

    pub fn get_component<C: 'static>(self, entity: EntityId) -> Option<&'e C> {
        unsafe { entities_holder_get_component(&*self.entities, self.types, entity) }
    }

    pub fn get_component_any(self, entity: EntityId, id: AnyTypeId) -> Option<AnyRef<'e>> {
        unsafe { entities_holder_get_component_any_ptr(&*self.entities, self.types, entity, id).map(|p| p.as_any_ref()) }
    }

    pub fn get_all_components<'r>(&'r self, entity: EntityId, handler: impl FnMut(AnyRef<'r>))
    where
        'e: 'r,
    {
        unsafe { entities_holder_get_all_components(&*self.entities, self.types, entity, handler) }
    }
}

unsafe impl<'e> Send for EntitiesHolderRef<'e> {}
unsafe impl<'e> Sync for EntitiesHolderRef<'e> {}

mod sealed {
    use super::ComponentsTuple;
    use super::{ComponentPtr, EntitiesHolderMut, EntityId};
    use aphyn_ffi::FfiSliceRef;
    use std::mem::MaybeUninit;

    pub trait Sealed {}

    macro_rules! components_tuple_impl {
        ($($t: ident),+) => {
            impl<$($t: 'static),+> Sealed for ($($t,)+) { }

            impl<$($t: 'static),+> ComponentsTuple for ($($t,)+) {
                fn set_as_components_into_entities_holder(ec: EntitiesHolderMut, entity: EntityId, components: Self) -> bool {
                    #[allow(non_snake_case)]
                    let ($(mut $t,)+) = {
                        let ($($t,)+) = components;
                        ($(MaybeUninit::new($t),)+)
                    };

                    let components_ptrs = [$(ComponentPtr { ptr: $t.as_mut_ptr() as *mut u8, id: ec.types.get_or_register::<$t>() },)+];

                    let is_set = unsafe { (*ec.entities).set_components(entity, FfiSliceRef::from_slice(&components_ptrs)) };

                    if !is_set {
                        $(unsafe { $t.assume_init_drop() };)+
                    }

                    is_set
                }
            }
        };
    }

    components_tuple_impl!(C0);
    components_tuple_impl!(C0, C1);
    components_tuple_impl!(C0, C1, C2);
    components_tuple_impl!(C0, C1, C2, C3);
    components_tuple_impl!(C0, C1, C2, C3, C4);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13, C14);
    components_tuple_impl!(C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13, C14, C15);
}

pub trait ComponentsTuple: sealed::Sealed {
    fn set_as_components_into_entities_holder(ec: EntitiesHolderMut, entity: EntityId, components: Self) -> bool;
}
