use aphyn_ffi::{FfiOption, FfiString};
use aphyn_render_core::{
    GpuIndexBuffer, GpuIndexFormat, GpuVertexBuffer, GpuVertexBufferLayout, GpuVertexBufferLayoutEntryTy, GpuVertexStepMode,
    RenderApi,
};
use aphyn_serialization::*;
use aphyn_utils::mem::{AllBitVariationsValid, AllBitsInit};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Serializable)]
pub struct StandardVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

unsafe impl AllBitVariationsValid for StandardVertex {}
unsafe impl AllBitsInit for StandardVertex {}

impl StandardVertex {
    pub fn desc() -> GpuVertexBufferLayout {
        GpuVertexBufferLayout::new(
            GpuVertexStepMode::Vertex,
            [
                (0, GpuVertexBufferLayoutEntryTy::Float3),
                (1, GpuVertexBufferLayoutEntryTy::Float3),
                (2, GpuVertexBufferLayoutEntryTy::Float4),
                (3, GpuVertexBufferLayoutEntryTy::Float4),
                (4, GpuVertexBufferLayoutEntryTy::Float2),
            ],
        )
        .unwrap()
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serializable)]
pub enum CoordinateSpaceType {
    #[default]
    LeftHandZForward,
    RightHandZBack,
    RightHandZUp,
}

#[repr(C)]
#[derive(Serializable, Clone, Debug, Default)]
pub struct StandardMeshAssetMetadata {
    pub raw_mesh: FfiString,
    pub coordinate_space: CoordinateSpaceType,
    pub has_clockwise_winding: bool,
    pub has_inverted_u: bool,
    pub has_inverted_v: bool,
}

#[repr(C)]
pub struct StandardMesh {
    vertex_buffer: GpuVertexBuffer,
    index_buffer: GpuIndexBuffer,
    indices_count: usize,
    meta: FfiOption<StandardMeshAssetMetadata>,
}

impl StandardMesh {
    pub(crate) fn new(
        core: &RenderApi,
        vertices: &[StandardVertex],
        indices: &[u16],
        meta: Option<StandardMeshAssetMetadata>,
    ) -> Self {
        let label = meta
            .as_ref()
            .map(|meta| meta.raw_mesh.as_str())
            .filter(|label| !label.is_empty());

        let vertex_buffer = core.create_vertex_buffer(label, vertices.len(), StandardVertex::desc());
        crate::write_buffer_bytes(core, &vertex_buffer, 0, aphyn_utils::mem::as_bytes_slice(vertices));

        // core only writes multiples of 4 bytes, so an odd count gets a padding index
        let mut padded_indices = indices.to_vec();
        if padded_indices.len() % 2 != 0 {
            padded_indices.push(0);
        }

        let index_buffer = core.create_index_buffer(label, padded_indices.len(), GpuIndexFormat::U16);
        crate::write_buffer_bytes(core, &index_buffer, 0, aphyn_utils::mem::as_bytes_slice(&padded_indices));

        Self {
            vertex_buffer,
            index_buffer,
            indices_count: indices.len(),
            meta: meta.into(),
        }
    }

    pub fn meta(&self) -> Option<&StandardMeshAssetMetadata> {
        self.meta.as_ref()
    }

    pub fn vertex_buffer(&self) -> &GpuVertexBuffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &GpuIndexBuffer {
        &self.index_buffer
    }

    pub fn indices_count(&self) -> usize {
        self.indices_count
    }
}
