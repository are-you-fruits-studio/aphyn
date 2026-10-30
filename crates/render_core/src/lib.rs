//! # aphyn_render_core
//!
//! A work-in-progress, ECS-independent GPU API on top of wgpu. [`RenderApiNew`] owns the device,
//! the queue, and every GPU resource, and hands out [`GpuVertexBuffer`], [`GpuUniformStorageBuffer`],
//! [`GpuIndexBuffer`], [`GpuTexture`], [`GpuSampler`],
//! and [`GpuShader`] handles. It is not used by the rest of the engine yet, and the `aphyn` facade
//! does not export it, because its `GpuVertexBuffer` would clash with the one in `aphyn_render`.
//!
//! # How to use
//!
//! Create a [`RenderApiNew`] from a `wgpu::Device` and `wgpu::Queue`, create resources through it,
//! and describe each draw with a [`GpuRenderPass`]:
//!
//! ```ignore
//! let mut api = RenderApiNew::new(device, queue);
//!
//! let shader = api.create_shader(wgsl, GpuShaderDesc::default()).unwrap();
//! let target = api.create_texture([256, 256], GpuTextureFormat::Rgba8UnormSrgb, 1);
//!
//! api.draw(
//!     &GpuRenderPass {
//!         shader: &shader,
//!         target: Some(GpuRenderPassTarget {
//!             view: &target,
//!             ops: GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]),
//!         }),
//!         depth: None,
//!         bind_groups: &[],
//!         vertex_buffers: &[],
//!         viewport: None,
//!         scissor: None,
//!     },
//!     0..3,
//!     0..1,
//! );
//! ```
//!
//! Failed operations return `None` or `false`, and shader and validation errors are printed.
//!
//! [`GpuVertexBuffer`] holds vertices or instances, as set by its [`GpuVertexBufferLayout`], which
//! computes tightly packed
//! attribute offsets and rejects duplicate shader locations. [`GpuUniformStorageBuffer`] holds
//! untyped bytes for bind groups, and [`GpuIndexBuffer`] stores its [`GpuIndexFormat`]. The
//! `write_*_buffer_with` methods take byte offsets and sizes.
//!
//! A draw fails unless the pass's vertex buffers supply every attribute the shader declares, with
//! the same type, and no two of them supply the same location. Attributes the shader does not read
//! are allowed.
//!
//! # How to maintain
//!
//! #### Dependencies
//!
//! This crate sits below `aphyn_render`, which re-exports [`GpuVertexBufferLayout`] and its entry
//! types from here. It must not depend on `aphyn_render` or on the ECS.
//!
//! #### Resources and caches
//!
//! Handles store a `VersionIndex` into the collections of `RenderApiResources`. Bind group
//! layouts, bind groups, pipeline layouts, and pipelines are created on first use and cached by
//! value in `RenderApiCache`. Resources cannot be destroyed yet, so cached entries are never evicted.
//! Pipelines are keyed by the shader, the target formats, and the vertex buffer layouts of the pass,
//! so one shader can be drawn with differently laid out buffers.
//!
//! #### Shader reflection
//!
//! `create_shader` parses the WGSL with naga to derive the bind group layouts and the `@location`
//! attributes of `vs_main`, from its arguments and struct members. Entry points must be
//! named `vs_main` and `fs_main`. Storage textures and non-2D textures are rejected.
//!
//! #### Draws
//!
//! Every draw encodes and submits its own command encoder, and resolves its bind groups and
//! pipeline through the caches.

mod buffers;
pub use buffers::*;

mod render_api_new;
pub use render_api_new::*;
