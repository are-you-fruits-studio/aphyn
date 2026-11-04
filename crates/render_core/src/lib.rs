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
//!             mip_level: 0,
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
//! [`RenderApiNew::create_shader`] returns `None` unless every `@location` input of `fs_main` is a
//! `@location` output of `vs_main` with the same type. Extra outputs are allowed. Pipeline errors,
//! such as a fragment output that does not fit the target format, only show at the first draw.
//!
//! Draws and clears work on one mip level at a time: [`GpuRenderPassTarget`] and
//! [`GpuRenderPassDepth`] name the `mip_level` they render into. A [`GpuBinding::Texture`] samples
//! either all mip levels (`mip_level: None`) or only one (`Some(level)`), so a pass can render mip
//! level `n` while sampling level `n - 1` of the same texture, which is how mipmaps are filled.
//! Sampling a level that the same pass renders into fails, as does any mip level out of range.
//! [`GpuTexture::mip_level_count`] returns the number of levels.
//!
//! [`RenderApiNew::clear_color_texture`] and [`RenderApiNew::clear_depth_texture`] clear one mip level
//! of a texture without a shader, and return `false` for a texture of the other kind.
//!
//! [`RenderApiNew::copy_texture`] copies mip level 0 of one texture over mip level 0 of another.
//! The sizes must match and the formats must be equal up to sRGB, so `Rgba8Unorm` and `Rgba8UnormSrgb`
//! can be copied into each other. It returns `false` otherwise, for depth textures, and for a texture
//! copied onto itself.
//!
//! Dropping a handle releases its GPU resource. Handles are not `Clone`, so each resource has one
//! owner. A handle may outlive the [`RenderApiNew`] that created it, and dropping it then does nothing.
//!
//! All [`RenderApiNew`] methods and buffer writes take shared references, and the API is
//! `Send + Sync`, so it can be used from several threads. Draws run one at a time. The `writer` closure
//! of a `write_*_buffer_with` method runs without any lock held, so it may call back into the API.
//!
//! [`RenderApiNew::clone`] returns another handle to the same device and resources, so resources
//! created through one clone can be drawn through any other. The API and its handles can be passed to
//! a dynamically loaded library and used from there.
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
//! `write_*_buffer_with` methods take byte offsets and sizes; the size is clamped to the end of the
//! buffer, and the write returns `false` if the offset or the clamped size is not a multiple of 4.
//!
//! Buffers may be empty, and writes to an empty buffer return `false`. A draw with an empty vertex
//! or index buffer returns `false`, even when its ranges are empty. An empty [`GpuUniformStorageBuffer`]
//! can be bound where the shader's binding fits into 4 bytes; a larger binding makes the draw fail.
//!
//! A draw fails unless the pass's vertex buffers supply every attribute the shader declares, with
//! the same type, and no two of them supply the same location. Attributes the shader does not read
//! are allowed.
//!
//! Bind groups are matched by position: `bind_groups[n]` binds `@group(n)`. A pass needs at least one
//! entry per group up to the shader's highest declared group, including groups the shader leaves empty.
//! Bindings and groups the shader does not declare are ignored, so one set of bindings can serve shaders
//! that use only part of it. A draw fails if a declared binding is missing or has the wrong type, or if
//! any group of the pass lists the same binding index twice.
//!
//! # How to maintain
//!
//! #### Dependencies
//!
//! This crate sits below `aphyn_render`, which re-exports [`GpuVertexBufferLayout`] and its entry
//! types from here. It must not depend on `aphyn_render` or on the ECS.
//!
//! #### FFI
//!
//! [`RenderApiNew`] is an `FfiDroppable` holding an `Arc<RenderApiInner>` plus a static vtable of
//! `extern "C-unwind"` functions, the same pattern as `RenderApiResource` in `aphyn_render`, so all
//! wgpu work runs in the binary that created the API. `RenderApiInner` holds the device, queue,
//! surface and state, and implements every operation. The public methods keep Rust signatures and
//! convert their arguments to private `#[repr(C)]` mirrors before calling the vtable: `FfiRenderPass`
//! for [`GpuRenderPass`], `FfiRange` for ranges, `FfiArray` for arrays passed by value, `FfiOption`,
//! `FfiSliceRef` and `FfiStrSliceRef`. Bind groups cross as one flat slice of entries tagged with their
//! group, plus the group count, because nested `FfiSliceRef`s can't be read through a reference; this
//! costs one `Vec` per draw. `Option<&T>` fields are ABI-stable as they are. Buffer writers cross as an
//! `FfiFnMutMut` taking an `FfiSliceMut`, whose `'static` lifetime is a lie the wrapper keeps safe by
//! only passing the slice on to the caller's `FnOnce(&mut [u8])`. `RenderApiNew::new` is generic over
//! the window, so it only runs in the creating binary. Handle accessors such as [`GpuTexture::size`]
//! read `#[repr(C)]` fields directly, and handles release through `FfiTypedDroppable`. Every new
//! method needs a vtable entry and an `ffi_*` function in `RenderApiNew::from_inner`.
//!
//! #### Resources and caches
//!
//! Handles store a `VersionIndex` into the collections of `RenderApiResources`. Bind group
//! layouts, bind groups, pipeline layouts, and pipelines are created on first use and cached by
//! value in `RenderApiCache`. Pipelines are keyed by the shader, the target formats, and the vertex
//! buffer layouts of the pass, so one shader can be drawn with differently laid out buffers.
//! A failed pipeline is cached as `None`, so its error prints once and later draws with the same key
//! fail without retrying. It is evicted with its shader, like a successful one.
//!
//! Each `TextureResource` has a view of all mip levels and one view per level, all created with the
//! texture. Render attachments always use a per-level view, because wgpu only renders into single-mip
//! views. Texture bind group entries store the selected mip level, so binding different levels of one
//! texture creates different bind groups, and releasing the texture evicts all of them.
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
//! named `vs_main` and `fs_main`, and both must exist. Storage textures and non-2D textures are rejected.
//! The `@location` inputs of `fs_main` are checked against the outputs of `vs_main` by location and
//! naga `TypeInner`; builtins and interpolation are not compared.
//!
//! #### Draws
//!
//! Every draw encodes and submits its own command encoder, and resolves its bind groups and
//! pipeline through the caches. `encode_pass` rejects empty vertex and index buffers before any wgpu
//! call, because wgpu panics on slices of empty buffers.
//!
//! #### Buffer sizes
//!
//! Vertex and index buffers are allocated with their exact size. A uniform or storage buffer is
//! allocated rounded up to `COPY_BUFFER_ALIGNMENT` and at least that, because a binding cannot be empty,
//! while its handle reports and writes stay within the requested size. `Queue::write_buffer_with`
//! panics on writes whose offset or size is not a multiple of 4, so `write_buffer_resource_with`
//! rejects them first.

mod buffers;
pub use buffers::*;

mod render_api_new;
pub use render_api_new::*;
