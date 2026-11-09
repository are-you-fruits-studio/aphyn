use std::{
    ffi::c_void,
    fmt::{Debug, Display},
    marker::PhantomData,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{self, AtomicUsize, Ordering},
};

#[repr(C)]
struct FfiArcInner<T> {
    strong: AtomicUsize,
    drop_fn: unsafe extern "C-unwind" fn(*mut c_void),
    value: T,
}

#[repr(C)]
pub struct FfiArc<T> {
    ptr: NonNull<FfiArcInner<T>>,
    _phantom: PhantomData<FfiArcInner<T>>,
}

impl<T> FfiArc<T> {
    pub fn new(value: T) -> Self {
        unsafe extern "C-unwind" fn ffi_drop<T>(ptr: *mut c_void) {
            unsafe { drop(Box::from_raw(ptr as *mut FfiArcInner<T>)) }
        }

        let inner = Box::new(FfiArcInner {
            strong: AtomicUsize::new(1),
            drop_fn: ffi_drop::<T>,
            value,
        });

        Self {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(inner)) },
            _phantom: PhantomData,
        }
    }

    pub fn strong_count(this: &Self) -> usize {
        this.inner().strong.load(Ordering::Acquire)
    }

    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        a.ptr == b.ptr
    }

    fn inner(&self) -> &FfiArcInner<T> {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> Clone for FfiArc<T> {
    fn clone(&self) -> Self {
        let old = self.inner().strong.fetch_add(1, Ordering::Relaxed);

        if old > isize::MAX as usize {
            std::process::abort();
        }

        Self {
            ptr: self.ptr,
            _phantom: PhantomData,
        }
    }
}

impl<T> Drop for FfiArc<T> {
    fn drop(&mut self) {
        if self.inner().strong.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        atomic::fence(Ordering::Acquire);

        unsafe {
            let drop_fn = self.inner().drop_fn;
            drop_fn(self.ptr.as_ptr() as *mut c_void);
        }
    }
}

impl<T> Deref for FfiArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

impl<T: Debug> Debug for FfiArc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&**self, f)
    }
}

impl<T: Display> Display for FfiArc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&**self, f)
    }
}

unsafe impl<T: Send + Sync> Send for FfiArc<T> {}
unsafe impl<T: Send + Sync> Sync for FfiArc<T> {}

#[cfg(test)]
mod tests {
    use super::FfiArc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DropCounter<'a>(&'a AtomicUsize);

    impl Drop for DropCounter<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn drop_runs_once_after_last_clone() {
        let drops = AtomicUsize::new(0);

        let a = FfiArc::new(DropCounter(&drops));
        let b = a.clone();
        let c = b.clone();

        drop(a);
        drop(b);
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        drop(c);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn strong_count_tracks_clones() {
        let a = FfiArc::new(5);
        assert_eq!(FfiArc::strong_count(&a), 1);

        let b = a.clone();
        assert_eq!(FfiArc::strong_count(&a), 2);
        assert_eq!(FfiArc::strong_count(&b), 2);
        assert!(FfiArc::ptr_eq(&a, &b));
        assert!(!FfiArc::ptr_eq(&a, &FfiArc::new(5)));

        drop(b);
        assert_eq!(FfiArc::strong_count(&a), 1);
    }

    #[test]
    fn clone_across_threads() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        let arc = FfiArc::new(DropCounter(&DROPS));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let local = arc.clone();
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        drop(local.clone());
                    }
                    drop(local);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(FfiArc::strong_count(&arc), 1);
        assert_eq!(DROPS.load(Ordering::SeqCst), 0);

        drop(arc);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    }
}
