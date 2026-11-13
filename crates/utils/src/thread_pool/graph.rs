use std::ops::Range;

pub trait TaskId: Copy + Send {
    fn into_u64(self) -> u64;
    fn from_u64(v: u64) -> Self;
}

macro_rules! task_id_impl {
    ($($t: ty),*) => {
        $(
            impl TaskId for $t {
                fn into_u64(self) -> u64 {
                    self as u64
                }

                fn from_u64(v: u64) -> Self {
                    v as $t
                }
            }
        )*
    };
}

task_id_impl!(u8, u16, u32, u64, usize);

pub trait GraphIter: Send {
    type Item: TaskId;

    fn next(&mut self, finished: Option<Self::Item>) -> Option<Self::Item>;
}

pub(super) struct ChunksIter {
    next: usize,
    count: usize,
}

impl ChunksIter {
    pub fn new(count: usize) -> Self {
        Self { next: 0, count }
    }
}

impl GraphIter for ChunksIter {
    type Item = usize;

    fn next(&mut self, _finished: Option<usize>) -> Option<usize> {
        if self.next >= self.count {
            return None;
        }

        self.next += 1;

        Some(self.next - 1)
    }
}

pub(super) const MAX_CHUNKS_COUNT: usize = 200;
pub(super) const CHUNKS_PER_THREAD: usize = 4;

pub(super) fn chunks_count(len: usize, parallelism: usize) -> usize {
    len.min((parallelism * CHUNKS_PER_THREAD).min(MAX_CHUNKS_COUNT))
}

pub(super) fn chunk_range(len: usize, chunks_count: usize, chunk: usize) -> Range<usize> {
    let bound = |c: usize| (c as u128 * len as u128 / chunks_count as u128) as usize;

    bound(chunk)..bound(chunk + 1)
}
