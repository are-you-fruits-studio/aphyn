use std::{cmp::Ordering, fmt::Debug};

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub enum FfiOption<T> {
    Some(T),
    None,
}

impl<T> FfiOption<T> {
    pub fn from_option(v: Option<T>) -> Self {
        match v {
            None => Self::None,
            Some(v) => Self::Some(v),
        }
    }

    pub fn into_option(self) -> Option<T> {
        match self {
            Self::None => None,
            Self::Some(v) => Some(v),
        }
    }

    pub const fn as_ref(&self) -> Option<&T> {
        match self {
            Self::None => None,
            Self::Some(v) => Some(v),
        }
    }

    pub const fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::None => None,
            Self::Some(v) => Some(v),
        }
    }

    pub const fn is_some(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn take(&mut self) -> Option<T> {
        std::mem::replace(self, FfiOption::None).into()
    }
}

impl<'a, T> FfiOption<&'a T> {
    pub const fn as_option<'r>(&'r self) -> Option<&'a T>
        where 'r: 'a
    {
        match self {
            FfiOption::None => None,
            FfiOption::Some(v) => Some(v),
        }
    }
}

impl<'a, T> FfiOption<&'a mut T> {
    pub const fn as_option<'r>(&'r self) -> Option<&'a T>
        where 'r: 'a
    {
        match self {
            FfiOption::None => None,
            FfiOption::Some(v) => Some(v),
        }
    }
    pub const fn as_option_mut<'r>(&'r mut self) -> Option<&'a mut T>
        where 'r: 'a
    {
        match self {
            FfiOption::None => None,
            FfiOption::Some(v) => Some(v),
        }
    }
}

impl<T: Debug> Debug for FfiOption<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Self::Some(v) = self {
            f.debug_tuple("FfiOption::Some").field(v).finish()
        } else {
            f.debug_tuple("FfiOption::None").finish()
        }
    }
}

impl<T> Default for FfiOption<T> {
    fn default() -> Self {
        Self::None
    }
}

impl<T> From<Option<T>> for FfiOption<T> {
    fn from(value: Option<T>) -> Self {
        Self::from_option(value)
    }
}

impl<T> From<FfiOption<T>> for Option<T> {
    fn from(value: FfiOption<T>) -> Self {
        value.into_option()
    }
}

impl<T> From<T> for FfiOption<T> {
    fn from(value: T) -> Self {
        Self::Some(value)
    }
}

impl<T: Clone> Clone for FfiOption<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Some(v) => Self::Some(v.clone()),
            Self::None => Self::None,
        }
    }
}

impl<T: PartialEq> PartialEq for FfiOption<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(_), None) => false,
            (Some(l), Some(r)) => l == r,
        }
    }
}

impl<T: PartialOrd> PartialOrd for FfiOption<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.as_ref(), other.as_ref()) {
            (None, None) => Some(Ordering::Equal),
            (None, Some(_)) => Some(Ordering::Less),
            (Some(_), None) => Some(Ordering::Greater),
            (Some(l), Some(r)) => l.partial_cmp(r),
        }
    }
}

impl<T: Ord> Ord for FfiOption<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.as_ref(), other.as_ref()) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(l), Some(r)) => l.cmp(r),
        }
    }
}

impl<T: Eq> Eq for FfiOption<T> {}
impl<T: Copy> Copy for FfiOption<T> {}

// todo: impl standard traits
