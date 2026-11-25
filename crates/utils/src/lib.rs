//! # aphyn_utils
//!
//! Foundational data structures and low-level helpers shared across the engine.
//! It is the low-level toolbox the other crates reach for: slot maps with
//! stable handles, fixed-capacity containers, a thread pool, a type-keyed store,
//! graph and tree helpers, and a handful of memory utilities.
//!
//! # How to use
//!
//! Each utility lives in its own module and is used on its own; there is no
//! registration step. Import the type you need directly.
//!
//! #### Fixed-capacity stack vector
//!
//! [`stack_vec::StackVec`] stores up to `C` elements inline, with no heap
//! allocation. [`push`](stack_vec::StackVec::push) returns the value back on
//! overflow instead of growing.
//!
//! ```
//! use aphyn_utils::stack_vec::StackVec;
//!
//! let mut v = StackVec::<i32, 4>::new();
//! v.push(1).unwrap();
//! v.push(2).unwrap();
//!
//! assert_eq!(v.len(), 2);
//! assert_eq!(v[0], 1);
//! assert_eq!(v.as_slice(), &[1, 2]);
//! ```
//!
//! #### Type-keyed storage
//!
//! [`typed_map::TypedMap`] keeps at most one value per concrete type, keyed by
//! [`TypeId`](std::any::TypeId). Inserting a second value of the same type fails.
//!
//! ```
//! use aphyn_utils::typed_map::TypedMap;
//!
//! let mut map: TypedMap = TypedMap::new();
//! map.insert(42_i32).unwrap();
//! map.insert("hello").unwrap();
//!
//! assert_eq!(map.get_ref::<i32>(), Some(&42));
//! assert!(map.contains::<&str>());
//! assert!(map.insert(7_i32).is_err());
//! ```
//!
//! #### Stable handles into a slot map
//!
//! [`index_version_collection::VersionCollection`] hands out a
//! [`VersionIndex`](index_version_collection::VersionIndex) for each inserted
//! value. Removing a value frees its slot for reuse, but bumps the slot's
//! version so the old handle stops resolving.
//!
//! ```
//! use aphyn_utils::index_version_collection::VersionCollection;
//!
//! let mut col = VersionCollection::new();
//! let a = col.insert("a");
//!
//! assert_eq!(col.get(a), Some(&"a"));
//! assert_eq!(col.remove(a), Some("a"));
//!
//! // The stale handle no longer resolves, even after the slot is reused.
//! let _c = col.insert("c");
//! assert_eq!(col.get(a), None);
//! ```
//!
//! #### Change tracking
//!
//! [`versioned::Versioned`] wraps a value and counts mutable accesses, so a
//! consumer can detect when the value changed. Any mutable deref bumps the
//! version; read [`Versioned::version`](versioned::Versioned::version) to observe it.
//!
//! ```
//! use aphyn_utils::versioned::Versioned;
//!
//! let mut v = Versioned::new(10);
//! assert_eq!(Versioned::version(&v), 0);
//!
//! *v += 5;
//! assert_eq!(*v, 15);
//! assert_eq!(Versioned::version(&v), 1);
//! ```
//!
//! #### Parallel work on a thread pool
//!
//! [`thread_pool::ThreadPool`] owns two sets of threads: frame workers that run
//! blocking parallel work, and background threads for long-running jobs. Work is
//! never submitted to the pool directly but through a
//! [`ThreadPoolAccess`](thread_pool::ThreadPoolAccess), a `Copy` handle that is
//! either backed by a pool or [single-threaded](thread_pool::ThreadPoolAccess::single_threaded).
//! The same code then runs in parallel or inline on the calling thread.
//!
//! [`execute_work_stealing_parallel`](thread_pool::ThreadPoolAccess::execute_work_stealing_parallel)
//! calls a closure for each element of a slice and returns once all are done. The
//! slice is split into at least one chunk per thread (the caller included) and at
//! most 200 chunks; idle threads take the next chunk from a shared queue.
//!
//! ```
//! use aphyn_utils::thread_pool::{ThreadPool, ThreadPoolAccess};
//! use std::sync::atomic::{AtomicU64, Ordering};
//!
//! fn double_and_sum(access: ThreadPoolAccess, items: &mut [u64]) -> u64 {
//!     let sum = AtomicU64::new(0);
//!
//!     access.execute_work_stealing_parallel(items, |item| {
//!         *item *= 2;
//!         sum.fetch_add(*item, Ordering::Relaxed);
//!     });
//!
//!     sum.into_inner()
//! }
//!
//! let pool = ThreadPool::new(3, 1);
//! let mut items: Vec<u64> = (0..1000).collect();
//!
//! assert_eq!(double_and_sum(pool.access(), &mut items), 999_000);
//! assert_eq!(double_and_sum(ThreadPoolAccess::single_threaded(), &mut items), 1_998_000);
//! ```
//!
//! [`execute_graph`](thread_pool::ThreadPoolAccess::execute_graph) runs tasks whose
//! readiness depends on finished tasks, described by a [`GraphIter`](thread_pool::GraphIter).
//! The pool first calls `next(None)` until it returns `None`; after a task
//! finishes it calls `next(Some(task))` once and then `next(None)` until `None`.
//! The call returns when no task is running and `next` has nothing more to give.
//! A task may itself call back into the same access, for example to run a slice
//! in parallel.
//!
//! ```
//! use aphyn_utils::thread_pool::{GraphIter, ThreadPool};
//! use std::sync::Mutex;
//!
//! // 0 -> {1, 2} -> 3
//! struct Diamond {
//!     started: [bool; 4],
//!     finished: [bool; 4],
//! }
//!
//! impl GraphIter for Diamond {
//!     type Item = u32;
//!
//!     fn next(&mut self, finished: Option<u32>) -> Option<u32> {
//!         if let Some(f) = finished {
//!             self.finished[f as usize] = true;
//!         }
//!
//!         let deps: [&[usize]; 4] = [&[], &[0], &[0], &[1, 2]];
//!         let ready = (0..4).find(|&t| !self.started[t] && deps[t].iter().all(|&d| self.finished[d]))?;
//!         self.started[ready] = true;
//!
//!         Some(ready as u32)
//!     }
//! }
//!
//! let pool = ThreadPool::new(3, 1);
//! let order = Mutex::new(Vec::new());
//!
//! pool.access().execute_graph(Diamond { started: [false; 4], finished: [false; 4] }, |task| {
//!     let mut inner = vec![1_u32; 64];
//!     pool.access().execute_work_stealing_parallel(&mut inner, |x| *x += task);
//!
//!     order.lock().unwrap().push(task);
//! });
//!
//! let order = order.into_inner().unwrap();
//! assert_eq!(order.len(), 4);
//! assert_eq!(order[0], 0);
//! assert_eq!(order[3], 3);
//! ```
//!
//! A panic in a task stops the batch from handing out new tasks; once the running
//! ones finish, the panic is re-raised on the calling thread and the pool stays usable.
//!
//! ```
//! use aphyn_utils::thread_pool::ThreadPool;
//!
//! let pool = ThreadPool::new(3, 1);
//! let items = [1, 2, 3, 4];
//!
//! let result = std::panic::catch_unwind(|| {
//!     pool.access().execute_work_stealing_parallel_ref(&items, |&x| assert_ne!(x, 3));
//! });
//! assert!(result.is_err());
//!
//! pool.access().execute_work_stealing_parallel_ref(&items, |&x| assert!(x > 0));
//! ```
//!
//! [`current_worker_index`](thread_pool::ThreadPoolAccess::current_worker_index) tells which thread a task runs on,
//! stable across frames: `1..=threads_count()` on the pool's frame workers, `threads_count() + 1..` on its background
//! workers, and 0 on any other thread (the caller that takes part in a batch, another pool's worker) and without a
//! pool. Profilers use it as a lane id.
//!
//! ```
//! use aphyn_utils::thread_pool::{ThreadPool, ThreadPoolAccess};
//! use std::sync::Mutex;
//!
//! let pool = ThreadPool::new(3, 1);
//! let seen = Mutex::new(Vec::new());
//!
//! pool.access()
//!     .execute_work_stealing_ranges(64, |_| seen.lock().unwrap().push(pool.access().current_worker_index()));
//!
//! assert!(seen.into_inner().unwrap().iter().all(|&i| i <= 3));
//! assert_eq!(pool.access().current_worker_index(), 0);
//! assert_eq!(ThreadPoolAccess::single_threaded().current_worker_index(), 0);
//!
//! let job_pool = pool.clone();
//! assert_eq!(pool.access().push_job_handled(move || job_pool.current_worker_index()).block_and_take(), 4);
//! ```
//!
//! #### Profiling jobs
//!
//! [`with_profile`](thread_pool::ThreadPoolAccess::with_profile) attaches an `aphyn_profiling::ProfileCtx` (a
//! recorder plus the parent span's name) to an access. Work started through such an access is recorded: one
//! `BatchTask` span per graph task / work-stealing chunk (tagged with the task id), one `ScopeJob` span per scope job
//! and one `BackgroundJob` span per background job, each named after the parent and carrying the worker index. The
//! ECS scheduler and `render_view` attach it to the access they give each system and render feature while profiling.
//!
//! ```
//! use aphyn_ffi::FfiString;
//! use aphyn_profiling::{ProfileCtx, ProfileRecorder, ProfileSpanKind};
//! use aphyn_utils::thread_pool::ThreadPool;
//!
//! let pool = ThreadPool::new(3, 1);
//! let recorder = ProfileRecorder::new();
//! let parent = FfiString::from("my_system");
//! let access = pool.access().with_profile(Some(ProfileCtx { recorder: &recorder, parent: &parent }));
//!
//! access.execute_work_stealing_ranges(1000, |_| {});
//! access.scope(|scope| scope.push_job_unhandled(|| {}));
//! access.push_job_handled(|| {}).block_and_take();
//!
//! let spans = recorder.take_spans();
//! let count = |kind| spans.iter().filter(|s| s.kind == kind).count();
//! assert!(count(ProfileSpanKind::BatchTask) > 1);
//! assert_eq!(count(ProfileSpanKind::ScopeJob), 1);
//! assert_eq!(count(ProfileSpanKind::BackgroundJob), 1);
//! assert!(spans.iter().all(|s| s.name.as_str() == "my_system"));
//! ```
//!
//! [`push_job_handled`](thread_pool::ThreadPoolAccess::push_job_handled) runs a
//! `'static` job on a background thread and returns a
//! [`JobHandle`](thread_pool::JobHandle) to poll (`try_take`) or wait for
//! (`block_and_take`), possibly frames later.
//! [`push_job_unhandled`](thread_pool::ThreadPoolAccess::push_job_unhandled) is the
//! fire-and-forget form. Without a pool both run the job immediately on the caller.
//!
//! ```
//! use aphyn_utils::thread_pool::{ThreadPool, ThreadPoolAccess};
//!
//! let pool = ThreadPool::new(1, 1);
//!
//! let handle = pool.access().push_job_handled(|| (1..=10).sum::<u32>());
//! assert_eq!(handle.block_and_take(), 55);
//!
//! let caller = std::thread::current().id();
//! let handle = ThreadPoolAccess::single_threaded().push_job_handled(move || std::thread::current().id() == caller);
//! assert!(handle.is_finished());
//! assert!(handle.block_and_take());
//! ```
//!
//! [`scope`](thread_pool::ThreadPoolAccess::scope) borrows the surrounding stack
//! and blocks until every job pushed inside it has finished; while waiting, the
//! caller runs pending jobs of that scope itself.
//!
//! ```
//! use aphyn_utils::thread_pool::ThreadPool;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! let pool = ThreadPool::new(4, 1);
//! let counter = AtomicUsize::new(0);
//!
//! pool.access().scope(|scope| {
//!     for _ in 0..8 {
//!         scope.push_job_unhandled(|| {
//!             counter.fetch_add(1, Ordering::Relaxed);
//!         });
//!     }
//! });
//!
//! assert_eq!(counter.load(Ordering::Relaxed), 8);
//! ```
//!
//! #### Topological ordering
//!
//! [`graph::Graph`] records directed edges and orders the nodes so that every
//! edge points forward. [`to_vec`](graph::Graph::to_vec) returns `Err` with the
//! offending node set when a cycle is found.
//!
//! ```
//! use aphyn_utils::graph::Graph;
//!
//! let mut graph = Graph::new();
//! graph.insert_edge("compile", "link");
//! graph.insert_edge("link", "run");
//!
//! let order = graph.to_vec().unwrap();
//! let pos = |n| order.iter().position(|x| *x == n).unwrap();
//!
//! assert!(pos("compile") < pos("link"));
//! assert!(pos("link") < pos("run"));
//! ```
//!
//! #### Sparse map over near-contiguous integer keys
//!
//! [`close_int_map::CloseIntMap`] stores values against `usize` keys that
//! cluster in a narrow range, backing them with a single offset-based `Vec`.
//!
//! ```
//! use aphyn_utils::close_int_map::CloseIntMap;
//!
//! let mut map = CloseIntMap::new();
//! map.insert(100, "a");
//! map.insert(102, "b");
//!
//! assert_eq!(map.get(100), Some(&"a"));
//! assert_eq!(map.get(102), Some(&"b"));
//! assert_eq!(map.get(101), None);
//! ```
//!
//! #### Building a forest from parent-child pairs
//!
//! [`tree::TreeBuilder`] collects parent-child pairs (and standalone nodes) in any
//! order and [`build`](tree::TreeBuilder::build)s them into one
//! [`TreeNode`](tree::TreeNode) per root.
//!
//! ```
//! use aphyn_utils::tree::TreeBuilder;
//!
//! let mut builder = TreeBuilder::new();
//! builder.insert_pair("root", "child");
//! builder.insert_pair("child", "grandchild");
//!
//! let roots = builder.build();
//! assert_eq!(roots.len(), 1);
//! assert_eq!(roots[0].value, "root");
//! assert_eq!(roots[0].children[0].children[0].value, "grandchild");
//!
//! let mut builder = TreeBuilder::new();
//! builder.insert_single("root");
//! builder.insert_pair("child", "grandchild");
//! builder.insert_pair("root", "child");
//!
//! let roots = builder.build();
//! assert_eq!(roots.len(), 1);
//! assert_eq!(roots[0].children[0].children[0].value, "grandchild");
//! ```
//!
//! #### Reinterpreting a buffer's element type
//!
//! [`morph_vec::MorphVec`] is a growable vector whose backing allocation can be
//! handed to a vector of a different element type, avoiding a reallocation.
//!
//! ```no_run
//! use aphyn_utils::morph_vec::MorphVec;
//!
//! let mut bytes: MorphVec<u8> = MorphVec::new();
//! bytes.push(1);
//! bytes.push(2);
//! assert_eq!(&*bytes, &[1, 2]);
//!
//! // Reuse the same allocation for a different element type.
//! let mut floats: MorphVec<f32> = bytes.morph_into();
//! assert!(floats.is_empty());
//! floats.push(1.5);
//! assert_eq!(&*floats, &[1.5]);
//! ```
//!
//! #### Viewing a value as raw bytes
//!
//! [`mem::as_bytes`] exposes the bytes of any type that opts in via the
//! [`AllBitsInit`](mem::AllBitsInit) marker, and [`mem::ReadOnly`] wraps a value
//! to hand out shared access without exposing a mutable path.
//!
//! ```
//! use aphyn_utils::mem::{as_bytes, ReadOnly};
//!
//! let value: u32 = 0x0403_0201;
//! assert_eq!(as_bytes(&value).len(), 4);
//!
//! let ro = ReadOnly::new(5);
//! assert_eq!(*ro, 5);
//! ```
//!
//! # How to maintain
//!
//! The crate's only dependency is the workspace crate `aphyn_ffi`, whose
//! stable-layout containers back the `#[repr(C)]`
//! [`VersionCollection`](index_version_collection::VersionCollection) and
//! [`CloseIntMap`](close_int_map::CloseIntMap); otherwise it is a flat
//! collection of independent modules. Adding a utility means adding a module;
//! the modules do not depend on each other except where noted below. Almost
//! every public method is a thin, self-evident accessor, which is why the
//! per-item surface carries no doc comments: the names and signatures are the
//! documentation, and the design notes that are *not* obvious from a signature
//! live here.
//!
//! #### Versioned handles
//!
//! [`VersionCollection`](index_version_collection::VersionCollection) is a slot
//! map. Each slot carries a `version`; a
//! [`VersionIndex`](index_version_collection::VersionIndex) pairs a slot index
//! with the version it was issued under. Version `0` is reserved as the "empty"
//! sentinel ([`VersionIndex::EMPTY`](index_version_collection::VersionIndex::EMPTY)),
//! so fresh slots start at version `1`. `remove` wraps the version forward
//! (skipping back over `0`) and pushes the slot onto `free_places`; a later
//! `insert` reuses that slot, which is why a handle held across a remove/insert
//! cycle reads as absent rather than silently aliasing the new occupant.
//!
//! [`Versioned`](versioned::Versioned) tracks mutation by bumping its counter in
//! `DerefMut`, so any `&mut` access — even one that writes nothing — advances the
//! version. [`Versioned::version`](versioned::Versioned::version) and
//! [`get_silent`](versioned::Versioned::get_silent) (a raw pointer that skips the
//! bump) are associated functions, not methods, to avoid being shadowed by the
//! inner type's own methods reachable through `Deref`.
//!
//! #### Thread pool
//!
//! [`ThreadPool`](thread_pool::ThreadPool) crosses the dynamic-library boundary, so
//! it is a `#[repr(C)]` [`FfiArc`](aphyn_ffi::FfiArc) around a native pool
//! (`ThreadPoolNative`, an [`FfiDroppable`](aphyn_ffi::FfiDroppable)) plus
//! `extern "C-unwind"` function pointers. All scheduling code lives in the binary
//! that created the pool; the other side only calls through the pointers.
//! Nothing generic crosses: a parallel call reaches the pool as one
//! [`FfiBatch`](thread_pool::FfiBatch) (data pointer plus `next` / `execute`
//! trampolines, tasks erased to `u64` through [`TaskId`](thread_pool::TaskId)),
//! and owned closures as [`FfiJob`](thread_pool::FfiJob)s. So the vtable is called
//! once per batch, and only the trampolines are called per task.
//! [`ThreadPoolAccess`](thread_pool::ThreadPoolAccess) is a `#[repr(C)]` nullable
//! reference; its single-threaded paths are plain generic code.
//!
//! The native pool has one frame queue (`Mutex` + `Condvar`, no spinning) of batch
//! tasks and frame jobs (used by `scope`), and a separate background queue served
//! by its own threads, so multi-frame jobs never occupy frame workers. A batch's
//! state (`BatchState`: iterator lock, `running` / `queued` counters guarded by the
//! frame mutex, `poisoned` flag, panic payload, caller `Condvar`) lives on the
//! caller's stack. That is sound because `execute_batch` returns only when both
//! counters are zero, and a worker never touches a batch after the `publish` that
//! released its last slot. `next` is only called under the batch's iterator lock
//! (lock order: iterator lock, then frame mutex), so a `GraphIter` needs `Send`,
//! not `Sync`.
//!
//! Each worker stores `(address of its pool's shared state, 1-based index)` in a thread-local at spawn, frame
//! workers first, then background workers; `current_worker_index` goes through the vtable so the thread-local of the
//! pool's own binary is read, and returns 0 when the stored address is not this pool's.
//!
//! `ThreadPoolAccess` carries an `FfiOption<ProfileCtx>` next to the pool, so its layout is part of the
//! dynamic-library ABI. Profiling wraps the user closure before it reaches the pool (`execute_graph` wraps the task
//! function, `Scope` and the background job paths wrap the job), so the native pool knows nothing about it. A
//! background job may outlive the code that pushed it, so it captures an `OwnedProfileCtx` (a recorder clone and the
//! parent name) and a `ThreadPool` clone for the worker index instead of the borrowed context.
//!
//! A thread that finishes a task pulls the newly ready tasks, keeps one for itself
//! and queues the rest. While waiting, a batch's caller runs only queued tasks of
//! its own batch and never those of other batches. Every caller can therefore
//! finish its batch alone, which makes nested batches (a system running a query in
//! parallel) deadlock-free. Task and `next` panics are caught, poison the batch
//! (queued tasks of it are dropped, `next` is not called again, so dependent
//! tasks never start) and are re-raised on the caller with `resume_unwind`.
//! Background job panics set the pool's `did_panic` flag (re-raised by
//! `panic_if_err` and on `Drop`), except handled jobs, which keep the panic in
//! their `JobHandle` and re-raise it on take. `JobHandle` follows the same
//! FFI pattern: an `FfiArc` of a native `Mutex` / `Condvar` slot plus trampolines.
//!
//! `scope` keeps its borrowed jobs in an `Arc`ed queue and pushes one `'static`
//! ticket per job to the frame queue; a ticket runs one queued job if any is
//! left. The scope runs pending jobs itself and waits until none is outstanding,
//! even when its body panics, which is what makes erasing the jobs' lifetimes
//! sound.
//!
//! #### Collections and memory helpers
//!
//! [`Graph`](graph::Graph) keeps both forward and backward adjacency maps so it
//! can walk dependencies in either direction; `to_vec` topologically sorts by
//! repeatedly extracting a node with no remaining predecessors and returns the
//! visited set as the `Err` cycle witness when none exists.
//! [`IntGraph`](graph::IntGraph) is a copy of the same algorithm specialized to
//! `u64` nodes; it is not used in the workspace yet and still contains a `dbg!`
//! in its sort loop that prints to stderr.
//!
//! [`TreeBuilder`](tree::TreeBuilder) keeps a root set plus a child list per node.
//! [`insert_pair`](tree::TreeBuilder::insert_pair) attaches the child under the
//! parent (adding either node if it is new) as long as the child is new or still a
//! root; it returns `false` when the child already has a parent or when the parent
//! is a descendant of the child (a cycle). A pair of equal nodes is inserted as a
//! single node.
//!
//! [`CloseIntMap`](close_int_map::CloseIntMap) stores an
//! `FfiVec<FfiOption<FfiBox<T>>>` plus a `u64` base `offset`; inserting a key below
//! the current offset re-bases the whole
//! vector (an O(n) prepend), so it pays off only when keys stay clustered. The
//! re-base and gap-filling paths are flagged `// todo: optimize`.
//!
//! [`TypedMap`](typed_map::TypedMap) is generic over a sealed
//! [`TypedMapStrategy`](typed_map::strategies::TypedMapStrategy) that selects the
//! stored value bound — `Box<dyn Any>`, `+ Send`, or `+ Send + Sync`. `insert` is
//! provided per strategy as separate inherent impls (each adding the matching
//! bound) and refuses to overwrite an existing entry.
//!
//! [`MorphVec`](morph_vec::MorphVec) manages its own allocation: it over-allocates
//! by `align_of::<T>() - 1` extra bytes and aligns the start pointer at runtime,
//! so the same byte buffer can back successive element types across
//! `morph_into`. Two rough edges to be aware of:
//! [`push`](morph_vec::MorphVec::push) computes the current capacity (an unsigned
//! subtraction) *before* the empty-buffer check, so the first push underflows and
//! panics in debug builds; and the index-based [`get`](morph_vec::MorphVec::get)
//! and [`get_mut`](morph_vec::MorphVec::get_mut) offset by `len` rather than the
//! requested index. Callers in the engine reach elements through the slice
//! `Deref` instead.
//!
//! The [`mem`] module gates its byte-cast helpers behind two unsafe marker traits:
//! [`AllBitsInit`](mem::AllBitsInit) (every bit pattern of the value is
//! initialized) and [`AllBitVariationsValid`](mem::AllBitVariationsValid) (every
//! bit pattern is also a *valid* value, required for the `_mut` casts). Both are
//! implemented for the primitive numeric types, slices, and arrays.
//!
//! #### Small building blocks
//!
//! [`AssumeSend`] unsafely asserts `Send` for a wrapped value;
//! [`ExecOnDrop`](exec_on_drop::ExecOnDrop) runs a closure when dropped;
//! [`Semaphore`](thread_pool::Semaphore) is a counting semaphore over a `Mutex` and
//! `Condvar`; [`SameType`](same_type::SameType) is a blanket-implemented identity
//! conversion used to bridge generic code that needs to treat `T` and an
//! associated type as interchangeable. The `bit_array` module
//! (`BitArray`) is currently private — not part of the public surface — and
//! `mem.rs` still carries a `// todo: BitVec`. The exported
//! [`try_default!`](crate::try_default!) macro is an experimental autoref-based
//! specialization that yields `Some(T::default())` for `Default` types and `None`
//! otherwise; it is not yet used anywhere in the workspace.

mod bit_array;
pub mod close_int_map;
pub mod exec_on_drop;
pub mod graph;
pub mod index_version_collection;
pub mod mem;
pub mod morph_vec;
pub mod same_type;
pub mod stack_vec;
pub mod thread_pool;
pub mod tree;
pub mod try_default;
pub mod typed_map;
pub mod versioned;

// todo
pub struct AssumeSend<T>(T);

impl<T> AssumeSend<T> {
    pub unsafe fn new(v: T) -> Self {
        Self(v)
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

unsafe impl<T> Send for AssumeSend<T> {}
