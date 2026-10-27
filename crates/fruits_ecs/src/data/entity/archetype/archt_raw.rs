use std::alloc::Layout;

use fruits_ffi::{FfiAllocator, FfiVec};

pub const CHUNK_SIZE: u64 = 1024 * 12;
pub const CHUNK_ALIGN: u64 = 64;

pub const CHUNK_LAYOUT: Layout = {
    let Ok(layout) = Layout::from_size_align(CHUNK_SIZE as usize, CHUNK_ALIGN as usize) else {
        panic!("invalid chunk layout")
    };
    layout
};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ArchetypeItemPhysicalLocation {
    pub chunk_index: u64,
    pub memory_offset: u64,
}

#[repr(C)]
pub struct ArchetypeRaw {
    chunks: FfiVec<*mut u8>,
    alloc: FfiAllocator,
}

unsafe impl Send for ArchetypeRaw {}
unsafe impl Sync for ArchetypeRaw {}

impl ArchetypeRaw {
    pub fn new() -> Self {
        Self {
            chunks: FfiVec::new(),
            alloc: FfiAllocator::from_global(),
        }
    }

    pub fn get_chunk(&self, chunk_index: u64) -> *mut u8 {
        self.chunks[chunk_index]
    }

    pub fn get_memory(&self, location: &ArchetypeItemPhysicalLocation) -> *mut u8 {
        let chunk = &self.chunks[location.chunk_index];

        // Safety. Safe, because of the overflow check and all the offsets are precalculated.
        unsafe {
            if location.memory_offset >= CHUNK_SIZE {
                panic!("fruits: memory out of bounds access");
            }

            chunk.add(location.memory_offset as usize)
        }
    }

    pub fn chunks_count(&self) -> u64 {
        self.chunks.len()
    }

    pub fn push_chunk(&mut self) {
        self.chunks.push(unsafe { self.alloc.alloc(CHUNK_SIZE, CHUNK_ALIGN) });
    }

    pub fn pop_chunk(&mut self) {
        if let Some(ptr) = self.chunks.pop() {
            unsafe { self.alloc.dealloc(ptr, CHUNK_SIZE, CHUNK_ALIGN) };
        }
    }
}

impl Drop for ArchetypeRaw {
    fn drop(&mut self) {
        // Safety. Unsafe for access - missing lifetimes. But leak-safe.
        unsafe {
            for &ptr in &self.chunks {
                self.alloc.dealloc(ptr, CHUNK_SIZE, CHUNK_ALIGN);
            }
        }
    }
}

impl Default for ArchetypeRaw {
    fn default() -> Self {
        Self::new()
    }
}
