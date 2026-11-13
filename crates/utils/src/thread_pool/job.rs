use std::{
    any::Any,
    ffi::c_void,
    marker::PhantomData,
    mem::MaybeUninit,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Condvar, Mutex, MutexGuard},
};

use aphyn_ffi::{FfiArc, FfiDroppable};

#[repr(C)]
struct JobStateFfi {
    native: FfiDroppable,
    is_finished_fn: unsafe extern "C-unwind" fn(*const c_void) -> bool,
    wait_fn: unsafe extern "C-unwind" fn(*const c_void),
    take_fn: unsafe extern "C-unwind" fn(*const c_void, *mut c_void),
}

// The native state is a `JobStateNative<T>` with `T: Send`.
unsafe impl Send for JobStateFfi {}
unsafe impl Sync for JobStateFfi {}

struct JobStateNative<T> {
    slot: Mutex<JobSlot<T>>,
    cv: Condvar,
}

enum JobSlot<T> {
    Pending,
    Done(T),
    Panicked(Box<dyn Any + Send>),
    Taken,
}

impl<T> JobStateNative<T> {
    fn lock(&self) -> MutexGuard<'_, JobSlot<T>> {
        self.slot.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[repr(C)]
pub struct JobHandle<T: 'static + Send> {
    state: FfiArc<JobStateFfi>,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: 'static + Send> JobHandle<T> {
    pub fn is_finished(&self) -> bool {
        unsafe { (self.state.is_finished_fn)(self.state.native.get()) }
    }

    pub fn try_take(self) -> Result<T, Self> {
        if self.is_finished() { Ok(self.take()) } else { Err(self) }
    }

    pub fn block_and_take(self) -> T {
        unsafe { (self.state.wait_fn)(self.state.native.get()) };

        self.take()
    }

    fn take(self) -> T {
        let mut result = MaybeUninit::<T>::uninit();

        unsafe {
            (self.state.take_fn)(self.state.native.get(), result.as_mut_ptr() as *mut c_void);
            result.assume_init()
        }
    }
}

pub(super) fn create_job<F: 'static + Send + FnOnce() -> T, T: 'static + Send>(
    f: F,
) -> (JobHandle<T>, impl 'static + Send + FnOnce()) {
    unsafe extern "C-unwind" fn ffi_is_finished<T>(native: *const c_void) -> bool {
        let native = unsafe { &*(native as *const JobStateNative<T>) };

        matches!(*native.lock(), JobSlot::Done(_) | JobSlot::Panicked(_))
    }

    unsafe extern "C-unwind" fn ffi_wait<T>(native: *const c_void) {
        let native = unsafe { &*(native as *const JobStateNative<T>) };

        let mut slot = native.lock();

        while let JobSlot::Pending = *slot {
            slot = native.cv.wait(slot).unwrap_or_else(|e| e.into_inner());
        }
    }

    unsafe extern "C-unwind" fn ffi_take<T>(native: *const c_void, out: *mut c_void) {
        let native = unsafe { &*(native as *const JobStateNative<T>) };

        let slot = std::mem::replace(&mut *native.lock(), JobSlot::Taken);

        match slot {
            JobSlot::Done(v) => unsafe { (out as *mut T).write(v) },
            JobSlot::Panicked(panic) => resume_unwind(panic),
            JobSlot::Pending | JobSlot::Taken => unreachable!(),
        }
    }

    let state = FfiArc::new(JobStateFfi {
        native: FfiDroppable::new(JobStateNative::<T> {
            slot: Mutex::new(JobSlot::Pending),
            cv: Condvar::new(),
        }),
        is_finished_fn: ffi_is_finished::<T>,
        wait_fn: ffi_wait::<T>,
        take_fn: ffi_take::<T>,
    });

    let executor_state = state.clone();

    let executor = move || {
        let result = match catch_unwind(AssertUnwindSafe(f)) {
            Ok(v) => JobSlot::Done(v),
            Err(panic) => JobSlot::Panicked(panic),
        };

        let native = unsafe { &*(executor_state.native.get() as *const JobStateNative<T>) };

        *native.lock() = result;
        native.cv.notify_all();
    };

    (
        JobHandle {
            state,
            _phantom: PhantomData,
        },
        executor,
    )
}

unsafe impl<T: 'static + Send> Send for JobHandle<T> {}
unsafe impl<T: 'static + Send> Sync for JobHandle<T> {}
