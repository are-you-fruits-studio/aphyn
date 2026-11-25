use std::{
    cell::UnsafeCell,
    ffi::c_void,
    ops::Range,
    sync::{Condvar, Mutex},
};

use aphyn_ffi::{FfiArc, FfiDroppable, FfiOption};
use aphyn_profiling::{ProfileCtx, ProfileSpanKind};

mod ffi;
mod graph;
mod job;
mod native;
mod scope;

pub use ffi::*;
pub use graph::{GraphIter, TaskId};
pub use job::JobHandle;
pub use scope::Scope;

use graph::{ChunksIter, chunk_range, chunks_count};
use native::ThreadPoolNative;

#[repr(C)]
struct ThreadPoolInner {
    native: FfiDroppable,
    execute_batch_fn: unsafe extern "C-unwind" fn(*const c_void, FfiBatch),
    push_frame_job_fn: unsafe extern "C-unwind" fn(*const c_void, FfiJob),
    push_background_job_fn: unsafe extern "C-unwind" fn(*const c_void, FfiJob),
    threads_count_fn: unsafe extern "C-unwind" fn(*const c_void) -> u64,
    background_threads_count_fn: unsafe extern "C-unwind" fn(*const c_void) -> u64,
    did_panic_fn: unsafe extern "C-unwind" fn(*const c_void) -> bool,
    current_worker_index_fn: unsafe extern "C-unwind" fn(*const c_void) -> u32,
}

#[repr(C)]
#[derive(Clone)]
pub struct ThreadPool {
    inner: FfiArc<ThreadPoolInner>,
}

impl ThreadPool {
    pub fn new(threads_count: usize, background_threads_count: usize) -> Self {
        unsafe fn native<'a>(this: *const c_void) -> &'a ThreadPoolNative {
            unsafe { &*(this as *const ThreadPoolNative) }
        }

        unsafe extern "C-unwind" fn ffi_execute_batch(this: *const c_void, batch: FfiBatch) {
            unsafe { native(this).execute_batch(batch) }
        }

        unsafe extern "C-unwind" fn ffi_push_frame_job(this: *const c_void, job: FfiJob) {
            unsafe { native(this).push_frame_job(job) }
        }

        unsafe extern "C-unwind" fn ffi_push_background_job(this: *const c_void, job: FfiJob) {
            unsafe { native(this).push_background_job(job) }
        }

        unsafe extern "C-unwind" fn ffi_threads_count(this: *const c_void) -> u64 {
            unsafe { native(this).threads_count() as u64 }
        }

        unsafe extern "C-unwind" fn ffi_background_threads_count(this: *const c_void) -> u64 {
            unsafe { native(this).background_threads_count() as u64 }
        }

        unsafe extern "C-unwind" fn ffi_did_panic(this: *const c_void) -> bool {
            unsafe { native(this).did_panic() }
        }

        unsafe extern "C-unwind" fn ffi_current_worker_index(this: *const c_void) -> u32 {
            unsafe { native(this).current_worker_index() }
        }

        Self {
            inner: FfiArc::new(ThreadPoolInner {
                native: FfiDroppable::new(ThreadPoolNative::new(threads_count, background_threads_count)),
                execute_batch_fn: ffi_execute_batch,
                push_frame_job_fn: ffi_push_frame_job,
                push_background_job_fn: ffi_push_background_job,
                threads_count_fn: ffi_threads_count,
                background_threads_count_fn: ffi_background_threads_count,
                did_panic_fn: ffi_did_panic,
                current_worker_index_fn: ffi_current_worker_index,
            }),
        }
    }

    pub fn default_threads_count() -> usize {
        Self::available_parallelism().saturating_sub(1).max(1)
    }

    pub fn default_background_threads_count() -> usize {
        (Self::available_parallelism() / 4).max(1)
    }

    fn available_parallelism() -> usize {
        std::thread::available_parallelism().map_or(4, |c| c.get())
    }

    pub fn threads_count(&self) -> usize {
        unsafe { (self.inner.threads_count_fn)(self.inner.native.get()) as usize }
    }

    pub fn background_threads_count(&self) -> usize {
        unsafe { (self.inner.background_threads_count_fn)(self.inner.native.get()) as usize }
    }

    pub fn current_worker_index(&self) -> u32 {
        unsafe { (self.inner.current_worker_index_fn)(self.inner.native.get()) }
    }

    pub fn access(&self) -> ThreadPoolAccess<'_> {
        ThreadPoolAccess::from(self)
    }

    pub fn panic_if_err(&self) {
        if unsafe { (self.inner.did_panic_fn)(self.inner.native.get()) } {
            panic!("ThreadPool worker thread panicked");
        }
    }

    fn execute_batch(&self, batch: FfiBatch) {
        unsafe { (self.inner.execute_batch_fn)(self.inner.native.get(), batch) }
    }

    fn push_frame_job(&self, job: FfiJob) {
        unsafe { (self.inner.push_frame_job_fn)(self.inner.native.get(), job) }
    }

    fn push_background_job(&self, job: FfiJob) {
        unsafe { (self.inner.push_background_job_fn)(self.inner.native.get(), job) }
    }
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::new(Self::default_threads_count(), Self::default_background_threads_count())
    }
}

unsafe impl Send for ThreadPool {}
unsafe impl Sync for ThreadPool {}

//

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ThreadPoolAccess<'p> {
    pool: FfiOption<&'p ThreadPool>,
    profile: FfiOption<ProfileCtx<'p>>,
}

impl ThreadPoolAccess<'static> {
    pub const fn single_threaded() -> Self {
        Self {
            pool: FfiOption::None,
            profile: FfiOption::None,
        }
    }
}

impl<'p> ThreadPoolAccess<'p> {
    // Safety: the pool (if any) and the profile context (if any) must outlive every use of the returned access.
    pub unsafe fn extend_lifetime<'l>(self) -> ThreadPoolAccess<'l> {
        unsafe { std::mem::transmute::<ThreadPoolAccess<'p>, ThreadPoolAccess<'l>>(self) }
    }

    pub fn with_profile<'l>(self, profile: Option<ProfileCtx<'l>>) -> ThreadPoolAccess<'l>
    where
        'p: 'l,
    {
        ThreadPoolAccess {
            pool: self.pool,
            profile: profile.into(),
        }
    }

    pub fn profile(&self) -> Option<ProfileCtx<'p>> {
        self.profile.into_option()
    }

    pub fn is_single_threaded(&self) -> bool {
        self.pool().is_none()
    }

    pub fn current_worker_index(&self) -> u32 {
        self.pool().map_or(0, ThreadPool::current_worker_index)
    }

    pub fn parallelism(&self) -> usize {
        self.pool().map_or(1, |p| p.threads_count() + 1)
    }

    pub fn execute_graph<I: GraphIter, F: Sync + Fn(I::Item)>(self, iter: I, f: F) {
        match self.profile() {
            None => self.execute_graph_unprofiled(iter, f),
            Some(profile) => self.execute_graph_unprofiled(iter, |task: I::Item| {
                profile.measure(ProfileSpanKind::BatchTask, self.current_worker_index(), task.into_u64(), || f(task))
            }),
        }
    }

    fn execute_graph_unprofiled<I: GraphIter, F: Sync + Fn(I::Item)>(self, iter: I, f: F) {
        let Some(pool) = self.pool() else {
            execute_graph_inline(iter, f);
            return;
        };

        struct BatchData<I, F> {
            iter: UnsafeCell<I>,
            f: F,
        }

        unsafe extern "C-unwind" fn ffi_next<I: GraphIter, F>(data: *const c_void, finished: FfiOption<u64>) -> FfiOption<u64> {
            unsafe {
                let data = &*(data as *const BatchData<I, F>);

                (&mut *data.iter.get())
                    .next(finished.into_option().map(I::Item::from_u64))
                    .map(TaskId::into_u64)
                    .into()
            }
        }

        unsafe extern "C-unwind" fn ffi_execute<I: GraphIter, F: Fn(I::Item)>(data: *const c_void, task: u64) {
            unsafe {
                let data = &*(data as *const BatchData<I, F>);

                (data.f)(I::Item::from_u64(task))
            }
        }

        let data = BatchData {
            iter: UnsafeCell::new(iter),
            f,
        };

        pool.execute_batch(FfiBatch {
            data: (&raw const data) as *const c_void,
            next_fn: ffi_next::<I, F>,
            execute_fn: ffi_execute::<I, F>,
        });
    }

    pub fn execute_work_stealing_ranges<F: Sync + Fn(Range<usize>)>(self, len: usize, f: F) {
        if len == 0 {
            return;
        }

        if self.is_single_threaded() {
            match self.profile() {
                None => f(0..len),
                Some(profile) => profile.measure(ProfileSpanKind::BatchTask, 0, 0, || f(0..len)),
            }
            return;
        }

        let chunks_count = chunks_count(len, self.parallelism());

        self.execute_graph(ChunksIter::new(chunks_count), |chunk| {
            f(chunk_range(len, chunks_count, chunk))
        });
    }

    pub fn execute_work_stealing_parallel<T: Send, F: Sync + Fn(&mut T)>(self, items: &mut [T], f: F) {
        let len = items.len();
        let items = SyncMutPtr(items.as_mut_ptr());

        // Chunk ranges are disjoint, so every element is borrowed mutably at most once.
        self.execute_work_stealing_ranges(len, |range| {
            for i in range {
                f(unsafe { &mut *items.get().add(i) });
            }
        });
    }

    pub fn execute_work_stealing_parallel_ref<T: Sync, F: Sync + Fn(&T)>(self, items: &[T], f: F) {
        self.execute_work_stealing_ranges(items.len(), |range| {
            for item in &items[range] {
                f(item);
            }
        });
    }

    pub fn push_job_unhandled<F: 'static + Send + FnOnce()>(self, f: F) {
        let f = self.profile_background_job(f);

        match self.pool() {
            Some(pool) => pool.push_background_job(FfiJob::new(f)),
            None => f(),
        }
    }

    pub fn push_job_handled<F: 'static + Send + FnOnce() -> T, T: 'static + Send>(self, f: F) -> JobHandle<T> {
        let (handle, executor) = job::create_job(self.profile_background_job(f));

        match self.pool() {
            Some(pool) => pool.push_background_job(FfiJob::new(executor)),
            None => executor(),
        }

        handle
    }

    pub fn scope<'env, F, R>(self, f: F) -> R
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> R,
    {
        scope::scope(self.pool(), self.profile(), f)
    }

    // the job may outlive the pushing code, so it owns the recorder and a pool handle for the worker index
    fn profile_background_job<F: 'static + Send + FnOnce() -> T, T>(self, f: F) -> impl 'static + Send + FnOnce() -> T {
        let profile = self.profile().map(|p| (p.to_owned(), self.pool().cloned()));

        move || match profile {
            None => f(),
            Some((profile, pool)) => {
                let thread = pool.as_ref().map_or(0, ThreadPool::current_worker_index);
                profile.measure(ProfileSpanKind::BackgroundJob, thread, 0, f)
            }
        }
    }

    fn pool(&self) -> Option<&'p ThreadPool> {
        self.pool.into_option()
    }
}

impl<'p> From<&'p ThreadPool> for ThreadPoolAccess<'p> {
    fn from(pool: &'p ThreadPool) -> Self {
        Self {
            pool: FfiOption::Some(pool),
            profile: FfiOption::None,
        }
    }
}

impl<'p> From<Option<&'p ThreadPool>> for ThreadPoolAccess<'p> {
    fn from(pool: Option<&'p ThreadPool>) -> Self {
        Self {
            pool: pool.into(),
            profile: FfiOption::None,
        }
    }
}

struct SyncMutPtr<T>(*mut T);

impl<T> SyncMutPtr<T> {
    fn get(&self) -> *mut T {
        self.0
    }
}

unsafe impl<T: Send> Sync for SyncMutPtr<T> {}

fn execute_graph_inline<I: GraphIter, F: Fn(I::Item)>(mut iter: I, f: F) {
    let mut ready = std::collections::VecDeque::new();

    while let Some(task) = iter.next(None) {
        ready.push_back(task);
    }

    while let Some(task) = ready.pop_front() {
        f(task);

        let mut finished = Some(task);

        while let Some(task) = iter.next(finished.take()) {
            ready.push_back(task);
        }
    }
}

//

pub struct Semaphore {
    mutex: Mutex<usize>,
    cvar: Condvar,
}

impl Semaphore {
    pub fn new(initial: usize) -> Self {
        Self {
            mutex: Mutex::new(initial),
            cvar: Condvar::new(),
        }
    }

    pub fn acquire(&self) {
        let mut count = self.mutex.lock().unwrap();

        while *count == 0 {
            count = self.cvar.wait(count).unwrap();
        }

        *count -= 1;
    }

    pub fn release(&self) {
        let is_overflow = {
            let mut count = self.mutex.lock().unwrap();

            if let Some(new_count) = count.checked_add(1) {
                *count = new_count;
                false
            } else {
                true
            }
        };

        if is_overflow {
            panic!("semaphore overflow");
        }

        self.cvar.notify_one();
    }
}
