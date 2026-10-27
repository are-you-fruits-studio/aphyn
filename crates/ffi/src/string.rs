use std::{
    borrow::Borrow,
    fmt::{Debug, Display, Write},
    hash::Hash,
    ops::{Deref, DerefMut},
};

use crate::FfiVec;

// todo
#[repr(C)]
#[derive(Default, Clone)]
pub struct FfiString {
    data: FfiVec<u8>,
}

impl FfiString {
    pub fn new() -> Self {
        String::new().into()
    }

    pub fn from_string(v: String) -> Self {
        Self {
            data: v.into_bytes().into(),
        }
    }

    pub const fn as_str(&self) -> &str {
        // todo
        unsafe { str::from_utf8_unchecked(self.data.as_slice()) }
    }

    pub const fn as_mut_str(&mut self) -> &mut str {
        // todo
        unsafe { str::from_utf8_unchecked_mut(self.data.as_mut_slice()) }
    }

    pub fn to_string(&self) -> String {
        self.as_str().to_string()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn push_str(&mut self, string: &str) {
        for &byte in string.as_bytes() {
            self.data.push(byte);
        }
    }
}

impl Deref for FfiString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl DerefMut for FfiString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_str()
    }
}

impl From<String> for FfiString {
    fn from(value: String) -> Self {
        Self::from_string(value)
    }
}

impl From<&str> for FfiString {
    fn from(value: &str) -> Self {
        Self::from_string(value.to_string())
    }
}

impl From<&mut str> for FfiString {
    fn from(value: &mut str) -> Self {
        Self::from_string(value.to_string())
    }
}

impl Debug for FfiString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for FfiString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl PartialEq for FfiString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Write for FfiString {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        // todo: optimize
        for &byte in s.as_bytes() {
            self.data.push(byte);
        }

        Ok(())
    }
}

impl Eq for FfiString {}

impl Hash for FfiString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialOrd for FfiString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl Ord for FfiString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Borrow<str> for FfiString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

unsafe impl Send for FfiString {}
unsafe impl Sync for FfiString {}

// todo: impl standard traits
