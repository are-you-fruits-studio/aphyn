use std::{any::TypeId, marker::PhantomData, mem::MaybeUninit};

use crate::*;

pub trait Component: 'static + Send + Sync {}

pub unsafe trait ArchetypeIteratorItem {
    type Item<'w>: 'w + ArchetypeIteratorItem;
    type ReadOnlyItem<'w>: 'w + ArchetypeIteratorItem;
    type IterState;
    type TypeCache: TypeCache;

    unsafe fn prepare_iter_state(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> Self::IterState;
    unsafe fn next<'w>(item: &ArchetypeUnsafeFfiIteratorItem, iter_state: &mut Self::IterState) -> Self::Item<'w>;
    fn fill_usage(usage: &mut ByTypeWorldPartDataUsageBuilder, types: &TypesRegistryCache);
}

unsafe impl<C: Component> ArchetypeIteratorItem for &C {
    type Item<'w> = &'w C;
    type ReadOnlyItem<'w> = &'w C;
    type IterState = u64;
    type TypeCache = RegistrySpecificTypeId<C>;

    unsafe fn prepare_iter_state(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> Self::IterState {
        layout
            .get_component(type_cache.id())
            .map(|i| i.chunk_offset)
            .unwrap_or(0)
    }

    unsafe fn next<'w>(item: &ArchetypeUnsafeFfiIteratorItem, iter_state: &mut Self::IterState) -> Self::Item<'w> {
        unsafe {
            &*(item
                .chunk_ptr
                .add(*iter_state as usize + item.chunk_entity_idx as usize * std::mem::size_of::<C>()) as *mut C)
        }
    }

    fn fill_usage(usage: &mut ByTypeWorldPartDataUsageBuilder, types: &TypesRegistryCache) {
        usage.add_type(DataUsageEntry {
            type_id: types.get_or_register::<C>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: true,
            },
        });
    }
}

unsafe impl<C: Component> ArchetypeIteratorItem for &mut C {
    type Item<'w> = &'w mut C;
    type ReadOnlyItem<'w> = &'w C;
    type IterState = u64;
    type TypeCache = RegistrySpecificTypeId<C>;

    unsafe fn prepare_iter_state(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> Self::IterState {
        layout
            .get_component(type_cache.id())
            .map(|i| i.chunk_offset)
            .unwrap_or(0)
    }

    unsafe fn next<'w>(item: &ArchetypeUnsafeFfiIteratorItem, iter_state: &mut Self::IterState) -> Self::Item<'w> {
        unsafe {
            &mut *(item
                .chunk_ptr
                .add(*iter_state as usize + item.chunk_entity_idx as usize * std::mem::size_of::<C>()) as *mut C)
        }
    }

    fn fill_usage(usage: &mut ByTypeWorldPartDataUsageBuilder, types: &TypesRegistryCache) {
        usage.add_type(DataUsageEntry {
            type_id: types.get_or_register::<C>(),
            details: DataUsageDetails {
                is_mut: true,
                is_required: true,
            },
        });
    }
}

unsafe impl<C: Component> ArchetypeIteratorItem for Option<&C> {
    type Item<'w> = Option<&'w C>;
    type ReadOnlyItem<'w> = Option<&'w C>;
    type IterState = Option<u64>;
    type TypeCache = RegistrySpecificTypeId<C>;

    unsafe fn prepare_iter_state(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> Self::IterState {
        layout.get_component(type_cache.id()).map(|t| t.chunk_offset)
    }

    unsafe fn next<'w>(item: &ArchetypeUnsafeFfiIteratorItem, iter_state: &mut Self::IterState) -> Self::Item<'w> {
        Some(unsafe {
            &*(item
                .chunk_ptr
                .add((*iter_state)? as usize + item.chunk_entity_idx as usize * std::mem::size_of::<C>()) as *mut C)
        })
    }

    fn fill_usage(usage: &mut ByTypeWorldPartDataUsageBuilder, types: &TypesRegistryCache) {
        usage.add_type(DataUsageEntry {
            type_id: types.get_or_register::<C>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: false,
            },
        });
    }
}

unsafe impl<C: Component> ArchetypeIteratorItem for Option<&mut C> {
    type Item<'w> = Option<&'w mut C>;
    type ReadOnlyItem<'w> = Option<&'w C>;
    type IterState = Option<u64>;
    type TypeCache = RegistrySpecificTypeId<C>;

    unsafe fn prepare_iter_state(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> Self::IterState {
        layout.get_component(type_cache.id()).map(|i| i.chunk_offset)
    }
    unsafe fn next<'w>(item: &ArchetypeUnsafeFfiIteratorItem, iter_state: &mut Self::IterState) -> Self::Item<'w> {
        Some(unsafe {
            &mut *(item
                .chunk_ptr
                .add((*iter_state)? as usize + item.chunk_entity_idx as usize * std::mem::size_of::<C>())
                as *mut C)
        })
    }

    fn fill_usage(usage: &mut ByTypeWorldPartDataUsageBuilder, types: &TypesRegistryCache) {
        usage.add_type(DataUsageEntry {
            type_id: types.get_or_register::<C>(),
            details: DataUsageDetails {
                is_mut: true,
                is_required: false,
            },
        });
    }
}

unsafe impl ArchetypeIteratorItem for EntityId {
    type Item<'w> = EntityId;
    type ReadOnlyItem<'w> = EntityId;
    type IterState = ();
    type TypeCache = ();

    unsafe fn prepare_iter_state(_layout: &ArchetypeLayout, _type_cache: Self::TypeCache) -> Self::IterState {}

    unsafe fn next<'w>(item: &ArchetypeUnsafeFfiIteratorItem, _iter_state: &mut Self::IterState) -> Self::Item<'w> {
        unsafe {
            (item
                .chunk_ptr
                .add(item.chunk_entity_idx as usize * std::mem::size_of::<EntityId>()) as *mut EntityId)
                .read()
        }
    }

    fn fill_usage(usage: &mut ByTypeWorldPartDataUsageBuilder, types: &TypesRegistryCache) {
        usage.add_type(DataUsageEntry {
            type_id: types.get_or_register::<EntityId>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: true,
            },
        });
    }
}

macro_rules! archetype_iterator_item_impl {
    ($($P: ident),+) => {
        #[allow(unused_parens)]
        unsafe impl<$($P),+> ArchetypeIteratorItem for ($($P),+)
        where
            $($P: ArchetypeIteratorItem),+
        {
            type Item<'w> = (
                $($P::Item<'w>),+
            );
            type ReadOnlyItem<'w> = (
                $($P::ReadOnlyItem<'w>),+
            );
            type IterState = (
                $($P::IterState),+
            );
            type TypeCache = (
                $($P::TypeCache,)+
            );

            unsafe fn prepare_iter_state(_layout: &ArchetypeLayout, _type_cache: Self::TypeCache) -> Self::IterState {
                unsafe {
                    #[allow(non_snake_case)]
                    let (
                        $($P),+
                    ) = _type_cache;

                    (
                        $($P::prepare_iter_state(_layout, $P)),+
                    )
                }
            }
            unsafe fn next<'w>(item: &ArchetypeUnsafeFfiIteratorItem, iter_state: &mut Self::IterState) -> Self::Item<'w> {
                #[allow(non_snake_case)]
                let (
                    $($P),+
                ) = iter_state;

                unsafe {
                    (
                        $($P::next(item, $P)),+
                    )
                }
            }

            fn fill_usage(usage: &mut ByTypeWorldPartDataUsageBuilder, types: &TypesRegistryCache) {
                $($P::fill_usage(usage, types));+;
            }
        }
    };
}

archetype_iterator_item_impl!(P0, P1);
archetype_iterator_item_impl!(P0, P1, P2);
archetype_iterator_item_impl!(P0, P1, P2, P3);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13);
archetype_iterator_item_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14);

pub struct ArchetypeIterator<'a, A: ArchetypeIteratorItem> {
    iter: ArchetypeUnsafeFfiIterator<'a>,
    iter_state: A::IterState,
    _phantom: PhantomData<&'a mut A>,
}

impl<'a, A: ArchetypeIteratorItem> ArchetypeIterator<'a, A> {
    pub unsafe fn new(archetype: ArchetypeUnsafeRef<'a>, type_cache: A::TypeCache) -> Self {
        unsafe {
            let archetype_ffi = &*archetype.archetype;

            Self {
                iter_state: A::prepare_iter_state(archetype_ffi.layout(), type_cache),
                iter: archetype_ffi.iter(),
                _phantom: Default::default(),
            }
        }
    }
}

impl<'a, A: ArchetypeIteratorItem> Iterator for ArchetypeIterator<'a, A> {
    type Item = A::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(item) = self.iter.next() else {
            return None;
        };

        Some(unsafe { A::next(&item, &mut self.iter_state) })
    }
}

//

#[derive(Clone)]
pub struct ArchetypeUnsafeRef<'a> {
    archetype: *mut ArchetypeUnsafeFfi,
    types: &'a TypesRegistryCache,
}

impl<'a> ArchetypeUnsafeRef<'a> {
    pub fn new(archetype: *mut ArchetypeUnsafeFfi, types: &'a TypesRegistryCache) -> Self {
        Self { archetype, types }
    }

    pub unsafe fn contains_component_type(&self, type_id: &TypeId) -> bool {
        unsafe {
            let Some(type_id) = self.types.get_with_type_id(type_id) else {
                return false;
            };

            (&*self.archetype).contains_component_type(type_id)
        }
    }

    pub unsafe fn into_iter<A: ArchetypeIteratorItem>(self, type_cache: A::TypeCache) -> ArchetypeIterator<'a, A> {
        unsafe { ArchetypeIterator::new(self, type_cache) }
    }

    pub unsafe fn entities_count(&self) -> u64 {
        unsafe { (&*self.archetype).entities_count() }
    }

    pub unsafe fn get_entity(&self, entity_index: u64) -> Option<EntityId> {
        unsafe { (&*self.archetype).get_entity(entity_index) }
    }

    pub unsafe fn get_component_ptr<C: Component>(&self, entity_index: u64) -> Option<*mut C> {
        unsafe {
            (&*self.archetype)
                .get_component_ptr(entity_index, self.types.get_or_register::<C>())
                .map(|p| p as *mut C)
        }
    }

    pub unsafe fn create_entity(&mut self, entity: EntityId) {
        unsafe { (&mut *self.archetype).create_entity(entity) }
    }

    /// Returns the last entity before the destroy.
    pub unsafe fn destroy_entity(&mut self, entity_index: u64) -> Option<EntityId> {
        unsafe { (&mut *self.archetype).destroy_entity(entity_index) }
    }

    /// Returns the last entity from src archetype before the movement.
    pub unsafe fn add_component<C: Component>(
        src: &mut Self,
        dst: &mut Self,
        src_entity_index: u64,
        component: C,
    ) -> Result<EntityId, C> {
        unsafe {
            let mut component = MaybeUninit::new(component);

            if let Some(entity) = ArchetypeUnsafeFfi::add_component(
                &mut *src.archetype,
                &mut *dst.archetype,
                src_entity_index,
                component.as_mut_ptr() as *mut u8,
                src.types.get_or_register::<C>(),
            ) {
                Ok(entity)
            } else {
                Err(component.assume_init())
            }
        }
    }

    /// Returns the last entity from src archetype before the movement.
    pub unsafe fn remove_component<C: Component>(src: &mut Self, dst: &mut Self, src_entity_index: u64) -> Option<(EntityId, C)> {
        unsafe {
            let mut component = Option::<C>::None;

            if let Some(entity) = ArchetypeUnsafeFfi::remove_component(
                &mut *src.archetype,
                &mut *dst.archetype,
                src_entity_index,
                |ptr, _info| component = Some((ptr as *const C).read()),
                src.types.get_or_register::<C>(),
            ) {
                Some((entity, component.unwrap()))
            } else {
                None
            }
        }
    }

    pub unsafe fn ffi(&self) -> *mut ArchetypeUnsafeFfi {
        self.archetype
    }
}
