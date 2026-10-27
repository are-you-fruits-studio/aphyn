//! # fruits_alloc_monitor
//!
//! Tracks how many bytes the process currently holds on the heap. Linking the
//! crate installs a global allocator that keeps a running total of live
//! allocations, which a profiling overlay or test can read at any time.
//!
//! # How to use
//!
//! #### Read the current heap usage
//!
//! Add the crate as a dependency and call [`allocated`] to get the number of
//! bytes currently allocated through the global allocator. Linking the crate is
//! enough to activate the monitor; no setup call is required.
//!
//! ```
//! let before = fruits_alloc_monitor::allocated();
//!
//! let data = vec![0_u8; 1024];
//!
//! let after = fruits_alloc_monitor::allocated();
//! assert!(after >= before);
//!
//! drop(data);
//! ```
//!
//! # How to maintain
//!
//! The crate registers `ALLOC`, a zero-sized [`GlobalAlloc`] implementation, as
//! the process `#[global_allocator]`. Every allocation and deallocation forwards
//! directly to the [`System`] allocator and adjusts the `ALLOCATED` counter by
//! `layout.size()`, so [`allocated`] returns the
//! sum of the sizes of all live allocations rather than a peak or a call count.
//!
//! The counter uses [`Ordering::Relaxed`]: the total is eventually consistent
//! across threads and is meant for monitoring, not for ordering memory
//! operations. Because there is one global counter, the figure covers the whole
//! process — including allocations made by dependencies — not a single
//! subsystem.
//!
//! Only the size requested in the [`Layout`](std::alloc::Layout) is tracked;
//! allocator overhead and alignment padding are not, so the value approximates
//! requested bytes rather than the resident set the OS reports.

use std::{
    alloc::{GlobalAlloc, System},
    sync::atomic::{AtomicUsize, Ordering},
};

#[global_allocator]
static ALLOC: AllocMonitor = AllocMonitor;
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

pub fn allocated() -> usize {
    ALLOCATED.load(Ordering::Relaxed)
}

struct AllocMonitor;

unsafe impl GlobalAlloc for AllocMonitor {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        // Safety. AllocMonitor is just a wrapper.
        // Without it all the allocator calls would be called on the System allocator.
        let result = unsafe { System.alloc(layout) };

        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);

        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        // Safety. AllocMonitor is just a wrapper.
        // Without it all the allocator calls would be called on the System allocator.
        let result = unsafe { System.dealloc(ptr, layout) };

        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);

        result
    }
}
