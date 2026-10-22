use std::cmp::Ordering;

use fruits_ffi::{FfiOption, FfiVec, FfiVecDeque};

// todo?: Option to bitvec + MaybeUninit data
#[repr(C)]
struct DataWithVersion<T> {
    pub version: u64,
    pub data: FfiOption<T>,
}

#[repr(C)]
pub struct VersionCollection<T> {
    items: FfiVec<DataWithVersion<T>>,
    free_places: FfiVecDeque<u64>,
    // reserved_places: VecDeque<usize>,
    count: u64,
}
impl<T> Default for VersionCollection<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VersionIndex {
    pub index: u64,
    pub version: u64,
}

impl VersionIndex {
    pub const EMPTY: VersionIndex = VersionIndex { index: 0, version: 0 };
}

impl Default for VersionIndex {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Ord for VersionIndex {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.index.cmp(&other.index) {
            Ordering::Equal => self.version.cmp(&other.version),
            cmp => cmp,
        }
    }
}

impl PartialOrd for VersionIndex {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> VersionCollection<T> {
    pub fn new() -> Self {
        Self {
            items: FfiVec::new(),
            free_places: FfiVecDeque::new(),
            count: 0,
        }
    }

    pub fn insert(&mut self, data: T) -> VersionIndex {
        if let Some(index) = self.free_places.pop_front() {
            let item = &mut self.items[index];

            item.data = Some(data).into();

            self.count = self
                .count
                .checked_add(1)
                .unwrap_or_else(|| panic!("VersionCollection count overflow"));

            return VersionIndex {
                index,
                version: item.version,
            };
        }

        let index: u64 = self
            .items
            .len()
            .try_into()
            .unwrap_or_else(|_| panic!("VersionCollection count overflow"));
        // entities with version 0 cannot exist.
        let version = 1;

        self.items.push(DataWithVersion::<T> {
            data: Some(data).into(),
            version,
        });

        self.count = self
            .count
            .checked_add(1)
            .unwrap_or_else(|| panic!("VersionCollection count overflow"));

        VersionIndex { index, version }
    }

    pub fn remove(&mut self, index: VersionIndex) -> Option<T> {
        let data_with_version = self.get_data_with_version_mut(index)?;

        data_with_version.version = data_with_version.version.wrapping_add(1);

        // to prevent version = 0.
        if data_with_version.version == 0 {
            data_with_version.version += 1;
        }

        let data = data_with_version.data.take().unwrap();

        self.free_places.push_back(index.index);

        self.count -= 1;

        Some(data)
    }

    pub fn get_by_idx_only(&self, idx: u64) -> Option<VersionIndex> {
        let data_with_version = self.items.get(idx)?;

        Some(VersionIndex {
            index: idx,
            version: data_with_version.version,
        })
    }

    pub fn get_by_idx_only_ref(&self, idx: u64) -> Option<(VersionIndex, &T)> {
        let data_with_version = self.items.get(idx)?;

        let data = data_with_version.data.as_ref()?;

        Some((
            VersionIndex {
                index: idx,
                version: data_with_version.version,
            },
            data,
        ))
    }

    pub fn get_by_idx_only_mut(&mut self, idx: u64) -> Option<(VersionIndex, &mut T)> {
        let data_with_version = self.items.get_mut(idx)?;

        let data = data_with_version.data.as_mut()?;

        Some((
            VersionIndex {
                index: idx,
                version: data_with_version.version,
            },
            data,
        ))
    }

    pub fn get(&self, index: VersionIndex) -> Option<&T> {
        let data_with_version = self.items.get(index.index)?;

        if index.version != data_with_version.version {
            return None;
        }

        Some(data_with_version.data.as_ref().unwrap())
    }

    pub fn get_mut(&mut self, index: VersionIndex) -> Option<&mut T> {
        self.get_data_with_version_mut(index).map(|d| d.data.as_mut().unwrap())
    }

    fn get_data_with_version_mut(&mut self, index: VersionIndex) -> Option<&mut DataWithVersion<T>> {
        let data_with_version = self.items.get_mut(index.index)?;

        if index.version != data_with_version.version {
            return None;
        }

        Some(data_with_version)
    }

    pub fn contains_index(&self, index: VersionIndex) -> bool {
        let Some(data_with_version) = self.items.get(index.index) else {
            return false;
        };

        index.version == data_with_version.version
    }

    pub fn len(&self) -> u64 {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn iter<'a>(&'a self) -> VersionCollectionIter<'a, T> {
        VersionCollectionIter {
            collection: self,
            next_index: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a VersionCollection<T> {
    type Item = &'a T;

    type IntoIter = VersionCollectionIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[repr(C)]
pub struct VersionCollectionIter<'a, T> {
    collection: &'a VersionCollection<T>,
    next_index: u64,
}

impl<'a, T> Iterator for VersionCollectionIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.collection.items.get(self.next_index) {
            self.next_index += 1;

            if let FfiOption::Some(item) = &item.data {
                return Some(item);
            }
        }

        return None;
    }
}
// todo: other iterators?