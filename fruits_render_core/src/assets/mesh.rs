use fruits_ffi::{FfiDroppable, FfiOption, FfiString};
use fruits_serialization::*;
use fruits_utils::mem::{AllBitVariationsValid, AllBitsInit};
use wgpu::{Buffer, Device, util::DeviceExt};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
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
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32x2,
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StandardVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

pub struct StandardMeshNative {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub indices_count: usize,
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
    native: FfiDroppable,
    meta: FfiOption<StandardMeshAssetMetadata>,
}

impl StandardMesh {
    pub(crate) fn new(device: &Device, vertices: &[StandardVertex], indices: &[u16], meta: Option<StandardMeshAssetMetadata>) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: fruits_utils::mem::as_bytes_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: fruits_utils::mem::as_bytes_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            native: FfiDroppable::new(StandardMeshNative {
                index_buffer,
                vertex_buffer,
                indices_count: indices.len(),
            }),
            meta: meta.into(),
        }
    }

    pub fn meta(&self) -> Option<&StandardMeshAssetMetadata> {
        self.meta.as_ref()
    }

    pub unsafe fn native(&self) -> &StandardMeshNative {
        unsafe { &*(self.native.get() as *const StandardMeshNative) }
    }
}

unsafe impl Send for StandardMesh where StandardMeshNative: Send {}
unsafe impl Sync for StandardMesh where StandardMeshNative: Sync {}
