pub fn ref_into_nullable_ptr<T>(v: Option<&T>) -> *const T {
    match v {
        Some(v) => &raw const *v,
        None => std::ptr::null(),
    }
}

pub fn mut_into_nullable_ptr<T>(v: Option<&mut T>) -> *mut T {
    match v {
        Some(v) => &raw mut *v,
        None => std::ptr::null_mut(),
    }
}

pub unsafe fn ref_from_nullable_ptr<'r, T>(v: *const T) -> Option<&'r T> {
    unsafe { if v.is_null() { None } else { Some(&*v) } }
}

pub unsafe fn mut_from_nullable_ptr<'r, T>(v: *mut T) -> Option<&'r mut T> {
    unsafe { if v.is_null() { None } else { Some(&mut *v) } }
}
