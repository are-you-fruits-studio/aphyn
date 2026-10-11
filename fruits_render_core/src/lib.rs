//! # fruits_render_core
//!
//! The low-level rendering foundation of the engine. It owns the connection to the GPU and the
//! window surface, and defines the standard mesh, texture, and material assets, the vertex and
//! instance formats, and the light description that the rest of the rendering stack draws with.
//!
//! # How to use
//!
//! The engine inserts a single [`RenderApiResource`] into the world when the window is created, so
//! application code never constructs one itself — it pulls the resource out of the world and asks
//! it to build GPU resources. The three things a user creates are meshes, textures, and
//! materials; each carries optional serializable *asset metadata* describing how it was made.
//!
//! #### Uploading a mesh to the GPU
//!
//! Turn a vertex list and a `u16` index list into a [`StandardMesh`] that lives in GPU memory.
//! Most code reaches the resource through the world it is running in:
//!
//! ```ignore
//! use fruits_render_core::{RenderApiResource, StandardVertex};
//!
//! // `render_api` is the `&RenderApiResource` the engine inserted into the world.
//! let vertex = |position: [f32; 3], uv: [f32; 2]| StandardVertex {
//!     position,
//!     normal: [0.0, 0.0, 1.0],
//!     tangent: [1.0, 0.0, 0.0, 1.0],
//!     color: [1.0; 4],
//!     uv,
//! };
//! let vertices = [
//!     vertex([-0.5, -0.5, 0.0], [0.0, 1.0]),
//!     vertex([ 0.5, -0.5, 0.0], [1.0, 1.0]),
//!     vertex([ 0.0,  0.5, 0.0], [0.5, 0.0]),
//! ];
//! let indices = [0u16, 1, 2];
//!
//! // No asset metadata: the mesh was not loaded from an asset file.
//! let mesh = render_api.create_mesh(&vertices, &indices, None);
//! ```
//!
//! #### Uploading a texture to the GPU
//!
//! Upload raw pixel bytes as a [`StandardTexture`]. The [`FilterMode`] selects how the texture is
//! sampled; the dimensions are `[width, height]` in pixels. The
//! [`StandardTextureAssetMetadata`] decides whether the data is linear or sRGB and whether
//! mipmaps are generated:
//!
//! ```ignore
//! use fruits_render_core::{FilterMode, RenderApiResource, StandardTextureAssetMetadata};
//!
//! // `rgba` holds `width * height * 4` bytes; sources with fewer channels are padded to RGBA.
//! let texture = render_api.create_texture(
//!     FilterMode::Linear,
//!     [width, height],
//!     &rgba,
//!     StandardTextureAssetMetadata { should_generate_mipmaps: true, ..Default::default() },
//! );
//! ```
//!
//! #### Creating a material
//!
//! A [`StandardMaterial`] combines the PBR parameters in [`StandardMaterialAssetMetadata`] with up
//! to five textures ([`StandardMaterialAssets`]); a missing texture falls back to a built-in
//! white texture (or a flat normal map for the normal slot):
//!
//! ```ignore
//! use fruits_render_core::*;
//!
//! let material = render_api.create_material(
//!     StandardMaterialAssets { color_texture: Some(&texture), ..Default::default() },
//!     StandardMaterialAssetMetadata {
//!         is_lit: true,
//!         metallic: 0.0,
//!         roughness: 0.8,
//!         ..Default::default()
//!     },
//! );
//! ```
//!
//! #### Describing the standard vertex and instance formats
//!
//! When building a render pipeline, declare the buffer layouts geometry is fed through.
//! [`StandardVertex::desc`] describes the per-vertex stream (position, normal, tangent, color, uv
//! at shader locations 0–4) and [`StandardInstance::desc`] the per-instance stream (a
//! `local_to_world` matrix at locations 5–8):
//!
//! ```
//! use fruits_render_core::{StandardInstance, StandardVertex};
//!
//! let vertex_layout = StandardVertex::desc();
//! let instance_layout = StandardInstance::desc();
//!
//! assert_eq!(vertex_layout.step_mode, wgpu::VertexStepMode::Vertex);
//! assert_eq!(instance_layout.step_mode, wgpu::VertexStepMode::Instance);
//! ```
//!
//! # How to maintain
//!
//! #### The FFI-stable resource
//!
//! The crate's public surface is **FFI-stable**, so the engine can call into a dynamically linked
//! render module across a fixed ABI. [`RenderApiResource`] is the `#[repr(C)]` ECS resource that
//! actually lives in the world: it holds a type-erased [`fruits_ffi::FfiDroppable`] wrapping an
//! `Arc<`[`RenderState`]`>` plus a static vtable of `extern "C-unwind"` functions. Its inherent
//! methods ([`resize`](RenderApiResource::resize), [`size`](RenderApiResource::size),
//! [`create_texture`](RenderApiResource::create_texture),
//! [`create_mesh`](RenderApiResource::create_mesh),
//! [`create_material`](RenderApiResource::create_material), and
//! [`clone`](RenderApiResource::clone), which clones the `Arc`) only marshal their arguments
//! through that vtable into the underlying `RenderState`. Code running in the same binary skips
//! the marshalling with the unsafe [`raw`](RenderApiResource::raw) accessor, which reinterprets
//! the erased pointer and returns a clone of the `Arc<RenderState>` — `fruits_render` uses this to
//! reach the device, queue, and surface directly when building pipelines.
//!
//! #### Render state
//!
//! [`RenderState`] composes three parts. [`RenderApi`] holds the wgpu `Device`, `Queue`,
//! `Surface`, and owning `Window`, plus the surface configuration and cached size behind a
//! `Mutex` (so [`resize`](RenderState::resize) works through `&self`). `RenderApi::new` performs
//! all wgpu initialization on construction: it requests a **DX12-only** instance, creates the
//! surface from the window, requests an adapter compatible with that surface, and blocks on the
//! device and queue with `pollster`. It prefers an sRGB surface format when one is offered and
//! configures the surface for `RENDER_ATTACHMENT` usage with the first reported present and alpha
//! modes. `resize` ignores zero-sized requests and reconfigures the surface. [`RenderData`] holds
//! the long-lived bind group layouts — a global one (a uniform buffer plus a storage buffer) and
//! a material one (a uniform buffer plus five texture/sampler pairs) — and the layout, sampler,
//! and shader used to fill mipmaps. [`RenderAssets`] holds the two 2×2 fallback textures (white,
//! and a linear flat normal). Two `todo`s in `render_api.rs` flag that wgpu init may later move
//! into an ECS `Start` handler and that the per-field accessors should be replaced by exposing
//! the `api` as a struct.
//!
//! #### GPU assets
//!
//! The GPU asset types wrap their native handles behind the same FFI boundary.
//! [`StandardMesh`], [`StandardTexture`], and [`StandardMaterial`] are `#[repr(C)]` structs of an
//! `FfiDroppable` holding the native payload (`StandardMeshNative`, `StandardTextureNative`,
//! `StandardMaterialNative`) plus their asset metadata (optional for meshes and textures), which
//! is kept so the asset can be saved back. Their `Send`/`Sync` are forwarded from the native
//! payloads, and the native handles are reached only through the unsafe `native()` accessors.
//!
//! `StandardTexture::new` clamps the dimensions to at least 1, derives bytes-per-pixel from the
//! data length over the pixel count, and pads any source with fewer than four channels out to
//! RGBA with opaque alpha. It uploads as `Rgba8Unorm` when the metadata is `is_linear` and
//! `Rgba8UnormSrgb` otherwise, and builds a `ClampToEdge` sampler that uses the caller's filter
//! mode for magnification, minification, and mipmapping. With `should_generate_mipmaps` it
//! allocates the full mip chain and fills each level from the previous one by rendering a
//! fullscreen triangle through `mip_fill_shader.wgsl`, submitting one command buffer per level.
//!
//! `StandardMaterial::new` creates a uniform buffer initialized with the default
//! [`StandardUniformMaterial`] and a bind group against the material layout, with the textures
//! resolved once at creation. `fruits_render` rewrites the uniform from the metadata each time
//! it draws with the material, but the texture handles stored in the metadata are not
//! re-resolved afterwards (see the `todo` in `create_material`). [`RenderSpace`]
//! selects which space a material's geometry is drawn in, and [`StandardLight`] is the
//! serializable light description; it converts into the GPU-side [`StandardGenericLight`], whose
//! `light_type` is `0` point, `1` spot, `2` directional. [`StandardUniformGlobal`],
//! [`StandardUniformMaterial`], and [`StandardGenericLight`] are `#[repr(C)]` and marked
//! [`AllBitsInit`](fruits_utils::mem::AllBitsInit) /
//! [`AllBitVariationsValid`](fruits_utils::mem::AllBitVariationsValid) so they can be uploaded as
//! raw bytes.
//!
//! [`StandardVertex`] is marked the same way and reinterpreted with
//! `fruits_utils::mem::as_bytes_slice` when its buffer is uploaded. Its five attributes sit at
//! locations 0–4; the per-instance `local_to_world` matrix occupies locations 5–8 as four
//! `Float32x4` rows.
//!
//! #### Helpers and unfinished parts
//!
//! [`create_bind_group_layout`], [`create_bind_group`], and [`create_render_pipeline`] (with the
//! [`CreateBindGroupLayoutEntry`] / [`CreateBindGroupEntry`] shorthands) wrap the verbose wgpu
//! descriptors and are shared with `fruits_render`. [`Shader`] is a thin WGSL module wrapper that
//! the pipeline path does not use and is marked `// todo: remove`. `render_graph.rs` is an
//! unfinished render-graph sketch whose methods are `todo!()`; it is compiled but not exported.

mod assets;
pub use assets::*;

mod render_api;
pub use render_api::*;

mod utils;
pub use utils::*;

mod render_graph;
// todo
// pub use render_graph::*;
