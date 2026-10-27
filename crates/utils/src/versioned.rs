use std::ops::{Deref, DerefMut};

pub struct Versioned<T> {
    value: T,
    version: u64,
}

impl<T> Versioned<T> {
    pub fn new(v: T) -> Self {
        Self {
            value: v,
            version: 0,
        }
    }

    pub fn into_inner(self) -> T {
        self.value
    }

    pub fn version(v: &Self) -> u64 {
        v.version
    }

    pub fn get_silent(v: &mut Self) -> *mut T {
        &mut v.value
    }

    pub fn increment_version(v: &mut Self) {
        v.version += 1;
    }
}

impl<T: Default> Default for Versioned<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Deref for Versioned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Versioned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.version += 1;
        &mut self.value
    }
}

impl<T> From<T> for Versioned<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}