use aphyn_render_core::{GpuIndexBuffer, GpuUniformStorageBuffer, GpuVertexBuffer, RenderApi};
use aphyn_utils::mem::AllBitsInit;

use crate::{RenderApiResource, StandardTexture};

pub trait GpuWritableBuffer {
    fn write_bytes_with(&self, core: &RenderApi, offset: usize, size: usize, writer: impl FnOnce(&mut [u8])) -> bool;
}

impl GpuWritableBuffer for GpuVertexBuffer {
    fn write_bytes_with(&self, core: &RenderApi, offset: usize, size: usize, writer: impl FnOnce(&mut [u8])) -> bool {
        core.write_vertex_buffer_with(self, offset, size, writer)
    }
}

impl GpuWritableBuffer for GpuUniformStorageBuffer {
    fn write_bytes_with(&self, core: &RenderApi, offset: usize, size: usize, writer: impl FnOnce(&mut [u8])) -> bool {
        core.write_uniform_storage_buffer_with(self, offset, size, writer)
    }
}

impl GpuWritableBuffer for GpuIndexBuffer {
    fn write_bytes_with(&self, core: &RenderApi, offset: usize, size: usize, writer: impl FnOnce(&mut [u8])) -> bool {
        core.write_index_buffer_with(self, offset, size, writer)
    }
}

pub fn clear_color_target(render_api: &RenderApiResource, target: &StandardTexture, color: [f32; 4]) -> bool {
    render_api.core().clear_color_texture(target.texture(), 0, color)
}

pub fn write_buffer_elements_with<T: AllBitsInit>(
    core: &RenderApi,
    buffer: &impl GpuWritableBuffer,
    start: usize,
    count: usize,
    mut f: impl FnMut(usize) -> T,
) -> bool {
    let element_size = size_of::<T>();

    if count == 0 || element_size == 0 {
        return false;
    }

    buffer.write_bytes_with(core, start * element_size, count * element_size, |view| {
        // byte copies: the staging view has no alignment guarantee for T
        for (i, chunk) in view.chunks_exact_mut(element_size).enumerate() {
            chunk.copy_from_slice(aphyn_utils::mem::as_bytes(&f(i)));
        }
    })
}

pub(crate) fn write_buffer_bytes(core: &RenderApi, buffer: &impl GpuWritableBuffer, offset: usize, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    buffer.write_bytes_with(core, offset, bytes.len(), |view| view.copy_from_slice(&bytes[..view.len()]))
}
