use std::{
    ffi::c_void,
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

//
// todo

#[repr(C)]
struct FfiDroppableData<T> {
    meta: FfiDroppableMetadata,
    value: T,
}

#[repr(C)]
struct FfiDroppableMetadata {
    drop_fn: unsafe extern "C-unwind" fn(*mut c_void),
    value_ptr: *mut c_void,
}

#[repr(transparent)]
pub struct FfiDroppable {
    data: *mut c_void,
}

impl FfiDroppable {
    pub fn new<T>(v: T) -> Self {
        unsafe extern "C-unwind" fn ffi_drop<D>(data: *mut c_void) {
            unsafe { drop(Box::from_raw(data as *mut FfiDroppableData<D>)) }
        }

        unsafe {
            let data = FfiDroppableData {
                meta: FfiDroppableMetadata {
                    value_ptr: std::ptr::null_mut(),
                    drop_fn: ffi_drop::<T>,
                },
                value: v,
            };

            let data = Box::into_raw(Box::new(data));

            let value_ptr = &raw mut (*data).value;

            (&mut *data).meta.value_ptr = value_ptr as *mut c_void;

            Self {
                data: data as *mut c_void,
            }
        }
    }

    pub fn get(&self) -> *mut c_void {
        unsafe { (&*(self.data as *mut FfiDroppableMetadata)).value_ptr }
    }
}

impl Drop for FfiDroppable {
    fn drop(&mut self) {
        unsafe {
            let drop_fn = (&*(self.data as *mut FfiDroppableMetadata)).drop_fn;

            drop_fn(self.data);
        }
    }
}

#[repr(transparent)]
pub struct FfiTypedDroppable<T> {
    value: FfiDroppable,
    _phantom: PhantomData<T>,
}

impl<T> FfiTypedDroppable<T> {
    pub fn new(v: T) -> Self {
        Self {
            value: FfiDroppable::new(v),
            _phantom: PhantomData,
        }
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.get_ptr() }
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.get_ptr() }
    }

    pub fn get_ptr(&self) -> *mut T {
        self.value.get() as *mut T
    }
}

impl<T> Deref for FfiTypedDroppable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> DerefMut for FfiTypedDroppable<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T: Default> Default for FfiTypedDroppable<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone> Clone for FfiTypedDroppable<T> {
    fn clone(&self) -> Self {
        Self::new(self.get().clone())
    }
}

impl<T: Debug> Debug for FfiTypedDroppable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: Display> Display for FfiTypedDroppable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: Hash> Hash for FfiTypedDroppable<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get().hash(state);
    }
}

impl<T: PartialEq> PartialEq for FfiTypedDroppable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq> Eq for FfiTypedDroppable<T> {}

impl<T: PartialOrd> PartialOrd for FfiTypedDroppable<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.get().partial_cmp(other.get())
    }
}

impl<T: Ord> Ord for FfiTypedDroppable<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.get().cmp(other.get())
    }
}

unsafe impl<T: Send> Send for FfiTypedDroppable<T> {}
unsafe impl<T: Sync> Sync for FfiTypedDroppable<T> {}

//

#[repr(transparent)]
pub struct FfiStaticRef<T: 'static> {
    data: *const T,
}

impl<T: 'static> FfiStaticRef<T> {
    pub const fn new(data: &'static T) -> Self {
        Self { data }
    }

    pub const fn get(&self) -> &'static T {
        unsafe { &*self.data }
    }
}

impl<T: 'static> Clone for FfiStaticRef<T> {
    fn clone(&self) -> Self {
        Self { data: self.data }
    }
}

impl<T: 'static> Copy for FfiStaticRef<T> {}

impl<T: 'static> Deref for FfiStaticRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

unsafe impl<T: 'static> Send for FfiStaticRef<T> where &'static T: Send {}
unsafe impl<T: 'static> Sync for FfiStaticRef<T> where &'static T: Sync {}
