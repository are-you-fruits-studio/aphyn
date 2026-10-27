use std::{borrow::Borrow, ffi::c_void, fmt::{self, Debug}, hash::{BuildHasher, Hash, Hasher}};

use crate::{FfiDroppable, FfiFnRef, FfiOption, FfiSliceRef, FfiVec};

#[repr(C)]
struct VirtualHasherMutVtable {
    fn_finish: unsafe extern "C-unwind" fn(this: *const c_void) -> u64,
    fn_write: unsafe extern "C-unwind" fn(this: *mut c_void, bytes: FfiSliceRef<u8>),
}
#[repr(C)]
struct VirtualHasherMut {
    value: *mut c_void,
    vtable: &'static VirtualHasherMutVtable,
}
impl VirtualHasherMut {
    pub fn new<H: std::hash::Hasher>(hasher: &mut H) -> Self {
        unsafe extern "C-unwind" fn ffi_finish<H: std::hash::Hasher>(this: *const c_void) -> u64 {
            unsafe {
                let this = &*(this as *const H);

                this.finish()
            }
        }
        unsafe extern "C-unwind" fn ffi_write<H: std::hash::Hasher>(this: *mut c_void, bytes: FfiSliceRef<u8>) {
            unsafe {
                let this = &mut *(this as *mut H);

                this.write(bytes.into_slice());
            }
        }

        Self {
            value: hasher as *mut H as *mut c_void,
            vtable: &VirtualHasherMutVtable {
                fn_finish: ffi_finish::<H>,
                fn_write: ffi_write::<H>,
            }
        }
    }
}
impl std::hash::Hasher for VirtualHasherMut {
    fn finish(&self) -> u64 {
        unsafe {
            (self.vtable.fn_finish)(self.value)
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        unsafe {
            (self.vtable.fn_write)(self.value, FfiSliceRef::from_slice(bytes))
        }
    }
}

#[repr(C)]
struct FfiHashState {
    state: FfiDroppable,
    hash_fn: unsafe extern "C-unwind" fn(this: *const c_void, hash: FfiFnRef<VirtualHasherMut, ()>) -> u64,
}
impl FfiHashState {
    pub fn new() -> Self {
        unsafe extern "C-unwind" fn ffi_hash(this: *const c_void, hash: FfiFnRef<VirtualHasherMut, ()>) -> u64 {
            unsafe {
                let state = &*(this as *const std::hash::RandomState);

                let mut hasher = state.build_hasher();
                {
                    let hasher = VirtualHasherMut::new(&mut hasher);
                    hash.execute(hasher);
                }
                hasher.finish()
            }
        }

        Self {
            state: FfiDroppable::new(std::hash::RandomState::new()),
            hash_fn: ffi_hash,
        }
    }

    pub fn hash<H: ?Sized + std::hash::Hash>(&self, key: &H) -> u64 {
        unsafe {
            let hash = |mut hasher: VirtualHasherMut| key.hash(&mut hasher);
            let hash = FfiFnRef::new(&hash);
            (self.hash_fn)(self.state.get(), hash)
        }
    }
}

#[repr(C)]
struct FfiHashTableVtable {
    ffi_insert_unique: unsafe extern "C-unwind" fn(this: *mut c_void, hash: u64, value: u64, hasher: FfiFnRef<u64, u64>),
    ffi_remove: unsafe extern "C-unwind" fn(this: *mut c_void, hash: u64, key_eq: FfiFnRef<u64, bool>) -> FfiOption<u64>,
    ffi_get: unsafe extern "C-unwind" fn(this: *const c_void, hash: u64, key_eq: FfiFnRef<u64, bool>) -> FfiOption<u64>,
    ffi_set: unsafe extern "C-unwind" fn(this: *mut c_void, hash: u64, key_eq: FfiFnRef<u64, bool>, value: u64),
    ffi_clear: unsafe extern "C-unwind" fn(this: *mut c_void),
}

#[repr(C)]
struct FfiHashTable {
    table: FfiDroppable,
    vtable: &'static FfiHashTableVtable,
}

impl FfiHashTable {
    pub fn new() -> Self {
        unsafe extern "C-unwind" fn ffi_insert_unique(this: *mut c_void, hash: u64, value: u64, hasher: FfiFnRef<u64, u64>) {
            unsafe {
                let this = &mut *(this as *mut hashbrown::hash_table::HashTable<u64>);

                this.insert_unique(hash, value, |x| hasher.execute(*x));
            }
        }
        unsafe extern "C-unwind" fn ffi_remove(this: *mut c_void, hash: u64, key_eq: FfiFnRef<u64, bool>) -> FfiOption<u64> {
            unsafe {
                let this = &mut *(this as *mut hashbrown::hash_table::HashTable<u64>);
               
                let Ok(entry) = this.find_entry(hash, |i| key_eq.execute(*i)) else {
                    return None.into();
                };
           
                Some(entry.remove().0).into()
            }

        }
        unsafe extern "C-unwind" fn ffi_get(this: *const c_void, hash: u64, key_eq: FfiFnRef<u64, bool>) -> FfiOption<u64> {
            unsafe {
                let this = &*(this as *const hashbrown::hash_table::HashTable<u64>);
               
                this.find(hash, |i| key_eq.execute(*i)).copied().into()
            }
        }
        unsafe extern "C-unwind" fn ffi_set(this: *mut c_void, hash: u64, key_eq: FfiFnRef<u64, bool>, value: u64) {
            unsafe {
                let this = &mut *(this as *mut hashbrown::hash_table::HashTable<u64>);

                if let Ok(mut entry) = this.find_entry(hash, |i| key_eq.execute(*i)) {
                    *entry.get_mut() = value;
                }
            }
        }
        unsafe extern "C-unwind" fn ffi_clear(this: *mut c_void) {
            unsafe {
                let this = &mut *(this as *mut hashbrown::hash_table::HashTable<u64>);

                this.clear();
            }
        }

        Self {
            table: FfiDroppable::new(hashbrown::hash_table::HashTable::<u64>::new()),
            vtable: &FfiHashTableVtable {
                ffi_get: ffi_get,
                ffi_insert_unique: ffi_insert_unique,
                ffi_remove: ffi_remove,
                ffi_set: ffi_set,
                ffi_clear: ffi_clear,
            }
        }
    }
   
    pub fn insert_unique<K, V>(&mut self, hash: u64, value: u64, values: &FfiVec<FfiIndexMapElement<K, V>>) {
        unsafe {
            let hasher = |x: u64| values[x].hash;
            let hasher = FfiFnRef::new(&hasher);
            (self.vtable.ffi_insert_unique)(self.table.get(), hash, value, hasher)
        }
    }
   
    pub fn remove<K: Borrow<Q>, V, Q: ?Sized + Eq>(&mut self, hash: u64, key: &Q, values: &FfiVec<FfiIndexMapElement<K, V>>) -> Option<u64> {
        unsafe {
            let key_eq = |i: u64| key == values[i].key.borrow();
            let key_eq = FfiFnRef::new(&key_eq);
            (self.vtable.ffi_remove)(self.table.get(), hash, key_eq).into_option()
        }
    }
   
    pub fn get<K: Borrow<Q>, V, Q: ?Sized + Eq>(&self, hash: u64, key: &Q, values: &FfiVec<FfiIndexMapElement<K, V>>) -> Option<u64> {
        unsafe {
            let key_eq = |i: u64| key == values[i].key.borrow();
            let key_eq = FfiFnRef::new(&key_eq);
            (self.vtable.ffi_get)(self.table.get(), hash, key_eq).into_option()
        }
    }
   
    pub fn set<K: Borrow<Q>, V, Q: ?Sized + Eq>(&mut self, hash: u64, key: &Q, value: u64, values: &FfiVec<FfiIndexMapElement<K, V>>) {
        unsafe {
            let key_eq = |i: u64| key == values[i].key.borrow();
            let key_eq = FfiFnRef::new(&key_eq);
            (self.vtable.ffi_set)(self.table.get(), hash, key_eq, value)
        }
    }
   
    pub fn clear(&mut self) {
        unsafe {
            (self.vtable.ffi_clear)(self.table.get())
        }
    }
}

#[repr(C)]
struct FfiIndexMapElement<K, V> {
    key: K,
    value: V,
    hash: u64,
}

#[repr(C)]
pub struct FfiIndexMap<K, V> {
    indices: FfiHashTable,
    values: FfiVec<FfiIndexMapElement<K, V>>,
    state: FfiHashState,
}

impl<K, V> FfiIndexMap<K, V> {
    pub fn new() -> Self {
        Self {
            indices: FfiHashTable::new(),
            values: FfiVec::new(),
            state: FfiHashState::new(),
        }
    }
}

impl<K, V> FfiIndexMap<K, V> {
    pub fn contains_key<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> bool
        where K: Borrow<Q>
    {
        self.index_of(key).is_some()
    }
    pub fn get<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> Option<&V>
        where K: Borrow<Q>
    {
        self.index_of(key).map(|i| &self.values[i].value)
    }
    pub fn get_by_idx(&self, idx: u64) -> Option<(&K, &V)> {
        self.values.get(idx).map(|e| (&e.key, &e.value))
    }
    pub fn get_by_idx_mut(&mut self, idx: u64) -> Option<(&K, &mut V)> {
        self.values.get_mut(idx).map(|e| (&e.key, &mut e.value))
    }
    pub fn get_mut<Q: ?Sized + Eq + Hash>(&mut self, key: &Q) -> Option<&mut V>
        where K: Borrow<Q>
    {
        self.index_of(key).map(|i| &mut self.values[i].value)
    }
    pub fn entry<'a>(&'a mut self, key: K) -> FfiIndexMapEntry<'a, K, V>
        where K: Eq + Hash
    {
        if let Some(idx) = self.index_of(&key) {
            FfiIndexMapEntry::Occupied(FfiIndexMapOccupiedEntry { map: self, idx })
        } else {
            FfiIndexMapEntry::Vacant(FfiIndexMapVacantEntry { map: self, key })
        }
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V>
        where K: Hash + Eq
    {
        let hash = self.state.hash(&key);

        if let Some(idx) = self.index_of_prehashed(&key, hash) {
            return Some(std::mem::replace(&mut self.values[idx].value, value));
        }

        let idx = self.values.len();
        self.values.push(FfiIndexMapElement {
            key,
            value,
            hash,
        });
        self.indices.insert_unique(hash, idx, &self.values);

        None
    }
    fn insert_and_idx(&mut self, key: K, value: V) -> (Option<V>, u64)
        where K: Hash + Eq
    {
        let hash = self.state.hash(&key);

        if let Some(idx) = self.index_of_prehashed(&key, hash) {
            return (Some(std::mem::replace(&mut self.values[idx].value, value)), idx);
        }

        let idx = self.values.len();
        self.values.push(FfiIndexMapElement {
            key,
            value,
            hash,
        });
        self.indices.insert_unique(hash, idx, &self.values);

        (None, idx)
    }
    pub fn insert_mut(&mut self, key: K, value: V) -> &mut V
        where K: Hash + Eq
    {
        let hash = self.state.hash(&key);

        if let Some(idx) = self.index_of_prehashed(&key, hash) {
            let value_mut = &mut self.values[idx].value;
            *value_mut = value;
            return value_mut;
        }

        let idx = self.values.len();
        self.values.push(FfiIndexMapElement {
            key,
            value,
            hash,
        });
        self.indices.insert_unique(hash, idx, &self.values);

        &mut self.values[idx].value
    }
    pub fn remove_shift<Q: ?Sized + Eq + Hash>(&mut self, key: &Q) -> Option<V>
        where K: Borrow<Q> + Eq
    {
        let hash = self.state.hash(&key);

        let Some(idx) = self.indices.remove(hash, key, &self.values) else {
            return None;
        };

        Some(self.remove_shift_existing(idx).value)
    }
    pub fn remove_swap<Q: ?Sized + Eq + Hash>(&mut self, key: &Q) -> Option<V>
        where K: Borrow<Q> + Eq
    {
        let hash = self.state.hash(key);

        let Some(idx) = self.indices.remove(hash, key, &self.values) else {
            return None;
        };

        Some(self.remove_swap_existing(idx).value)
    }
    fn remove_shift_existing(&mut self, idx: u64) -> FfiIndexMapElement<K, V>
        where K: Eq
    {
        if idx == self.values.len() - 1 {
            return self.values.pop().unwrap()
        }

        // todo: as usize
        for i in (idx + 1)..self.values.len() {
            let entry = &self.values[i];
            self.indices.set(entry.hash, entry.key.borrow(), i - 1, &self.values);
        }

        let removed_element = self.values.remove(idx).unwrap();

        removed_element
    }
    fn remove_swap_existing(&mut self, idx: u64) -> FfiIndexMapElement<K, V>
        where K: Eq
    {
        if idx == self.values.len() - 1 {
            return self.values.pop().unwrap()
        }

        let moved_element = &self.values[self.values.len() - 1];
       
        self.indices.set(moved_element.hash, moved_element.key.borrow(), idx, &self.values);

        let removed_element = self.values.swap_remove(idx).unwrap();
       
        removed_element
    }
    pub fn len(&self) -> u64 {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn index_of<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> Option<u64>
        where K: Borrow<Q>
    {
        self.index_of_prehashed(key, self.state.hash(key))
    }
    pub fn clear(&mut self) {
        self.indices.clear();
        self.values.clear();
    }
    fn index_of_prehashed<Q: ?Sized + Eq>(&self, key: &Q, hash: u64) -> Option<u64>
        where K: Borrow<Q>
    {
        self.indices.get(hash, key, &self.values)
    }
}
impl<K, V> Default for FfiIndexMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: Clone + Hash + Eq, V: Clone> Clone for FfiIndexMap<K, V> {
    fn clone(&self) -> Self {
        // todo: clone instead of reassembling and remove redudnant generic bounds
        let mut map = FfiIndexMap::new();

        for (key, value) in self.iter() {
            map.insert(key.clone(), value.clone());
        }

        map
    }
}

impl<K, V> FfiIndexMap<K, V> {
    pub fn iter<'a>(&'a self) -> FfiIndexMapPairsIter<'a, K, V> {
        FfiIndexMapPairsIter(self.values.iter())
    }
    pub fn iter_mut<'a>(&'a mut self) -> FfiIndexMapPairsMutIter<'a, K, V> {
        FfiIndexMapPairsMutIter(self.values.iter_mut())
    }
    pub fn keys<'a>(&'a self) -> FfiIndexMapKeysIter<'a, K, V> {
        FfiIndexMapKeysIter(self.values.iter())
    }
    pub fn values<'a>(&'a self) -> FfiIndexMapValuesIter<'a, K, V> {
        FfiIndexMapValuesIter(self.values.iter())
    }
    pub fn values_mut<'a>(&'a mut self) -> FfiIndexMapValuesMutIter<'a, K, V> {
        FfiIndexMapValuesMutIter(self.values.iter_mut())
    }
    pub fn into_keys(self) -> FfiIndexMapIntoKeysIter<K, V> {
        FfiIndexMapIntoKeysIter(self.values.into_iter())
    }
    pub fn into_values(self) -> FfiIndexMapIntoValuesIter<K, V> {
        FfiIndexMapIntoValuesIter(self.values.into_iter())
    }
}

impl<K, V> IntoIterator for FfiIndexMap<K, V> {
    type Item = (K, V);
    type IntoIter = FfiIndexMapIntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        FfiIndexMapIntoIter(self.values.into_iter())
    }
}

impl<'a, K, V> IntoIterator for &'a FfiIndexMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = FfiIndexMapPairsIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut FfiIndexMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = FfiIndexMapPairsMutIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K: Hash + Eq, V> FromIterator<(K, V)> for FfiIndexMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut map = FfiIndexMap::new();

        for (key, value) in iter {
            map.insert(key, value);
        }

        map
    }
}

impl<K: Debug, V: Debug> Debug for FfiIndexMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

unsafe impl<K: Send, V: Send> Send for FfiIndexMap<K, V> { }
unsafe impl<K: Sync, V: Sync> Sync for FfiIndexMap<K, V> { }

// todo: ffi for all iters?

pub struct FfiIndexMapKeysIter<'a, K, V>(<&'a [FfiIndexMapElement<K, V>] as IntoIterator>::IntoIter);
impl<'a, K, V> Iterator for FfiIndexMapKeysIter<'a, K, V> {
    type Item = &'a K;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| &p.key)
    }
}
pub struct FfiIndexMapPairsIter<'a, K, V>(<&'a [FfiIndexMapElement<K, V>] as IntoIterator>::IntoIter);
impl<'a, K, V> Iterator for FfiIndexMapPairsIter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| (&p.key, &p.value))
    }
}
pub struct FfiIndexMapPairsMutIter<'a, K, V>(<&'a mut [FfiIndexMapElement<K, V>] as IntoIterator>::IntoIter);
impl<'a, K, V> Iterator for FfiIndexMapPairsMutIter<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| (&p.key, &mut p.value))
    }
}
pub struct FfiIndexMapValuesIter<'a, K, V>(<&'a [FfiIndexMapElement<K, V>] as IntoIterator>::IntoIter);
impl<'a, K, V> Iterator for FfiIndexMapValuesIter<'a, K, V> {
    type Item = &'a V;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| &p.value)
    }
}
pub struct FfiIndexMapValuesMutIter<'a, K, V>(<&'a mut [FfiIndexMapElement<K, V>] as IntoIterator>::IntoIter);
impl<'a, K, V> Iterator for FfiIndexMapValuesMutIter<'a, K, V> {
    type Item = &'a mut V;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| &mut p.value)
    }
}
pub struct FfiIndexMapIntoIter<K, V>(<FfiVec<FfiIndexMapElement<K, V>> as IntoIterator>::IntoIter);
impl<K, V> Iterator for FfiIndexMapIntoIter<K, V> {
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| (p.key, p.value))
    }
}
pub struct FfiIndexMapIntoKeysIter<K, V>(<FfiVec<FfiIndexMapElement<K, V>> as IntoIterator>::IntoIter);
impl<K, V> Iterator for FfiIndexMapIntoKeysIter<K, V> {
    type Item = K;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| p.key)
    }
}
pub struct FfiIndexMapIntoValuesIter<K, V>(<FfiVec<FfiIndexMapElement<K, V>> as IntoIterator>::IntoIter);
impl<K, V> Iterator for FfiIndexMapIntoValuesIter<K, V> {
    type Item = V;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|p| p.value)
    }
}

#[repr(C)]
pub struct FfiIndexMapOccupiedEntry<'a, K: 'a, V: 'a> {
    map: &'a mut FfiIndexMap<K, V>,
    idx: u64,
}

impl<K: Debug, V: Debug> Debug for FfiIndexMapOccupiedEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish_non_exhaustive()
    }
}

#[repr(C)]
pub struct FfiIndexMapVacantEntry<'a, K: 'a, V: 'a> {
    map: &'a mut FfiIndexMap<K, V>,
    key: K,
}

impl<K: Debug, V> Debug for FfiIndexMapVacantEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}

#[repr(C)]
pub enum FfiIndexMapEntry<'a, K: 'a, V: 'a> {
    Occupied(FfiIndexMapOccupiedEntry<'a, K, V>),
    Vacant(FfiIndexMapVacantEntry<'a, K, V>),
}


impl<'a, K, V> FfiIndexMapEntry<'a, K, V> {
    pub fn or_insert(self, default: V) -> &'a mut V
        where K: Eq + Hash
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default),
        }
    }

    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V
        where K: Eq + Hash
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default()),
        }
    }

    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V
        where K: Eq + Hash
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    pub fn key(&self) -> &K {
        match *self {
            Self::Occupied(ref entry) => entry.key(),
            Self::Vacant(ref entry) => entry.key(),
        }
    }

    pub fn and_modify<F>(self, f: F) -> Self
    where
        K: Eq + Hash,
        F: FnOnce(&mut V),
    {
        match self {
            Self::Occupied(mut entry) => {
                f(entry.get_mut());
                Self::Occupied(entry)
            }
            Self::Vacant(entry) => Self::Vacant(entry),
        }
    }

    pub fn insert_entry(self, value: V) -> FfiIndexMapOccupiedEntry<'a, K, V>
        // todo: remove Clone
        where K: Eq + Hash + Clone
    {
        match self {
            Self::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            Self::Vacant(entry) => entry.insert_entry(value),
        }
    }
}

impl<'a, K, V: Default> FfiIndexMapEntry<'a, K, V> {
    pub fn or_default(self) -> &'a mut V
        where K: Eq + Hash
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(Default::default()),
        }
    }
}

impl<'a, K, V> FfiIndexMapOccupiedEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        &self.map.values[self.idx].key
    }

    pub fn remove_entry_swap(self) -> (K, V)
        where K: Eq
    {
        let pair = self.map.remove_swap_existing(self.idx);
        (pair.key, pair.value)
    }

    pub fn remove_entry_shift(self) -> (K, V)
        where K: Eq
    {
        let pair = self.map.remove_shift_existing(self.idx);
        (pair.key, pair.value)
    }

    pub fn get(&self) -> &V {
        &self.map.values[self.idx].value
    }

    pub fn get_mut(&mut self) -> &mut V {
        &mut self.map.values[self.idx].value
    }

    pub fn into_mut(self) -> &'a mut V {
        &mut self.map.values[self.idx].value
    }

    pub fn insert(&mut self, value: V) -> V {
        std::mem::replace(&mut self.map.values[self.idx].value, value)
    }

    pub fn remove_swap(self) -> V
        where K: Eq
    {
        self.map.remove_swap_existing(self.idx).value
    }

    pub fn remove_shift(self) -> V
        where K: Eq
    {
        self.map.remove_shift_existing(self.idx).value
    }
}

impl<'a, K: 'a, V: 'a> FfiIndexMapVacantEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn into_key(self) -> K {
        self.key
    }

    pub fn insert(self, value: V) -> &'a mut V
        where K: Eq + Hash
    {
        self.map.insert_mut(self.key, value)
    }

    pub fn insert_entry(self, value: V) -> FfiIndexMapOccupiedEntry<'a, K, V>
        where K: Eq + Hash
    {
        let (_, idx) = self.map.insert_and_idx(self.key, value);

        FfiIndexMapOccupiedEntry {
            idx,
            map: self.map,
        }
    }
}

