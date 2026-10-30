use aphyn_ffi::FfiDroppable;
use wgpu::{
    BindGroup, Buffer, BufferDescriptor, BufferUsages,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{CreateBindGroupEntry, RenderState, StandardGenericLight, StandardUniformGlobal};

pub const LIGHTS_COUNT_MAX: usize = 1024;

pub struct StandardGlobalBindingsNative {
    pub uniform_buffer: Buffer,
    pub lights_buffer: Buffer,
    pub bind_group: BindGroup,
}

#[repr(C)]
pub struct StandardGlobalBindings {
    native: FfiDroppable,
}

impl StandardGlobalBindings {
    pub(crate) fn new(render_state: &RenderState) -> Self {
        let device = render_state.device();

        let uniform_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Standard Global Uniform Buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            contents: aphyn_utils::mem::as_bytes(&[StandardUniformGlobal::default()]),
        });

        let lights_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Standard Lights Buffer"),
            size: (LIGHTS_COUNT_MAX * size_of::<StandardGenericLight>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = crate::create_bind_group(
            device,
            Some("Standard Global Bind Group"),
            &render_state.render_data().bind_group_layout_global,
            &[
                CreateBindGroupEntry::Buffer(&uniform_buffer),
                CreateBindGroupEntry::Buffer(&lights_buffer),
            ],
        );

        Self {
            native: FfiDroppable::new(StandardGlobalBindingsNative {
                uniform_buffer,
                lights_buffer,
                bind_group,
            }),
        }
    }

    pub(crate) fn write_uniform(&self, render_state: &RenderState, uniform: StandardUniformGlobal) {
        let native = unsafe { self.native() };
        render_state
            .queue()
            .write_buffer(&native.uniform_buffer, 0, aphyn_utils::mem::as_bytes(&[uniform]));
    }

    pub(crate) fn write_lights_with(&self, render_state: &RenderState, count: u32, f: impl FnMut(u64) -> StandardGenericLight) {
        let count = (count as u64).min(LIGHTS_COUNT_MAX as u64);
        let native = unsafe { self.native() };

        crate::write_buffer_elements_with(render_state.queue(), &native.lights_buffer, 0, count, f);
    }

    pub unsafe fn native(&self) -> &StandardGlobalBindingsNative {
        unsafe { &*(self.native.get() as *const StandardGlobalBindingsNative) }
    }
}

unsafe impl Send for StandardGlobalBindings where StandardGlobalBindingsNative: Send {}
unsafe impl Sync for StandardGlobalBindings where StandardGlobalBindingsNative: Sync {}
