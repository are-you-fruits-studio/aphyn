use core::str;
use std::{fmt::{Debug, Display}, hash::Hash, ops::{Deref, DerefMut}};

const MAX_BYTES: usize = 23;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiSmallString {
    // Invariant:
    //
    // len <= MAX_BYTES
    // bytes[..len] contains valid UTF-8.
    bytes: [u8; MAX_BYTES],
    len: u8,
}

impl FfiSmallString {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_BYTES],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn capacity(&self) -> usize {
        MAX_BYTES
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn as_bytes(&self) -> &[u8] {
        // todo: use index when it's stable to use in const context.
        unsafe { std::slice::from_raw_parts(&raw const self.bytes[0], self.len()) }
    }

    const fn as_bytes_mut(&mut self) -> &mut [u8] {
        // todo: use index when it's stable to use in const context.
        unsafe { std::slice::from_raw_parts_mut(&raw mut self.bytes[0], self.len()) }
    }

    pub const fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    pub const fn as_str_mut(&mut self) -> &mut str {
        unsafe { str::from_utf8_unchecked_mut(self.as_bytes_mut()) }
    }

    pub fn push(&mut self, ch: char) -> bool {
        let len = self.len();
        let ch_len = ch.len_utf8();

        if len + ch_len > MAX_BYTES {
            return false;
        }

        ch.encode_utf8(&mut self.bytes[len..]);

        self.len += ch_len as u8;

        true
    }

    fn push_bytes_cut(&mut self, bytes: &[u8]) {
        let len = self.len();
        let available_len = MAX_BYTES - len;
        let moved_len = available_len.min(bytes.len());
        self.bytes[len..(len + moved_len)].copy_from_slice(&bytes[..moved_len]);
        self.len += moved_len as u8;
    }

    /// only pushes the string if enough space
    pub fn push_str_full(&mut self, string: &str) -> bool {
        if string.len() > MAX_BYTES - self.len() {
            return false;
        }

        self.push_bytes_cut(string.as_bytes());
        true
    }

    /// returns true if the string did not fit and had to be cut
    pub fn push_str_cut(&mut self, string: &str) -> bool {
        let did_cut = string.len() > MAX_BYTES - self.len();

        let available = MAX_BYTES - self.len();

        let mut string_end = available.min(string.len());

        while !string.is_char_boundary(string_end) {
            string_end -= 1;
        }

        self.push_bytes_cut(&string.as_bytes()[..string_end]);

        did_cut
    }

    pub fn pop(&mut self) -> Option<char> {
        let ch = self.chars().next_back()?;
        self.len -= ch.len_utf8() as u8;
        Some(ch)
    }

    pub const fn clear(&mut self) {
        self.len = 0;
    }
}

impl Default for FfiSmallString {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for FfiSmallString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl DerefMut for FfiSmallString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_str_mut()
    }
}

impl PartialEq for FfiSmallString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for FfiSmallString { }

impl PartialOrd for FfiSmallString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl Ord for FfiSmallString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for FfiSmallString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl Debug for FfiSmallString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for FfiSmallString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl From<&str> for FfiSmallString {
    fn from(value: &str) -> Self {
        let mut small = Self::new();

        small.push_str_cut(value);

        small
    }
}