//! # aphyn_render_core
//!
//! A work-in-progress, ECS-independent GPU API on top of wgpu. [`RenderApiNew`] owns the device,
//! the queue, the window surface, and every GPU resource, and hands out [`GpuVertexBuffer`], [`GpuUniformStorageBuffer`],
//! [`GpuIndexBuffer`], [`GpuTexture`], [`GpuSampler`],
//! and [`GpuShader`] handles. It is not used by the rest of the engine yet, and the `aphyn` facade
//! does not export it, because its `GpuVertexBuffer` would clash with the one in `aphyn_render`.
//!
//! # How to use
//!
//! Create a [`RenderApiNew`] from a window and its size in pixels, create resources through it,
//! and describe each draw with a [`GpuRenderPass`]. The window is anything that provides raw window
//! and display handles, such as `Arc<winit::window::Window>`:
//!
//! ```ignore
//! let api = RenderApiNew::new(window, [1280, 720]).unwrap();
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
//! [`RenderApiNew::clear_color_texture`] and [`RenderApiNew::clear_depth_texture`] clear a texture
//! without a shader. They clear only mip level 0, and return `false` for a texture of the other kind.
//!
//! Dropping a handle releases its GPU resource. Handles are not `Clone`, so each resource has one
//! owner. A handle may outlive the [`RenderApiNew`] that created it, and dropping it then does nothing.
//!
//! All [`RenderApiNew`] methods and buffer writes take shared references, and the API is
//! `Send + Sync`, so it can be used from several threads. Draws run one at a time. The `writer` closure
//! of a `write_*_buffer_with` method runs without any lock held, so it may call back into the API.
//!
//! [`RenderApiNew::draw_to_surface`] draws into the window and presents the frame, so it is one draw
//! per frame. Its pass must have no `target`, and the load operation is passed separately. It returns
//! `false` without presenting if the frame cannot be acquired or the draw fails.
//! [`RenderApiNew::resize_surface`] ignores zero sizes, and [`RenderApiNew::surface_size`] returns the
//! last accepted size.
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
//! value in `RenderApiCache`. Pipelines are keyed by the shader, the target formats, and the vertex
//! buffer layouts of the pass, so one shader can be drawn with differently laid out buffers.
//!
//! Each handle holds a `ResourceGuard` with a `Weak` reference to the state. Its drop removes the
//! resource and evicts what keeps the wgpu object alive: the bind groups that use a texture, sampler,
//! or buffer, or the pipelines of a shader. Bind group layouts and pipeline layouts stay cached,
//! because they are keyed by value and reference no resources. wgpu keeps resources alive for work
//! already submitted, so releasing right after a draw is safe. The guard is stored in an
//! `FfiTypedDroppable`, so it drops through a function pointer into the binary that created it.
//!
//! #### Locking
//!
//! One `Mutex` guards `RenderApiResources` and `RenderApiCache`. wgpu keeps a single error scope stack
//! per device, not per thread, so this lock must be held from `push_error_scope` to `pop_error_scope`,
//! or errors get reported to the wrong operation. That covers `create_shader`, pipeline creation, and
//! all of `encode_pass`. A second `Mutex` guards the surface and is held from acquire to present, so
//! waiting for a frame does not block other threads from creating resources, and a resize cannot
//! reconfigure the surface while a frame is acquired. Always lock the surface first, then the state.
//! `ResourceGuard` locks the state on drop, so no handle may be dropped while the state lock is held.
//! Removing a resource only drops wgpu objects, and `writer` closures run unlocked, so neither does this.
//!
//! #### Surface
//!
//! `RenderApiNew::new` creates a DX12-only instance, an adapter compatible with the surface, the
//! device, and the queue. It picks the first sRGB surface format if there is one, because the format
//! is only known at runtime and is often not a [`GpuTextureFormat`]; this is why `PipelineKey::target`
//! stores a `wgpu::TextureFormat`. Only tests build the API without a surface, through `new_headless`.
//! The window bound uses `raw-window-handle` traits so that this crate does not depend on winit and
//! callers do not need wgpu.
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
