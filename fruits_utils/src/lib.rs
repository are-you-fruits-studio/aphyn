//! # fruits_utils
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
//! use fruits_utils::stack_vec::StackVec;
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
//! use fruits_utils::typed_map::TypedMap;
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
//! use fruits_utils::index_version_collection::VersionCollection;
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
//! use fruits_utils::versioned::Versioned;
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
//! [`thread_pool::ThreadPool`] runs jobs on a fixed set of worker threads.
//! [`scope`](thread_pool::ThreadPool::scope) borrows the surrounding stack and
//! blocks until every job spawned inside it has finished.
//!
//! ```
//! use fruits_utils::thread_pool::ThreadPool;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! let pool = ThreadPool::new(4);
//! let counter = AtomicUsize::new(0);
//!
//! pool.scope(|scope| {
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
//! use fruits_utils::graph::Graph;
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
//! use fruits_utils::close_int_map::CloseIntMap;
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
//! use fruits_utils::tree::TreeBuilder;
//!
//! let mut builder = TreeBuilder::new();
//! builder.insert_pair("root", "child");
//! builder.insert_pair("child", "grandchild");
//!
//! let roots = builder.build();
//! assert_eq!(roots.len(), 1);
//! assert_eq!(roots[0].value, "root");
//! assert_eq!(roots[0].children[0].children[0].value, "grandchild");
//! ```
//!
//! #### Reinterpreting a buffer's element type
//!
//! [`morph_vec::MorphVec`] is a growable vector whose backing allocation can be
//! handed to a vector of a different element type, avoiding a reallocation.
//!
//! ```no_run
//! use fruits_utils::morph_vec::MorphVec;
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
//! use fruits_utils::mem::{as_bytes, ReadOnly};
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
//! The crate's only dependency is the workspace crate `fruits_ffi`, whose
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
//! [`ThreadPool`](thread_pool::ThreadPool) shares a single `mpsc` receiver across
//! workers behind an `Arc<Mutex<…>>`; each worker locks, receives one `Message`,
//! and runs it. A panicking worker sets a shared `did_panic` flag (via an
//! [`ExecOnDrop`](exec_on_drop::ExecOnDrop) guard), which the pool re-checks on
//! `Drop` and re-raises as a panic. Jobs come in two forms: *unhandled* (fire and
//! forget) and *handled*, where a `JobState` drives a small atomic state machine
//! (`0` pending → `1` executing → `2` done → `3` result taken) over `UnsafeCell`
//! slots, advanced with `fetch_max` so each transition happens once. `scope`
//! tracks outstanding jobs with an `AtomicUsize` and blocks until it returns to
//! zero, which is what makes the lifetime `transmute` in
//! `Scope::push_job_unhandled` sound: no borrowed job can outlive the scope.
//! Several waits are busy-loops marked `// todo: CondVar`.
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
//! [`insert_pair`](tree::TreeBuilder::insert_pair) only accepts a pair that extends
//! the forest — a new parent above an existing root, a new child under an existing
//! node, or two new nodes — and returns `false` otherwise (for example when both
//! nodes already exist); a pair of equal nodes is inserted as a single node.
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
//! [`ExecOnDrop`](exec_on_drop::ExecOnDrop) runs a closure when dropped (used for
//! the thread-pool cleanup and scope counters);
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
pub mod exec_on_drop;
pub mod graph;
pub mod tree;
pub mod index_version_collection;
pub mod mem;
pub mod morph_vec;
pub mod same_type;
pub mod stack_vec;
pub mod thread_pool;
pub mod typed_map;
pub mod close_int_map;
pub mod try_default;
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
