use std::hash::{Hash, Hasher};

use aphyn_ffi::FfiVec;
use aphyn_utils::index_version_collection::VersionIndex;
use wgpu::{VertexAttribute, VertexFormat, VertexStepMode};

pub(crate) struct BufferResource {
    pub(crate) buffer: wgpu::Buffer,
}

#[repr(C)]
pub struct GpuVertexBuffer {
    pub(crate) id: VersionIndex,
    pub(crate) layout: GpuVertexBufferLayout,
    pub(crate) len: usize,
}

impl GpuVertexBuffer {
    pub fn layout(&self) -> &GpuVertexBufferLayout {
        &self.layout
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn size(&self) -> usize {
        self.len * self.layout.total_size
    }
}

#[repr(C)]
pub struct GpuUniformStorageBuffer {
    pub(crate) id: VersionIndex,
    pub(crate) size: usize,
}

impl GpuUniformStorageBuffer {
    pub fn size(&self) -> usize {
        self.size
    }
}

#[repr(C)]
pub struct GpuIndexBuffer {
    pub(crate) id: VersionIndex,
    pub(crate) format: GpuIndexFormat,
    pub(crate) len: usize,
}

impl GpuIndexBuffer {
    pub fn format(&self) -> GpuIndexFormat {
        self.format
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn size(&self) -> usize {
        self.len * self.format.gpu_size()
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuIndexFormat {
    #[default]
    U16,
    U32,
}

impl GpuIndexFormat {
    pub fn gpu_size(self) -> usize {
        match self {
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }

    pub(crate) fn to_wgpu(self) -> wgpu::IndexFormat {
        match self {
            Self::U16 => wgpu::IndexFormat::Uint16,
            Self::U32 => wgpu::IndexFormat::Uint32,
        }
    }
}

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

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GpuVertexStepMode {
    Vertex,
    Instance,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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

impl Hash for GpuVertexBufferLayout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entries().hash(state);
        self.step_mode.hash(state);
        self.total_size.hash(state);
        self.total_align.hash(state);
    }
}

impl GpuVertexBufferLayout {
    pub fn new(step_mode: GpuVertexStepMode, layout: impl IntoIterator<Item = (usize, GpuVertexBufferLayoutEntryTy)>) -> Option<Self> {
        let mut offset = 0_usize;
        let mut max_align = 1;
        let mut entries = FfiVec::<GpuVertexBufferLayoutEntry>::new();

        for (entry_location, entry_ty) in layout {
            if entries.iter().any(|e| e.location == entry_location) {
                return None;
            }

            let align = entry_ty.gpu_align();
            offset = offset.next_multiple_of(align);
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
            total_size: offset.next_multiple_of(max_align),
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

#[cfg(test)]
mod tests {
    use super::*;

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
