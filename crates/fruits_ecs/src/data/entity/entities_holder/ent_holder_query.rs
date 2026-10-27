use std::marker::PhantomData;

use crate::*;

pub struct EntitiesHolderQuery<'d, A: ArchetypeIteratorItem, F: QueryFilter = ()> {
    entities: &'d EntitiesHolderUnsafeFfi,
    types: &'d TypesRegistryCache,
    archetype_indices: Vec<u64>,
    type_cache: <<A as ArchetypeIteratorItem>::Item<'static> as ArchetypeIteratorItem>::TypeCache,
    type_cache_readonly: <<A as ArchetypeIteratorItem>::ReadOnlyItem<'static> as ArchetypeIteratorItem>::TypeCache,
    type_cache_filter: F::TypeCache,
    _phantom: (PhantomData<fn(A::Item<'static>) -> A::Item<'static>>, PhantomData<fn(F) -> F>),
}

impl<'d, A: ArchetypeIteratorItem, F: QueryFilter> EntitiesHolderQuery<'d, A, F> {
    /// # Safety
    ///
    /// No access sync - needs to be managed by caller.
    pub(crate) unsafe fn new(entities: &'d EntitiesHolderUnsafeFfi, types: &'d TypesRegistryCache) -> Self {
        let mut usage = ByTypeWorldPartDataUsageBuilder::new();

        A::fill_usage(&mut usage, types);

        let Some(mut components) = usage.build() else {
            panic!("fruits: Invalid entities query DataUsage: {}", std::any::type_name::<A>());
        };

        let entity_type_id = types.get_or_register::<EntityId>();
        if let Some(entity_id_idx) = components.iter().position(|x| x.type_id == entity_type_id) {
            components.remove(entity_id_idx as u64);
        }

        let archetypes = entities.archetypes();

        if components.is_empty() {
            let type_cache_filter = F::TypeCache::new(types);

            // Query is with entities only (should iterate all entities).
            return Self {
                archetype_indices: (0..archetypes.len())
                    .filter(|ai| F::matches(archetypes.by_id_ref(*ai).unwrap().layout(), type_cache_filter))
                    .collect::<Vec<_>>(),
                entities,
                types,
                type_cache: <<A as ArchetypeIteratorItem>::Item<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
                type_cache_readonly: <<A as ArchetypeIteratorItem>::ReadOnlyItem<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
                type_cache_filter,
                _phantom: Default::default(),
            };
        }

        let required_components = components.iter().filter(|e| e.details.is_required).map(|e| e.type_id);

        let mut archetypes_with_rarest_component = None;

        for component in required_components.clone() {
            let Some(archetypes_with_component) = archetypes.ids_by_component(component) else {
                let type_cache_filter = F::TypeCache::new(types);

                // Query is with some required component that no archetype has (should iterate none).
                return Self {
                    entities,
                    types,
                    archetype_indices: Vec::new(),
                    type_cache: <<A as ArchetypeIteratorItem>::Item<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
                    type_cache_readonly: <<A as ArchetypeIteratorItem>::ReadOnlyItem<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
                    type_cache_filter,
                    _phantom: Default::default(),
                };
            };

            let Some(archetypes_with_rarest_component) = &mut archetypes_with_rarest_component else {
                archetypes_with_rarest_component = Some((archetypes_with_component.len(), archetypes_with_component));
                continue;
            };

            if archetypes_with_component.len() < archetypes_with_rarest_component.0 {
                *archetypes_with_rarest_component = (archetypes_with_component.len(), archetypes_with_component);
            }
        }

        let Some((_, archetypes_with_rarest_component)) = archetypes_with_rarest_component else {
            let type_cache_filter = F::TypeCache::new(types);

            // Query has optional components only (should iterate all entities).
            return Self {
                archetype_indices: (0..archetypes.len())
                    .filter(|ai| F::matches(archetypes.by_id_ref(*ai).unwrap().layout(), type_cache_filter))
                    .collect::<Vec<_>>(),
                entities,
                types,
                type_cache: <<A as ArchetypeIteratorItem>::Item<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
                type_cache_readonly: <<A as ArchetypeIteratorItem>::ReadOnlyItem<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
                type_cache_filter,
                _phantom: Default::default(),
            };
        };

        let mut suitable_archetypes = Vec::new();

        let type_cache_filter = F::TypeCache::new(types);

        for archetype_id in archetypes_with_rarest_component.iter() {
            let archetype = archetypes.by_id_ref(*archetype_id).unwrap();

            let contains_all_components = required_components.clone().all(|c| archetype.contains_component_type(c));

            // Archetypes that are missing any required component are skipped.
            if contains_all_components && F::matches(archetype.layout(), type_cache_filter) {
                suitable_archetypes.push(*archetype_id);
            }
        }

        Self {
            entities,
            types,
            archetype_indices: suitable_archetypes,
            type_cache: <<A as ArchetypeIteratorItem>::Item<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
            type_cache_readonly: <<A as ArchetypeIteratorItem>::ReadOnlyItem<'static> as ArchetypeIteratorItem>::TypeCache::new(types),
            type_cache_filter,
            _phantom: Default::default(),
        }
    }

    pub fn iter<'r>(&'r self) -> QueryIter<'r, A>
    where
        'd: 'r,
    {
        QueryIter::<A>::new(self.archetypes_iter(), self.type_cache_readonly)
    }

    pub fn iter_mut<'r>(&'r mut self) -> QueryIterMut<'r, A>
    where
        'd: 'r,
    {
        QueryIterMut::<A>::new(self.archetypes_iter(), self.type_cache)
    }

    pub fn len(&self) -> u64 {
        unsafe { self.archetypes_iter().map(|a| a.entities_count()).sum() }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { !self.archetypes_iter().any(|a| a.entities_count() > 0) }
    }

    pub fn get<'r>(&'r self, entity: EntityId) -> Option<<A::ReadOnlyItem<'static> as ArchetypeIteratorItem>::Item<'r>>
    where
        'd: 'r,
    {
        unsafe {
            let location = self.entities.entities_meta().get(entity)?;

            let archetype = self.entities.archetypes().by_id_ref(location.archetype_id)?;

            if !F::matches(archetype.layout(), self.type_cache_filter) {
                return None;
            }

            if !self.archetype_indices.contains(&location.archetype_id) {
                return None;
            }

            let chunk_idx = archetype.layout().chunk_index(location.entity_archetype_index);

            let chunk_ptr = archetype.raw_archetype().get_chunk(chunk_idx);

            let iter_item = ArchetypeUnsafeFfiIteratorItem {
                chunk_ptr,
                chunk_entity_idx: archetype.layout().entity_in_chunk_index(location.entity_archetype_index),
            };

            let mut iter_state =
                <A::ReadOnlyItem<'static> as ArchetypeIteratorItem>::prepare_iter_state(archetype.layout(), self.type_cache_readonly);
            let item = <A::ReadOnlyItem<'static> as ArchetypeIteratorItem>::next(&iter_item, &mut iter_state);

            Some(item)
        }
    }

    pub fn get_mut<'r>(&'r mut self, entity: EntityId) -> Option<<A::Item<'static> as ArchetypeIteratorItem>::Item<'r>>
    where
        'd: 'r,
    {
        unsafe {
            let location = self.entities.entities_meta().get(entity)?;

            let archetype = self.entities.archetypes().by_id_ref(location.archetype_id)?;

            if !F::matches(archetype.layout(), self.type_cache_filter) {
                return None;
            }

            if !self.archetype_indices.contains(&location.archetype_id) {
                return None;
            }

            let chunk_idx = archetype.layout().chunk_index(location.entity_archetype_index);

            let chunk_ptr = archetype.raw_archetype().get_chunk(chunk_idx);

            let iter_item = ArchetypeUnsafeFfiIteratorItem {
                chunk_ptr,
                chunk_entity_idx: archetype.layout().entity_in_chunk_index(location.entity_archetype_index),
            };

            let mut iter_state = <A::Item<'static> as ArchetypeIteratorItem>::prepare_iter_state(archetype.layout(), self.type_cache);
            let item = <A::Item<'static> as ArchetypeIteratorItem>::next(&iter_item, &mut iter_state);

            Some(item)
        }
    }

    fn archetypes_iter<'a>(&'a self) -> ArchetypesIter<'a> {
        unsafe { ArchetypesIter::new(self.entities.archetypes(), self.types, self.archetype_indices.iter()) }
    }
}

struct ArchetypesIter<'a> {
    archetypes: &'a ArchetypesHolderFfi,
    types: &'a TypesRegistryCache,
    iter: std::slice::Iter<'a, u64>,
}

impl<'a> ArchetypesIter<'a> {
    /// Only for readonly archetypes access
    pub unsafe fn new(archetypes: &'a ArchetypesHolderFfi, types: &'a TypesRegistryCache, iter: std::slice::Iter<'a, u64>) -> Self {
        Self { archetypes, types, iter }
    }
}

impl<'a> Iterator for ArchetypesIter<'a> {
    type Item = ArchetypeUnsafeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(ArchetypeUnsafeRef::<'a>::new(
            (&raw const *self.archetypes.by_id_ref(*self.iter.next()?).unwrap()) as *mut ArchetypeUnsafeFfi,
            self.types,
        ))
    }
}

pub struct QueryIter<'a, A: ArchetypeIteratorItem> {
    iter: ArchetypesIter<'a>,
    iter2: Option<ArchetypeIterator<'a, A::ReadOnlyItem<'static>>>,
    type_cache: <<A as ArchetypeIteratorItem>::ReadOnlyItem<'static> as ArchetypeIteratorItem>::TypeCache,
}

impl<'a, A: ArchetypeIteratorItem> QueryIter<'a, A> {
    fn new(iter: ArchetypesIter<'a>, type_cache: <<A as ArchetypeIteratorItem>::ReadOnlyItem<'static> as ArchetypeIteratorItem>::TypeCache) -> Self {
        Self {
            iter,
            iter2: None,
            type_cache,
        }
    }
}
impl<'a, A: ArchetypeIteratorItem> Iterator for QueryIter<'a, A> {
    type Item = <A::ReadOnlyItem<'static> as ArchetypeIteratorItem>::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // if let item @ Some(_) = self.iter2.as_mut().map(Iterator::next).flatten() {}
            if let Some(iter2) = &mut self.iter2
                && let item @ Some(_) = iter2.next()
            {
                return item;
            } else if let Some(iter_item) = self.iter.next() {
                self.iter2 = Some(unsafe { iter_item.into_iter::<A::ReadOnlyItem<'static>>(self.type_cache) });
            } else {
                return None;
            }
        }
    }
}

pub struct QueryIterMut<'a, A: ArchetypeIteratorItem> {
    iter: ArchetypesIter<'a>,
    iter2: Option<ArchetypeIterator<'a, A::Item<'static>>>,
    type_cache: <<A as ArchetypeIteratorItem>::Item<'static> as ArchetypeIteratorItem>::TypeCache,
}

impl<'a, A: ArchetypeIteratorItem> QueryIterMut<'a, A> {
    fn new(iter: ArchetypesIter<'a>, type_cache: <<A as ArchetypeIteratorItem>::Item<'static> as ArchetypeIteratorItem>::TypeCache) -> Self {
        Self {
            iter,
            iter2: None,
            type_cache,
        }
    }
}
impl<'a, A: ArchetypeIteratorItem> Iterator for QueryIterMut<'a, A> {
    type Item = <A::Item<'static> as ArchetypeIteratorItem>::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // if let item @ Some(_) = self.iter2.as_mut().map(Iterator::next).flatten() {}
            if let Some(iter2) = &mut self.iter2
                && let item @ Some(_) = iter2.next()
            {
                return item;
            } else if let Some(iter_item) = self.iter.next() {
                self.iter2 = Some(unsafe { iter_item.into_iter::<A::Item<'static>>(self.type_cache) });
            } else {
                return None;
            }
        }
    }
}

// todo

// impl<'d, A: ArchetypeIteratorItem, F: QueryFilter> IntoIterator for &EntitiesComponentsQuery<'d, A, F> {
//     type Item = <QueryIter<'d, A> as Iterator>::Item;

//     type IntoIter = QueryIter<'d, A>;

//     fn into_iter(self) -> Self::IntoIter {
//         self.iter()
//     }
// }

pub trait QueryFilter: 'static {
    type TypeCache: TypeCache;

    fn get_type_cache(types: &TypesRegistryCache) -> Self::TypeCache;
    fn matches(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> bool;
}

pub struct WithoutFilter<T: Component> {
    _phantom: PhantomData<fn(T) -> T>,
}

impl<C: 'static + Component> QueryFilter for WithoutFilter<C> {
    type TypeCache = RegistrySpecificTypeId<C>;

    fn get_type_cache(types: &TypesRegistryCache) -> Self::TypeCache {
        Self::TypeCache::new(types)
    }
    fn matches(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> bool {
        layout.get_component(type_cache.id()).is_none()
    }
}

pub struct WithFilter<T: Component> {
    _phantom: PhantomData<fn(T) -> T>,
}

impl<C: 'static + Component> QueryFilter for WithFilter<C> {
    type TypeCache = RegistrySpecificTypeId<C>;

    fn get_type_cache(types: &TypesRegistryCache) -> Self::TypeCache {
        Self::TypeCache::new(types)
    }
    fn matches(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> bool {
        layout.get_component(type_cache.id()).is_some()
    }
}

macro_rules! query_filter_tuple {
    ($($q: ident),*) => {
        impl<
        $($q: QueryFilter),*
        > QueryFilter for (
        $($q,)*
        ) {
            type TypeCache = (
                $($q::TypeCache,)*
            );

            fn get_type_cache(types: &TypesRegistryCache) -> Self::TypeCache {
                Self::TypeCache::new(types)
            }
            fn matches(_layout: &ArchetypeLayout, _types_cache: Self::TypeCache) -> bool {
                #[allow(non_snake_case)]
                let (
                    $($q,)*
                ) = _types_cache;

                true
                $(&& $q::matches(_layout, $q))*
            }
        }
    };
}

query_filter_tuple!();
query_filter_tuple!(Q1);
query_filter_tuple!(Q1, Q2);
query_filter_tuple!(Q1, Q2, Q3);
query_filter_tuple!(Q1, Q2, Q3, Q4);
query_filter_tuple!(Q1, Q2, Q3, Q4, Q5);
query_filter_tuple!(Q1, Q2, Q3, Q4, Q5, Q6);
query_filter_tuple!(Q1, Q2, Q3, Q4, Q5, Q6, Q7);

// todo: to sealed
pub trait OrFilterArg: 'static {
    type TypeCache: TypeCache;

    fn get_type_cache(types: &TypesRegistryCache) -> Self::TypeCache;
    fn or(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> bool;
}

pub struct OrFilter<A: OrFilterArg> {
    _phantom: PhantomData<fn(A) -> A>,
}

impl<A: OrFilterArg> QueryFilter for OrFilter<A> {
    type TypeCache = A::TypeCache;

    fn get_type_cache(types: &TypesRegistryCache) -> Self::TypeCache {
        A::get_type_cache(types)
    }
    fn matches(layout: &ArchetypeLayout, type_cache: Self::TypeCache) -> bool {
        A::or(layout, type_cache)
    }
}

macro_rules! query_filter_or {
    ($($q: ident),*) => {
        impl<
        $($q: QueryFilter),*
        > OrFilterArg for (
        $($q,)*
        ) {
            type TypeCache = (
                $($q::TypeCache,)*
            );

            fn get_type_cache(_types: &TypesRegistryCache) -> Self::TypeCache {
                (
                    $($q::TypeCache::new(_types),)*
                )
            }

            fn or(_layout: &ArchetypeLayout, _types_cache: Self::TypeCache) -> bool {
                #[allow(non_snake_case)]
                let (
                    $($q,)*
                ) = _types_cache;

                false
                $(|| $q::matches(_layout, $q))*
            }
        }
    };
}

query_filter_or!();
query_filter_or!(Q1);
query_filter_or!(Q1, Q2);
query_filter_or!(Q1, Q2, Q3);
query_filter_or!(Q1, Q2, Q3, Q4);
query_filter_or!(Q1, Q2, Q3, Q4, Q5);
query_filter_or!(Q1, Q2, Q3, Q4, Q5, Q6);
query_filter_or!(Q1, Q2, Q3, Q4, Q5, Q6, Q7);
