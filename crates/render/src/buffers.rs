use aphyn_ffi::FfiVec;
use aphyn_render_core::{GpuUniformStorageBuffer, GpuVertexBuffer, RenderApi};
use aphyn_utils::mem::AllBitsInit;

pub use aphyn_render_core::{GpuVertexBufferLayout, GpuVertexBufferLayoutEntry, GpuVertexBufferLayoutEntryTy, GpuVertexStepMode};

//

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpuRuntimeTy {
    Float,
    Float2,
    Float3,
    Float4,
    Float2x2,
    Float2x3,
    Float2x4,
    Float3x2,
    Float3x3,
    Float3x4,
    Float4x2,
    Float4x3,
    Float4x4,
    UInt,
    UInt2,
    UInt3,
    UInt4,
    Int,
    Int2,
    Int3,
    Int4,
}

impl GpuRuntimeTy {
    pub fn gpu_size(&self) -> usize {
        match self {
            GpuRuntimeTy::Float | GpuRuntimeTy::UInt | GpuRuntimeTy::Int => 4,
            GpuRuntimeTy::Float2 | GpuRuntimeTy::UInt2 | GpuRuntimeTy::Int2 => 8,
            GpuRuntimeTy::Float3 | GpuRuntimeTy::UInt3 | GpuRuntimeTy::Int3 => 12,
            GpuRuntimeTy::Float4 | GpuRuntimeTy::UInt4 | GpuRuntimeTy::Int4 => 16,
            GpuRuntimeTy::Float2x2 => 16,
            GpuRuntimeTy::Float2x3 | GpuRuntimeTy::Float2x4 => 32,
            GpuRuntimeTy::Float3x2 => 24,
            GpuRuntimeTy::Float3x3 | GpuRuntimeTy::Float3x4 => 48,
            GpuRuntimeTy::Float4x2 => 32,
            GpuRuntimeTy::Float4x3 | GpuRuntimeTy::Float4x4 => 64,
        }
    }

    pub fn gpu_align(&self) -> usize {
        match self {
            GpuRuntimeTy::Float | GpuRuntimeTy::UInt | GpuRuntimeTy::Int => 4,
            GpuRuntimeTy::Float2 | GpuRuntimeTy::UInt2 | GpuRuntimeTy::Int2 => 8,
            GpuRuntimeTy::Float2x2 | GpuRuntimeTy::Float3x2 | GpuRuntimeTy::Float4x2 => 8,
            _ => 16,
        }
    }
}

pub trait GpuRuntimeValue {
    const TY: GpuRuntimeTy;

    fn write_gpu_bytes(&self, dst: &mut [u8]);
}

macro_rules! impl_gpu_runtime_value {
    ($($t: ty => $ty: ident),+ $(,)?) => {
        $(
            impl GpuRuntimeValue for $t {
                const TY: GpuRuntimeTy = GpuRuntimeTy::$ty;

                fn write_gpu_bytes(&self, dst: &mut [u8]) {
                    dst[..size_of::<$t>()].copy_from_slice(aphyn_utils::mem::as_bytes(self));
                }
            }
        )+
    };
}

impl_gpu_runtime_value! {
    f32 => Float, [f32; 2] => Float2, [f32; 3] => Float3, [f32; 4] => Float4,
    u32 => UInt, [u32; 2] => UInt2, [u32; 3] => UInt3, [u32; 4] => UInt4,
    i32 => Int, [i32; 2] => Int2, [i32; 3] => Int3, [i32; 4] => Int4,
}

// matrices are column-major like WGSL: `[[f32; R]; C]` is `matCxR<f32>`, columns padded to their vector alignment
macro_rules! impl_gpu_runtime_value_matrix {
    ($($c: literal x $r: literal => $ty: ident),+ $(,)?) => {
        $(
            impl GpuRuntimeValue for [[f32; $r]; $c] {
                const TY: GpuRuntimeTy = GpuRuntimeTy::$ty;

                fn write_gpu_bytes(&self, dst: &mut [u8]) {
                    let column_stride = Self::TY.gpu_size() / $c;

                    for (column, dst) in self.iter().zip(dst.chunks_exact_mut(column_stride)) {
                        column.write_gpu_bytes(dst);
                    }
                }
            }
        )+
    };
}

impl_gpu_runtime_value_matrix! {
    2 x 2 => Float2x2, 2 x 3 => Float2x3, 2 x 4 => Float2x4,
    3 x 2 => Float3x2, 3 x 3 => Float3x3, 3 x 4 => Float3x4,
    4 x 2 => Float4x2, 4 x 3 => Float4x3, 4 x 4 => Float4x4,
}

// todo: separate GpuUniformStorageBuffer into GpuUniformBuffer and GpuStorageBuffer and move it into render_core?
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TypedUniformBufferLayoutEntry {
    pub offset: usize,
    pub ty: GpuRuntimeTy,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedUniformBufferLayout {
    entries: FfiVec<TypedUniformBufferLayoutEntry>,
    total_size: usize,
    total_align: usize,
}

impl TypedUniformBufferLayout {
    pub fn new(layout: impl IntoIterator<Item = GpuRuntimeTy>) -> Self {
        let mut offset = 0_usize;
        let mut max_align = 1;
        let mut entries = FfiVec::new();

        for entry_ty in layout.into_iter() {
            let align = entry_ty.gpu_align();
            offset = offset.next_multiple_of(align);
            max_align = max_align.max(align);

            entries.push(TypedUniformBufferLayoutEntry { offset, ty: entry_ty });

            offset += entry_ty.gpu_size();
        }

        Self {
            entries,
            total_size: offset.next_multiple_of(max_align),
            total_align: max_align,
        }
    }

    pub fn entries(&self) -> &[TypedUniformBufferLayoutEntry] {
        &self.entries
    }

    pub fn total_size(&self) -> usize {
        self.total_size
    }

    pub fn total_align(&self) -> usize {
        self.total_align
    }

    fn set_in<T: GpuRuntimeValue>(&self, data: &mut [u8], location: usize, value: T) -> bool {
        let Some(entry) = self.entries.get(location as u64) else {
            return false;
        };

        if entry.ty != T::TY {
            return false;
        }

        value.write_gpu_bytes(&mut data[entry.offset..(entry.offset + entry.ty.gpu_size())]);

        true
    }
}

#[repr(C)]
pub struct TypedUniformBuffer {
    buffer: GpuUniformStorageBuffer,
    cache: FfiVec<u8>,
    layout: TypedUniformBufferLayout,
    is_dirty: bool,
}

impl TypedUniformBuffer {
    pub(crate) fn new(core: &RenderApi, layout: TypedUniformBufferLayout) -> Self {
        Self {
            buffer: core.create_uniform_storage_buffer(Some("typed uniform buffer"), layout.total_size),
            cache: vec![0_u8; layout.total_size].into(),
            layout,
            is_dirty: false,
        }
    }

    pub fn set<T: GpuRuntimeValue>(&mut self, location: usize, value: T) -> bool {
        let is_set = self.layout.set_in(&mut self.cache, location, value);
        self.is_dirty |= is_set;

        is_set
    }

    pub(crate) fn write_if_dirty(&mut self, core: &RenderApi) {
        if !self.is_dirty {
            return;
        }

        crate::write_buffer_bytes(core, &self.buffer, 0, &self.cache);
        self.is_dirty = false;
    }

    pub fn buffer(&self) -> &GpuUniformStorageBuffer {
        &self.buffer
    }

    pub fn layout(&self) -> &TypedUniformBufferLayout {
        &self.layout
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }
}

//

#[repr(C)]
pub struct TypedStorageBuffer {
    buffer: GpuUniformStorageBuffer,
    cache: FfiVec<u8>,
    layout: TypedUniformBufferLayout,
    len: usize,
    is_dirty: bool,
}

impl TypedStorageBuffer {
    pub(crate) fn new(core: &RenderApi, len: usize, layout: TypedUniformBufferLayout) -> Self {
        let size = len * layout.total_size;

        Self {
            buffer: core.create_uniform_storage_buffer(Some("typed storage buffer"), size),
            cache: vec![0_u8; size].into(),
            layout,
            len,
            is_dirty: false,
        }
    }

    pub fn set<T: GpuRuntimeValue>(&mut self, idx: usize, location: usize, value: T) -> bool {
        if idx >= self.len {
            return false;
        }

        let stride = self.layout.total_size;
        let element = &mut (*self.cache)[(idx * stride)..((idx + 1) * stride)];

        let is_set = self.layout.set_in(element, location, value);
        self.is_dirty |= is_set;

        is_set
    }

    pub(crate) fn write_if_dirty(&mut self, core: &RenderApi) {
        if !self.is_dirty {
            return;
        }

        crate::write_buffer_bytes(core, &self.buffer, 0, &self.cache);
        self.is_dirty = false;
    }

    pub fn buffer(&self) -> &GpuUniformStorageBuffer {
        &self.buffer
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn layout(&self) -> &TypedUniformBufferLayout {
        &self.layout
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }
}

//

pub trait GpuVertexValue: AllBitsInit {
    const TY: GpuVertexBufferLayoutEntryTy;
}

macro_rules! impl_gpu_vertex_value {
    ($($t: ty => $ty: ident),+ $(,)?) => {
        $(
            impl GpuVertexValue for $t {
                const TY: GpuVertexBufferLayoutEntryTy = GpuVertexBufferLayoutEntryTy::$ty;
            }
        )+
    };
}

impl_gpu_vertex_value! {
    f32 => Float, [f32; 2] => Float2, [f32; 3] => Float3, [f32; 4] => Float4,
    u32 => UInt, [u32; 2] => UInt2, [u32; 3] => UInt3, [u32; 4] => UInt4,
    i32 => Int, [i32; 2] => Int2, [i32; 3] => Int3, [i32; 4] => Int4,
}

pub struct TypedVertexWriter<'a> {
    index: usize,
    data: &'a mut [u8],
    layout: &'a GpuVertexBufferLayout,
}

impl TypedVertexWriter<'_> {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn set<T: GpuVertexValue>(&mut self, location: usize, value: T) -> bool {
        let Some(entry) = self.layout.entries().iter().find(|e| e.location == location) else {
            return false;
        };

        if entry.ty != T::TY {
            return false;
        }

        self.data[entry.offset..(entry.offset + entry.ty.gpu_size())].copy_from_slice(aphyn_utils::mem::as_bytes(&value));

        true
    }
}

#[repr(C)]
pub struct TypedVertexBuffer {
    buffer: GpuVertexBuffer,
}

impl TypedVertexBuffer {
    pub(crate) fn new(core: &RenderApi, len: usize, layout: GpuVertexBufferLayout) -> Self {
        Self {
            buffer: core.create_vertex_buffer(Some("typed vertex buffer"), len, layout),
        }
    }

    pub(crate) fn write_with(&self, core: &RenderApi, start: usize, count: usize, mut f: impl FnMut(&mut TypedVertexWriter)) {
        let [start, count] = self.clamp_range(start, count);
        let layout = self.buffer.layout();
        let stride = layout.total_size();

        if count == 0 || stride == 0 {
            return;
        }

        core.write_vertex_buffer_with(&self.buffer, start * stride, count * stride, |view| {
            for (i, chunk) in view.chunks_exact_mut(stride).enumerate() {
                // staging memory is not zeroed, and unset attributes must not leak garbage
                chunk.fill(0);

                f(&mut TypedVertexWriter {
                    index: start + i,
                    data: chunk,
                    layout,
                });
            }
        });
    }

    pub fn buffer(&self) -> &GpuVertexBuffer {
        &self.buffer
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn layout(&self) -> &GpuVertexBufferLayout {
        self.buffer.layout()
    }

    pub fn clamp_range(&self, start: usize, count: usize) -> [usize; 2] {
        let len = self.len();
        let start = start.min(len);
        [start, count.min(len - start)]
    }
}

//

// todo: IndexBuffer?

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_layout_follows_wgsl_rules() {
        let layout = TypedUniformBufferLayout::new([
            GpuRuntimeTy::Float,
            GpuRuntimeTy::Float3,
            GpuRuntimeTy::Float2,
            GpuRuntimeTy::Float3x3,
        ]);
        let offsets = layout.entries().iter().map(|e| e.offset).collect::<Vec<_>>();

        assert_eq!(offsets, [0, 16, 32, 48]);
        assert_eq!(layout.total_size(), 96);
        assert_eq!(layout.total_align(), 16);
    }

    #[test]
    fn matrix_columns_are_padded() {
        let mut dst = [0xff_u8; 48];
        [[1.0_f32, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].write_gpu_bytes(&mut dst);

        let floats = dst
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(&floats[0..3], [1.0, 2.0, 3.0]);
        assert_eq!(&floats[4..7], [4.0, 5.0, 6.0]);
        assert_eq!(&floats[8..11], [7.0, 8.0, 9.0]);
    }
}
