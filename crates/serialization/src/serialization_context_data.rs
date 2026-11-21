use std::{ffi::c_void, marker::PhantomData};

use aphyn_ffi::{FfiIndexMap, FfiString};

pub trait ContextDataKey: 'static {
    type Value<'a>;
}

#[repr(C)]
pub struct ContextData<'a> {
    entries: FfiIndexMap<FfiString, *const c_void>,
    _phantom: PhantomData<fn(&'a ()) -> &'a ()>,
}

impl<'a> ContextData<'a> {
    pub fn new() -> Self {
        Self {
            entries: FfiIndexMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn insert<K: ContextDataKey>(&mut self, value: &'a K::Value<'a>) {
        let value = value as *const K::Value<'a> as *const c_void;
        self.entries.insert(std::any::type_name::<K>().into(), value);
    }

    pub fn with<K: ContextDataKey>(mut self, value: &'a K::Value<'a>) -> Self {
        self.insert::<K>(value);
        self
    }

    pub fn get<K: ContextDataKey>(&self) -> Option<&'a K::Value<'a>> {
        let value = *self.entries.get(std::any::type_name::<K>())?;
        // the entry was inserted under the same key with the same lifetime
        Some(unsafe { &*(value as *const K::Value<'a>) })
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|k| k.as_str())
    }
}

impl<'a> Default for ContextData<'a> {
    fn default() -> Self {
        Self::new()
    }
}
