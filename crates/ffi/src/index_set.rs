use std::{borrow::Borrow, fmt::Debug, hash::Hash};

use crate::{FfiIndexMap, FfiIndexMapIntoKeysIter, FfiIndexMapKeysIter};

// todo: maybe not reuse the map (so we can iterate it like a vec or even extract a slice from it)
#[repr(C)]
pub struct FfiIndexSet<T> {
    values: FfiIndexMap<T, ()>,
}

impl<T> FfiIndexSet<T> {
    pub fn new() -> Self {
        Self {
            values: FfiIndexMap::new(),
        }
    }
}

impl<T: Hash + Eq> FfiIndexSet<T> {
    pub fn contains<U: ?Sized + Eq + Hash>(&self, key: &U) -> bool
        where T: Borrow<U>
    {
        self.values.contains_key(key)
    }
    pub fn insert(&mut self, key: T) -> bool {
        self.values.insert(key, ()).is_none()
    }
    pub fn remove_shift<Q: ?Sized + Eq + Hash>(&mut self, key: &Q) -> bool
        where T: Borrow<Q>
    {
        self.values.remove_shift(key).is_some()
    }
    pub fn remove_swap<Q: ?Sized + Eq + Hash>(&mut self, key: &Q) -> bool
        where T: Borrow<Q>
    {
        self.values.remove_swap(key).is_some()
    }
    pub fn len(&self) -> u64 {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn index_of<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> Option<u64>
        where T: Borrow<Q>
    {
        self.values.index_of(key)
    }
    pub fn clear(&mut self) {
        self.values.clear()
    }
}
impl<T> Default for FfiIndexSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Clone + Hash + Eq> Clone for FfiIndexSet<T> {
    fn clone(&self) -> Self {
        // todo: clone instead of reassembling and remove redudnant generic bounds
        Self {
            values: self.values.clone()
        }
    }
}

impl<T> FfiIndexSet<T> {
    pub fn iter<'a>(&'a self) -> FfiIndexMapKeysIter<'a, T, ()> {
        self.values.keys()
    }
}

impl<T> IntoIterator for FfiIndexSet<T> {
    type Item = T;
    type IntoIter = FfiIndexMapIntoKeysIter<T, ()>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_keys()
    }
}

//

impl<'a, T> IntoIterator for &'a FfiIndexSet<T> {
    type Item = &'a T;
    type IntoIter = FfiIndexMapKeysIter<'a, T, ()>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.keys()
    }
}

impl<T: Hash + Eq> FromIterator<T> for FfiIndexSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            values: iter.into_iter().map(|v| (v, ())).collect(),
        }
    }
}

impl<T: Debug> Debug for FfiIndexSet<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
