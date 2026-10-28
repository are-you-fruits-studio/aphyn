use aphyn_ffi::FfiDroppable;
use aphyn_math::Mat4;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, Queue};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct StandardInstance {
    pub local_to_world: [[f32; 4]; 4],
}

impl StandardInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StandardInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}

pub struct StandardInstanceBufferNative {
    pub buffer: Buffer,
}

#[repr(C)]
pub struct StandardInstanceBuffer {
    native: FfiDroppable,
    len: u32,
}

impl StandardInstanceBuffer {
    pub(crate) fn new(device: &Device, len: u32) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Standard Instance Buffer"),
            size: (len.max(1) as u64) * size_of::<Mat4<f32>>() as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            native: FfiDroppable::new(StandardInstanceBufferNative { buffer }),
            len,
        }
    }

    pub(crate) fn write(&self, queue: &Queue, start: u32, matrices: &[Mat4<f32>]) {
        let [start, count] = self.clamp_range(start, matrices.len().min(u32::MAX as usize) as u32);
        let matrices = &matrices[..count as usize];

        if matrices.is_empty() {
            return;
        }

        let offset = start as u64 * size_of::<Mat4<f32>>() as u64;
        let native = unsafe { self.native() };

        queue.write_buffer(&native.buffer, offset, aphyn_utils::mem::as_bytes_slice(matrices));
    }

    pub(crate) fn write_with(&self, queue: &Queue, start: u32, count: u32, mut f: impl FnMut(u32) -> Mat4<f32>) {
        let [start, count] = self.clamp_range(start, count);
        let native = unsafe { self.native() };

        crate::write_buffer_elements_with(queue, &native.buffer, start as u64, count as u64, |i| f(i as u32));
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn clamp_range(&self, start: u32, count: u32) -> [u32; 2] {
        let start = start.min(self.len);
        [start, count.min(self.len - start)]
    }

    pub unsafe fn native(&self) -> &StandardInstanceBufferNative {
        unsafe { &*(self.native.get() as *const StandardInstanceBufferNative) }
    }
}

unsafe impl Send for StandardInstanceBuffer where StandardInstanceBufferNative: Send {}
unsafe impl Sync for StandardInstanceBuffer where StandardInstanceBufferNative: Sync {}
