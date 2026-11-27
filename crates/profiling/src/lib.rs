//! # aphyn_profiling
//!
//! Optional CPU timing of engine work. A caller that wants to know what ran creates a [`ProfileRecorder`] and passes it
//! to the engine entry points that accept one: `aphyn_ecs::WorldBehavior::execute_iteration`,
//! `aphyn_app::run_frame_schedules` and `aphyn_render_features::RenderFeatureRegistry::render_view`. Each of them
//! records one [`ProfileSpan`] per system / render feature it runs, and the thread-pool work those start is recorded
//! as job spans. Passing `None` records nothing.
//!
//! # How to use
//!
//! Span times are nanoseconds since the recorder was created (or last [`reset`](ProfileRecorder::reset)), so spans of
//! consecutive frames share one time axis. [`take_spans`](ProfileRecorder::take_spans) hands over what was recorded
//! since the last take. [`measure`](ProfileRecorder::measure) times a closure and records it;
//! [`record`](ProfileRecorder::record) adds a span timed by the caller with [`now_ns`](ProfileRecorder::now_ns):
//!
//! ```
//! use aphyn_profiling::{ProfileRecorder, ProfileSpanKind};
//!
//! let recorder = ProfileRecorder::new();
//!
//! let sum = recorder.measure(ProfileSpanKind::System, "my_system", 0, 2, || (0..100u64).sum::<u64>());
//! assert_eq!(sum, 4950);
//!
//! let spans = recorder.take_spans();
//! assert_eq!(spans.len(), 1);
//! assert_eq!(spans[0].name.as_str(), "my_system");
//! assert!(spans[0].end_ns >= spans[0].start_ns);
//! assert!(recorder.take_spans().is_empty());
//! ```
//!
//! A [`ProfileSpan`] carries its [`kind`](ProfileSpanKind), name, `thread` (the thread-pool index from
//! `aphyn_utils::ThreadPoolAccess::current_worker_index`: 0 is a thread outside the pool, such as the one that
//! called into it, `1..=n` the frame workers, `n+1..` the background workers) and a kind-specific `tag`: the
//! `aphyn_ecs::Schedule` index for systems, the pass order for render passes, the task id for batch tasks, 0
//! otherwise. Job spans are named after the span that started them (the system or render feature).
//!
//! A [`ProfileCtx`] is what the thread pool carries to record jobs: the recorder plus the name of the parent span.
//! [`OwnedProfileCtx`] is its `'static` form for background jobs, which may outlive the code that pushed them.
//! [`ProfileRecorder`] is a cheap reference-counted handle (`Clone`), and spans recorded through any clone land in
//! the same recorder:
//!
//! ```
//! use aphyn_profiling::{ProfileCtx, ProfileRecorder, ProfileSpanKind};
//!
//! let recorder = ProfileRecorder::new();
//! let ctx = ProfileCtx { recorder: &recorder, parent: "my_system".into() };
//!
//! let owned = ctx.to_owned();
//! std::thread::spawn(move || owned.measure(ProfileSpanKind::BackgroundJob, 0, 0, || {})).join().unwrap();
//! ctx.measure(ProfileSpanKind::BatchTask, 0, 3, || {});
//!
//! let spans = recorder.take_spans();
//! assert_eq!(spans.len(), 2);
//! assert!(spans.iter().all(|s| s.name.as_str() == "my_system"));
//! ```
//!
//! # How to maintain
//!
//! [`ProfileRecorder`] crosses the dynamic-library boundary (systems of a game library are executed by the host's
//! scheduler with the host's recorder, and the pool carries it into the library's jobs), so it follows the
//! `ThreadPool` pattern: `#[repr(C)]` [`FfiArc`] around an [`FfiDroppable`] native state and `extern "C-unwind"`
//! function pointers, and every operation goes through those pointers into the code of the side that created it.
//! Its layout, [`ProfileSpan`], [`ProfileSpanKind`] and [`ProfileCtx`] must change in lockstep on both sides.
//!
//! Times are taken outside the lock; the mutex only guards the push. Profiling is opt-in per call: callers branch on
//! an `Option` and do nothing else when it is `None`, so the standalone app pays one branch per system, render
//! feature and pool batch. Each recorded span clones its name, which allocates; that cost exists only while
//! profiling.

use std::{
    ffi::c_void,
    sync::Mutex,
    time::{Duration, Instant},
};

use aphyn_ffi::{FfiArc, FfiDroppable, FfiStrSliceRef, FfiString, FfiVec};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProfileSpanKind {
    System = 0,
    RenderCollect = 1,
    RenderPass = 2,
    BatchTask = 3,
    ScopeJob = 4,
    BackgroundJob = 5,
}

impl ProfileSpanKind {
    pub const COUNT: usize = 6;

    pub fn is_job(self) -> bool {
        matches!(self, Self::BatchTask | Self::ScopeJob | Self::BackgroundJob)
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct ProfileSpan {
    pub kind: ProfileSpanKind,
    pub name: FfiString,
    pub thread: u32,
    pub tag: u64,
    pub start_ns: u64,
    pub end_ns: u64,
}

impl ProfileSpan {
    pub fn duration_ns(&self) -> u64 {
        self.end_ns.saturating_sub(self.start_ns)
    }
}

struct RecorderNative {
    state: Mutex<RecorderState>,
}

struct RecorderState {
    origin: Instant,
    spans: Vec<ProfileSpan>,
}

impl RecorderNative {
    fn lock(&self) -> std::sync::MutexGuard<'_, RecorderState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[repr(C)]
struct RecorderInner {
    native: FfiDroppable,
    now_ns_fn: unsafe extern "C-unwind" fn(*const c_void) -> u64,
    record_fn: unsafe extern "C-unwind" fn(*const c_void, ProfileSpan),
    reset_fn: unsafe extern "C-unwind" fn(*const c_void),
    take_spans_fn: unsafe extern "C-unwind" fn(*const c_void) -> FfiVec<ProfileSpan>,
}

#[repr(C)]
#[derive(Clone)]
pub struct ProfileRecorder {
    inner: FfiArc<RecorderInner>,
}

impl ProfileRecorder {
    pub fn new() -> Self {
        unsafe fn native<'a>(this: *const c_void) -> &'a RecorderNative {
            unsafe { &*(this as *const RecorderNative) }
        }

        unsafe extern "C-unwind" fn ffi_now_ns(this: *const c_void) -> u64 {
            let origin = unsafe { native(this) }.lock().origin;
            duration_ns(origin.elapsed())
        }

        unsafe extern "C-unwind" fn ffi_record(this: *const c_void, span: ProfileSpan) {
            unsafe { native(this) }.lock().spans.push(span);
        }

        unsafe extern "C-unwind" fn ffi_reset(this: *const c_void) {
            let mut state = unsafe { native(this) }.lock();
            state.origin = Instant::now();
            state.spans.clear();
        }

        unsafe extern "C-unwind" fn ffi_take_spans(this: *const c_void) -> FfiVec<ProfileSpan> {
            FfiVec::from_vec(std::mem::take(&mut unsafe { native(this) }.lock().spans))
        }

        Self {
            inner: FfiArc::new(RecorderInner {
                native: FfiDroppable::new(RecorderNative {
                    state: Mutex::new(RecorderState {
                        origin: Instant::now(),
                        spans: Vec::new(),
                    }),
                }),
                now_ns_fn: ffi_now_ns,
                record_fn: ffi_record,
                reset_fn: ffi_reset,
                take_spans_fn: ffi_take_spans,
            }),
        }
    }

    pub fn now_ns(&self) -> u64 {
        unsafe { (self.inner.now_ns_fn)(self.inner.native.get()) }
    }

    pub fn record(&self, span: ProfileSpan) {
        unsafe { (self.inner.record_fn)(self.inner.native.get(), span) }
    }

    pub fn reset(&self) {
        unsafe { (self.inner.reset_fn)(self.inner.native.get()) }
    }

    pub fn take_spans(&self) -> FfiVec<ProfileSpan> {
        unsafe { (self.inner.take_spans_fn)(self.inner.native.get()) }
    }

    pub fn measure<R>(&self, kind: ProfileSpanKind, name: &str, thread: u32, tag: u64, f: impl FnOnce() -> R) -> R {
        self.measure_named(kind, || name.into(), thread, tag, f)
    }

    fn measure_named<R>(
        &self,
        kind: ProfileSpanKind,
        name: impl FnOnce() -> FfiString,
        thread: u32,
        tag: u64,
        f: impl FnOnce() -> R,
    ) -> R {
        let start_ns = self.now_ns();
        let result = f();
        let end_ns = self.now_ns();

        self.record(ProfileSpan {
            kind,
            name: name(),
            thread,
            tag,
            start_ns,
            end_ns,
        });

        result
    }
}

impl Default for ProfileRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// Safety. The native state is a mutex-guarded `Send` value behind an atomically counted pointer.
unsafe impl Send for ProfileRecorder {}
unsafe impl Sync for ProfileRecorder {}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ProfileCtx<'a> {
    pub recorder: &'a ProfileRecorder,
    pub parent: FfiStrSliceRef<'a>,
}

impl<'a> ProfileCtx<'a> {
    pub fn measure<R>(&self, kind: ProfileSpanKind, thread: u32, tag: u64, f: impl FnOnce() -> R) -> R {
        self.recorder.measure_named(kind, || self.parent.into_slice().into(), thread, tag, f)
    }

    pub fn to_owned(&self) -> OwnedProfileCtx {
        OwnedProfileCtx {
            recorder: self.recorder.clone(),
            parent: self.parent.into_slice().into(),
        }
    }

    // Safety: the recorder and the name must outlive every use of the returned context.
    pub unsafe fn extend_lifetime<'l>(self) -> ProfileCtx<'l> {
        unsafe { std::mem::transmute::<ProfileCtx<'a>, ProfileCtx<'l>>(self) }
    }
}

#[derive(Clone)]
pub struct OwnedProfileCtx {
    pub recorder: ProfileRecorder,
    pub parent: FfiString,
}

impl OwnedProfileCtx {
    pub fn measure<R>(&self, kind: ProfileSpanKind, thread: u32, tag: u64, f: impl FnOnce() -> R) -> R {
        self.recorder.measure_named(kind, || self.parent.clone(), thread, tag, f)
    }
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_from_threads_are_collected() {
        let recorder = ProfileRecorder::new();

        std::thread::scope(|s| {
            for thread in 0..4 {
                let recorder = &recorder;
                s.spawn(move || recorder.measure(ProfileSpanKind::System, "s", thread, 0, || {}));
            }
        });

        let mut threads: Vec<u32> = recorder.take_spans().iter().map(|s| s.thread).collect();
        threads.sort_unstable();
        assert_eq!(threads, [0, 1, 2, 3]);
    }

    #[test]
    fn reset_clears_and_moves_origin() {
        let recorder = ProfileRecorder::new();
        std::thread::sleep(Duration::from_millis(5));
        recorder.measure(ProfileSpanKind::RenderPass, "p", 0, 7, || {});
        recorder.reset();

        assert!(recorder.take_spans().is_empty());
        assert!(recorder.now_ns() < 5_000_000);
    }

    #[test]
    fn clones_share_spans() {
        let recorder = ProfileRecorder::new();
        recorder.clone().measure(ProfileSpanKind::ScopeJob, "j", 0, 0, || {});
        assert_eq!(recorder.take_spans().len(), 1);
    }
}
