use aphyn_ffi::{FfiDroppable, FfiVec};
use aphyn_utils::mem::AllBitsInit;
use wgpu::{Buffer, BufferDescriptor, BufferSize, BufferUsages, Device, Queue, VertexAttribute, VertexFormat, VertexStepMode};

const MIN_BUFFER_SIZE: usize = wgpu::COPY_BUFFER_ALIGNMENT as usize;

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

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct GpuUniformBufferLayoutEntry {
    pub offset: usize,
    pub ty: GpuRuntimeTy,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuUniformBufferLayout {
    entries: FfiVec<GpuUniformBufferLayoutEntry>,
    total_size: usize,
    total_align: usize,
}

impl GpuUniformBufferLayout {
    pub fn new(layout: impl IntoIterator<Item = GpuRuntimeTy>) -> Self {
        let mut offset = 0;
        let mut max_align = 1;
        let mut entries = FfiVec::new();

        for entry_ty in layout.into_iter() {
            let align = entry_ty.gpu_align();
            offset = adjust_offset_to_align(offset, align);
            max_align = max_align.max(align);

            entries.push(GpuUniformBufferLayoutEntry {
                offset,
                ty: entry_ty,
            });

            offset += entry_ty.gpu_size();
        }

        Self {
            entries,
            total_size: adjust_offset_to_align(offset, max_align),
            total_align: max_align,
        }
    }

    pub fn entries(&self) -> &[GpuUniformBufferLayoutEntry] {
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

pub struct GpuUniformBufferNative {
    pub buffer: Buffer,
}

#[repr(C)]
pub struct GpuUniformBuffer {
    native: FfiDroppable,
    cache: FfiVec<u8>,
    layout: GpuUniformBufferLayout,
    is_dirty: bool,
}

impl GpuUniformBuffer {
    pub(crate) fn new(device: &Device, layout: GpuUniformBufferLayout) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Gpu Uniform Buffer"),
            size: layout.total_size.max(MIN_BUFFER_SIZE) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            native: FfiDroppable::new(GpuUniformBufferNative { buffer }),
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

    pub(crate) fn write_if_dirty(&mut self, queue: &Queue) {
        if !self.is_dirty {
            return;
        }

        let native = unsafe { self.native() };
        queue.write_buffer(&native.buffer, 0, &self.cache);
        self.is_dirty = false;
    }

    pub fn layout(&self) -> &GpuUniformBufferLayout {
        &self.layout
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub unsafe fn native(&self) -> &GpuUniformBufferNative {
        unsafe { &*(self.native.get() as *const GpuUniformBufferNative) }
    }
}

unsafe impl Send for GpuUniformBuffer where GpuUniformBufferNative: Send {}
unsafe impl Sync for GpuUniformBuffer where GpuUniformBufferNative: Sync {}

//

pub struct GpuStorageBufferNative {
    pub buffer: Buffer,
}

#[repr(C)]
pub struct GpuStorageBuffer {
    native: FfiDroppable,
    cache: FfiVec<u8>,
    layout: GpuUniformBufferLayout,
    len: usize,
    is_dirty: bool,
}

impl GpuStorageBuffer {
    pub(crate) fn new(device: &Device, len: usize, layout: GpuUniformBufferLayout) -> Self {
        let size = len * layout.total_size;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Gpu Storage Buffer"),
            size: size.max(MIN_BUFFER_SIZE) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            native: FfiDroppable::new(GpuStorageBufferNative { buffer }),
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

    pub(crate) fn write_if_dirty(&mut self, queue: &Queue) {
        if !self.is_dirty {
            return;
        }

        let native = unsafe { self.native() };
        queue.write_buffer(&native.buffer, 0, &self.cache);
        self.is_dirty = false;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn layout(&self) -> &GpuUniformBufferLayout {
        &self.layout
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub unsafe fn native(&self) -> &GpuStorageBufferNative {
        unsafe { &*(self.native.get() as *const GpuStorageBufferNative) }
    }
}

unsafe impl Send for GpuStorageBuffer where GpuStorageBufferNative: Send {}
unsafe impl Sync for GpuStorageBuffer where GpuStorageBufferNative: Sync {}

//

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpuVertexBufferLayoutEntryTy {
    Float,
    Float2,
    Float3,
    Float4,
    UInt,
    UInt2,
    UInt3,
    UInt4,
    Int,
    Int2,
    Int3,
    Int4,
}

impl GpuVertexBufferLayoutEntryTy {
    pub fn gpu_size(&self) -> usize {
        self.wgpu_format().size() as usize
    }

    pub fn gpu_align(&self) -> usize {
        4
    }

    pub fn wgpu_format(&self) -> VertexFormat {
        // todo: more variants
        match self {
            GpuVertexBufferLayoutEntryTy::Float => VertexFormat::Float32,
            GpuVertexBufferLayoutEntryTy::Float2 => VertexFormat::Float32x2,
            GpuVertexBufferLayoutEntryTy::Float3 => VertexFormat::Float32x3,
            GpuVertexBufferLayoutEntryTy::Float4 => VertexFormat::Float32x4,
            GpuVertexBufferLayoutEntryTy::UInt => VertexFormat::Uint32,
            GpuVertexBufferLayoutEntryTy::UInt2 => VertexFormat::Uint32x2,
            GpuVertexBufferLayoutEntryTy::UInt3 => VertexFormat::Uint32x3,
            GpuVertexBufferLayoutEntryTy::UInt4 => VertexFormat::Uint32x4,
            GpuVertexBufferLayoutEntryTy::Int => VertexFormat::Sint32,
            GpuVertexBufferLayoutEntryTy::Int2 => VertexFormat::Sint32x2,
            GpuVertexBufferLayoutEntryTy::Int3 => VertexFormat::Sint32x3,
            GpuVertexBufferLayoutEntryTy::Int4 => VertexFormat::Sint32x4,
        }
    }
}

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

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GpuVertexStepMode {
    Vertex,
    Instance,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct GpuVertexBufferLayoutEntry {
    pub offset: usize,
    pub location: usize,
    pub ty: GpuVertexBufferLayoutEntryTy,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuVertexBufferLayout {
    entries: FfiVec<GpuVertexBufferLayoutEntry>,
    step_mode: GpuVertexStepMode,
    total_size: usize,
    total_align: usize,
}

impl GpuVertexBufferLayout {
    pub fn new(step_mode: GpuVertexStepMode, layout: impl IntoIterator<Item = (usize, GpuVertexBufferLayoutEntryTy)>) -> Option<Self> {
        let mut offset = 0;
        let mut max_align = 1;
        let mut entries = FfiVec::<GpuVertexBufferLayoutEntry>::new();

        for (entry_location, entry_ty) in layout {
            if entries.iter().any(|e| e.location == entry_location) {
                return None;
            }

            let align = entry_ty.gpu_align();
            offset = adjust_offset_to_align(offset, align);
            max_align = max_align.max(align);

            entries.push(GpuVertexBufferLayoutEntry {
                offset,
                ty: entry_ty,
                location: entry_location,
            });

            offset += entry_ty.gpu_size();
        }

        Some(Self {
            entries,
            step_mode,
            total_size: adjust_offset_to_align(offset, max_align),
            total_align: max_align,
        })
    }

    pub fn entries(&self) -> &[GpuVertexBufferLayoutEntry] {
        &self.entries
    }

    pub fn step_mode(&self) -> GpuVertexStepMode {
        self.step_mode
    }

    pub fn total_size(&self) -> usize {
        self.total_size
    }

    pub fn total_align(&self) -> usize {
        self.total_align
    }

    pub fn to_wgpu_attributes(&self) -> Vec<VertexAttribute> {
        self.entries
            .iter()
            .map(|e| VertexAttribute {
                format: e.ty.wgpu_format(),
                offset: e.offset as u64,
                shader_location: e.location as u32,
            })
            .collect()
    }

    pub fn desc<'a>(&self, attributes: &'a [VertexAttribute]) -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: self.total_size as u64,
            step_mode: match self.step_mode {
                GpuVertexStepMode::Vertex => VertexStepMode::Vertex,
                GpuVertexStepMode::Instance => VertexStepMode::Instance,
            },
            attributes,
        }
    }
}

#[repr(C)]
pub struct GpuVertexWriter {
    index: usize,
    data: *mut u8,
    layout: *const GpuVertexBufferLayout,
}

impl GpuVertexWriter {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn set<T: GpuVertexValue>(&mut self, location: usize, value: T) -> bool {
        let layout = unsafe { &*self.layout };

        let Some(entry) = layout.entries.iter().find(|e| e.location == location) else {
            return false;
        };

        if entry.ty != T::TY {
            return false;
        }

        let data = unsafe { std::slice::from_raw_parts_mut(self.data, layout.total_size) };
        data[entry.offset..(entry.offset + entry.ty.gpu_size())].copy_from_slice(aphyn_utils::mem::as_bytes(&value));

        true
    }
}

pub struct GpuVertexBufferNative {
    pub buffer: Buffer,
}

#[repr(C)]
pub struct GpuVertexBuffer {
    native: FfiDroppable,
    layout: GpuVertexBufferLayout,
    len: usize,
}

impl GpuVertexBuffer {
    pub(crate) fn new(device: &Device, len: usize, layout: GpuVertexBufferLayout) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Gpu Vertex Buffer"),
            size: (len * layout.total_size).max(MIN_BUFFER_SIZE) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            native: FfiDroppable::new(GpuVertexBufferNative { buffer }),
            layout,
            len,
        }
    }

    pub(crate) fn write_with(&self, queue: &Queue, start: usize, count: usize, mut f: impl FnMut(&mut GpuVertexWriter)) {
        let [start, count] = self.clamp_range(start, count);
        let stride = self.layout.total_size;
        let native = unsafe { self.native() };

        let Some(size) = BufferSize::new((count * stride) as u64) else {
            return;
        };
        let Some(mut view) = queue.write_buffer_with(&native.buffer, (start * stride) as u64, size) else {
            return;
        };

        for (i, chunk) in view.chunks_exact_mut(stride).enumerate() {
            // staging memory is not zeroed, and unset attributes must not leak garbage
            chunk.fill(0);

            f(&mut GpuVertexWriter {
                index: start + i,
                data: chunk.as_mut_ptr(),
                layout: &self.layout,
            });
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn layout(&self) -> &GpuVertexBufferLayout {
        &self.layout
    }

    pub fn clamp_range(&self, start: usize, count: usize) -> [usize; 2] {
        let start = start.min(self.len);
        [start, count.min(self.len - start)]
    }

    pub unsafe fn native(&self) -> &GpuVertexBufferNative {
        unsafe { &*(self.native.get() as *const GpuVertexBufferNative) }
    }
}

unsafe impl Send for GpuVertexBuffer where GpuVertexBufferNative: Send {}
unsafe impl Sync for GpuVertexBuffer where GpuVertexBufferNative: Sync {}

//

fn adjust_offset_to_align(offset: usize, align: usize) -> usize {
    let overflow = offset % align;

    if overflow == 0 {
        return offset;
    }

    offset + align - overflow
}

// todo: IndexBuffer?

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_layout_follows_wgsl_rules() {
        let layout = GpuUniformBufferLayout::new([GpuRuntimeTy::Float, GpuRuntimeTy::Float3, GpuRuntimeTy::Float2, GpuRuntimeTy::Float3x3]);
        let offsets = layout.entries().iter().map(|e| e.offset).collect::<Vec<_>>();

        assert_eq!(offsets, [0, 16, 32, 48]);
        assert_eq!(layout.total_size(), 96);
        assert_eq!(layout.total_align(), 16);
    }

    #[test]
    fn matrix_columns_are_padded() {
        let mut dst = [0xff_u8; 48];
        [[1.0_f32, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].write_gpu_bytes(&mut dst);

        let floats = dst.chunks_exact(4).map(|c| f32::from_ne_bytes(c.try_into().unwrap())).collect::<Vec<_>>();
        assert_eq!(&floats[0..3], [1.0, 2.0, 3.0]);
        assert_eq!(&floats[4..7], [4.0, 5.0, 6.0]);
        assert_eq!(&floats[8..11], [7.0, 8.0, 9.0]);
    }

    #[test]
    fn vertex_layout_is_tightly_packed_and_rejects_duplicate_locations() {
        let layout = GpuVertexBufferLayout::new(
            GpuVertexStepMode::Instance,
            [(0, GpuVertexBufferLayoutEntryTy::Float3), (1, GpuVertexBufferLayoutEntryTy::UInt)],
        )
        .unwrap();

        assert_eq!(layout.entries()[1].offset, 12);
        assert_eq!(layout.total_size(), 16);
        assert!(GpuVertexBufferLayout::new(GpuVertexStepMode::Vertex, [(0, GpuVertexBufferLayoutEntryTy::Float); 2]).is_none());
    }
}
