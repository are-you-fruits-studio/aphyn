use fruits_ffi::{FfiBox, FfiOption, FfiVec};

#[repr(C)]
pub struct CloseIntMap<T> {
    inner: FfiVec<FfiOption<FfiBox<T>>>,
    offset: u64,
}

impl<T> CloseIntMap<T> {
    pub const fn new() -> Self {
        Self {
            inner: FfiVec::new(),
            offset: 0,
        }
    }

    pub fn insert(&mut self, k: u64, v: T) {
        if self.inner.len() == 0 {
            self.inner.push(Some(FfiBox::new(v)).into());
            self.offset = k;
            return;
        }

        if k > self.offset {
            let idx = k - self.offset;

            if let Some(stored) = self.inner.get_mut(idx) {
                // todo: optimize
                *stored = Some(FfiBox::new(v)).into();
                return;
            } else {
                // todo: optimize
                while idx > self.inner.len() {
                    self.inner.push(None.into());
                }
                self.inner.push(Some(FfiBox::new(v)).into());
                return;
            }
        }

        let temp_offset = self.offset - k;

        self.offset = k;

        // todo: optimize
        let old_inner = std::mem::replace(&mut self.inner, FfiVec::new());
        self.inner = std::iter::once(FfiOption::Some(FfiBox::new(v)))
            .chain(std::iter::repeat_with(|| FfiOption::None).take((temp_offset - 1) as usize))
            .chain(old_inner.into_iter()).collect();
    }

    pub fn get(&self, k: u64) -> Option<&T> {
        if k < self.offset {
            return None;
        }

        let idx = k - self.offset;

        let stored = self.inner.get(idx)?;
       
        Some(&*stored.as_ref()?)
    }

    pub fn get_mut(&mut self, k: u64) -> Option<&mut T> {
        if k < self.offset {
            return None;
        }

        let idx = k - self.offset;

        let Some(stored) = self.inner.get_mut(idx) else {
            return None;
        };
       
        stored.as_mut().map(|b| &mut **b)
    }
}