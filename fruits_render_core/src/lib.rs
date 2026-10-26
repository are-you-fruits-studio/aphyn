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
//! [`StandardTextureAssetMetadata`] picks the [`StandardTextureFormat`] (linear, sRGB, or HDR) and whether
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
//! white texture (or a flat normal map for the normal slot). Its `shader` is a
//! [`StandardMaterialShader`]: one of the four engine variants ([`StandardEngineShader`], opaque or
//! transparent, lit or unlit), or `Custom` with a [`StandardShader`] handle. Opaque shaders
//! discard fragments whose alpha is below `alpha_threshold`:
//!
//! ```ignore
//! use fruits_render_core::*;
//!
//! let material = render_api.create_material(
//!     StandardMaterialAssets { color_texture: Some(&texture), ..Default::default() },
//!     StandardMaterialAssetMetadata {
//!         shader: StandardEngineShader::OpaqueLit.into(),
//!         metallic: 0.0,
//!         roughness: 0.8,
//!         ..Default::default()
//!     },
//! );
//! ```
//!
//! #### Creating a custom shader
//!
//! A [`StandardShader`] is a composed WGSL source plus the opaque render pipeline built from it.
//! The source must have `vs_main` / `fs_main` entry points that take the standard vertex and
//! instance streams and use the standard bind groups (include `fruits/standard_bindings.wgsl`).
//! Creation returns `None` (and prints the wgpu error) when the WGSL or the pipeline is invalid:
//!
//! ```ignore
//! use fruits_render_core::*;
//!
//! let code = WgslComposer::with_engine_libraries().compose("shaders/water.wgsl", source, &[]).unwrap();
//! let shader = render_api.create_shader("shaders/water.wgsl", &code).unwrap();
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
//! #### Reading the camera being rendered
//!
//! Systems in the [`RenderCamera`](fruits_ecs::Schedule::RenderCamera) schedule read the camera and
//! the color target they draw into from [`CurrentRenderingResource`]. It exists only while that
//! schedule runs:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn draw_something(current_rendering: Res<CurrentRenderingResource>) {
//!     let world_to_clip = current_rendering.world_to_clip();
//!     let [width, height] = current_rendering.target_size();
//!     let view = &current_rendering.target_view;
//!     let _ = (world_to_clip, width, height, view);
//! }
//! ```
//!
//! #### Writing a shader with engine includes
//!
//! Shaders include the engine's WGSL structs and functions instead of copying them. A
//! [`WgslComposer`] holds named WGSL *libraries*;
//! [`with_engine_libraries`](WgslComposer::with_engine_libraries) registers the engine ones:
//!
//! - `fruits/standard_types.wgsl`: `GlobalData`, `MaterialData`, `GenericLight`, `VertexAttributes`,
//!   `InstanceRawAttributes`, `InstanceAttributes`, and `map_instance_data`.
//! - `fruits/standard_bindings.wgsl`: the standard bind groups (`global_data`, `lights`,
//!   `material_data`, and the five material texture/sampler pairs). Includes `standard_types`.
//! - `fruits/pbr.wgsl`: `PI`, `FragmentLight`, `light_generic_to_fragment`, `cook_torrance_brdf`
//!   and its terms, and `create_matrix_tbn`. Includes `standard_types`.
//! - `fruits/fullscreen.wgsl`: `fullscreen_triangle_position(vertex_index)` and `fullscreen_uv(position)`
//!   for drawing a fullscreen triangle with 3 vertices and no vertex buffer.
//!
//! [`compose`](WgslComposer::compose) resolves `#include "<path>"` and the conditional blocks
//! `#ifdef` / `#ifndef` / `#else` / `#endif` against the defines passed in, and returns plain WGSL:
//!
//! ```
//! use fruits_render_core::WgslComposer;
//!
//! let source = r#"
//! #include "fruits/fullscreen.wgsl"
//!
//! @vertex
//! fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
//!     return vec4f(fullscreen_triangle_position(vertex_index), 0.0, 1.0);
//! }
//!
//! @fragment
//! fn fs_main() -> @location(0) vec4f {
//! #ifdef RED
//!     return vec4f(1.0, 0.0, 0.0, 1.0);
//! #else
//!     return vec4f(1.0);
//! #endif
//! }
//! "#;
//!
//! let composer = WgslComposer::with_engine_libraries();
//! let red = composer.compose("my_shader.wgsl", source, &["RED"]).unwrap();
//!
//! assert!(red.contains("fn fullscreen_triangle_position"));
//! assert!(red.contains("vec4f(1.0, 0.0, 0.0, 1.0)"));
//! assert!(!red.contains("vec4f(1.0);"));
//! ```
//!
//! [`create_shader_module`](WgslComposer::create_shader_module) composes and creates the
//! `wgpu::ShaderModule` in one step. Register your own libraries with
//! [`add_library`](WgslComposer::add_library); every library is pasted at most once per
//! composition, however many files include it:
//!
//! ```
//! use fruits_render_core::WgslComposer;
//!
//! let mut composer = WgslComposer::new();
//! composer.add_library("game/noise.wgsl", "fn noise(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(12.9898, 78.233))) * 43758.5453); }");
//! composer.add_library("game/clouds.wgsl", "#include \"game/noise.wgsl\"");
//!
//! let code = composer.compose("sky.wgsl", "#include \"game/noise.wgsl\"\n#include \"game/clouds.wgsl\"", &[]).unwrap();
//! assert_eq!(code.matches("fn noise").count(), 1);
//! ```
//!
//! Errors carry the file and line of the directive that failed:
//!
//! ```
//! use fruits_render_core::{WgslComposeErrorKind, WgslComposer};
//!
//! let error = WgslComposer::new().compose("sky.wgsl", "\n#include \"game/missing.wgsl\"", &[]).unwrap_err();
//! assert_eq!(error.kind, WgslComposeErrorKind::UnknownInclude("game/missing.wgsl".into()));
//! assert_eq!(error.to_string(), "sky.wgsl:2: unknown include \"game/missing.wgsl\"");
//! ```
//!
//! # How to maintain
//!
//! #### Camera and current rendering
//!
//! [`Camera`] holds the plain projection parameters: a [`CameraProjection`] (`Perspective { fov }`
//! with `fov` in radians, or `Orthographic { height }` with the full view height in world units,
//! width following the aspect), plus `near` and `far`. [`Camera::projection_matrix`] picks the
//! matching `fruits_math` builder; route every projection through it rather than reading the
//! variant fields directly. The ECS component that wraps it is `fruits_render::CameraComponent`. [`CurrentRenderingResource`] is
//! inserted by `fruits_app` right before it executes `RenderCamera` and removed right after. It
//! stores the camera transform as plain math (`camera_position`, `camera_scale_rotation`) because
//! `fruits_transform` depends on this crate, so `GlobalTransform` cannot be named here. Its optional
//! `light` is a world-space [`StandardLight`] that `fruits_render` adds on top of the world's own
//! lights for this rendering only (the editor's Scene window uses it so unlit prefabs still show up
//! shaded); `fruits_app` passes `None`.
//! [`world_to_clip`](CurrentRenderingResource::world_to_clip) is the perspective projection, using
//! the target's aspect, times the inverse of the camera transform. The target is created by
//! `fruits_app` in [`RENDER_TARGET_FORMAT`], which `fruits_render` also uses for its pipelines, so
//! both sides must keep agreeing on it. Like the other wgpu-holding resources it is not FFI-safe
//! yet (`todo: support ffi`).
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
//! [`create_material`](RenderApiResource::create_material),
//! [`create_shader`](RenderApiResource::create_shader), and
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
//! a material one (a uniform buffer plus five texture/sampler pairs) — the standard pipeline layout
//! made of those two (shared by the engine standard pipelines in `fruits_render` and by
//! [`StandardShader`] pipelines), and the layout, sampler, and shader used to fill mipmaps. [`RenderAssets`] holds the two 2×2 fallback textures (white,
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
//! `StandardTexture::new` clamps the dimensions to at least 1 and creates the texture in the
//! metadata's [`StandardTextureFormat`] (`Rgba8Unorm`, `Rgba8UnormSrgb`, or `Rgba16Float`, the
//! last matching [`RENDER_TARGET_FORMAT`] so a texture can be drawn into by `fruits_render`). For
//! the 8-bit formats it derives bytes-per-pixel from the data length over the pixel count and pads
//! any source with fewer than four channels out to RGBA with opaque alpha; `Rgba16Float` data must
//! already be 8 bytes per pixel. Empty data skips the upload and leaves the texture zeroed, which
//! is how render targets are created. It builds a `ClampToEdge` sampler that uses the caller's filter
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
//! [`StandardShader`] holds only native data (`StandardShaderNative`: the module and its render
//! pipeline). `StandardShader::new` builds the pipeline with the same state as the engine opaque
//! standard pipeline in `fruits_render` (standard pipeline layout, [`RENDER_TARGET_FORMAT`] with
//! `REPLACE` blending, depth write with `LessEqual` in [`DEPTH_TEXTURE_FORMAT`], back-face
//! culling); keep the two in sync. It wraps the creation in a wgpu validation error scope so an
//! invalid user shader yields `None` instead of hitting wgpu's panicking error handler. Custom
//! shaders are opaque only for now: [`StandardMaterialShader::is_transparent`] is `false` for
//! `Custom`. The material uniform carries `alpha_threshold` for every shader; only the opaque
//! variants use it.
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
//! descriptors and are shared with `fruits_render`. [`clear_color_target`] clears a color view in its own
//! pass (used by `fruits_app` and the editor). `render_graph.rs` is an
//! unfinished render-graph sketch whose methods are `todo!()`; it is compiled but not exported.
//!
//! #### WGSL composer
//!
//! [`WgslComposer`] (`wgsl.rs`) is a line-based preprocessor. A directive is a line whose first
//! non-whitespace character is `#`; WGSL has no `#` token, so directives never clash with
//! shader code. Each directive line becomes an empty line in the output. `#include` looks the
//! quoted path up only in the registered libraries (there is no file-system lookup; callers
//! that load files, like the shader asset loader in `fruits_asset_loading`, collect the paths with
//! [`wgsl_include_paths`], check [`has_library`](WgslComposer::has_library), and register the
//! files first) and is
//! **include-once**: a set of already-pasted paths is kept for the whole
//! [`compose`](WgslComposer::compose) call, because WGSL rejects redefinitions. A path is marked
//! before its library is expanded, and module-scope WGSL declarations do not depend on order,
//! so include cycles terminate and still produce valid code. Conditional blocks nest, test only
//! the defines passed from Rust (there is no `#define`), and skip every directive inside an
//! inactive block, including `#include`. A malformed directive is an error even in an inactive
//! block. Errors carry the file and line of the directive ([`WgslComposeError`]). WGSL
//! compilation errors from wgpu still refer to lines of the composed output.
//!
//! The engine libraries live in `src/assets/wgsl/` and are embedded with `include_str!`. The
//! structs in `standard_types.wgsl` and the bindings in `standard_bindings.wgsl` must stay in
//! sync with [`StandardUniformGlobal`], [`StandardUniformMaterial`], [`StandardGenericLight`],
//! [`StandardVertex::desc`], [`StandardInstance::desc`], and the global and material bind group
//! layouts in [`RenderData`]. `mip_fill_shader.wgsl` uses `fruits/fullscreen.wgsl`. The
//! `engine_shaders_are_valid` unit test composes every library and `mip_fill_shader.wgsl`, and
//! `custom_shader_with_user_library_is_valid` composes a custom standard shader with a user
//! library; both validate with `naga` (a dev-dependency), so no GPU is needed. The composer uses std
//! types because it runs at pipeline creation and is not stored in the world.

mod assets;
pub use assets::*;

mod render_api;
pub use render_api::*;

mod utils;
pub use utils::*;

mod rendering;
pub use rendering::*;

mod wgsl;
pub use wgsl::*;

mod render_graph;
// todo
// pub use render_graph::*;
