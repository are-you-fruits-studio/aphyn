//! # aphyn_render
//!
//! The low-level rendering foundation of the engine. It wraps the GPU API of `aphyn_render_core`
//! in an ECS resource, and defines the standard mesh, texture, and material assets, the vertex and
//! instance formats, and the light description that the rest of the rendering stack draws with.
//! All GPU work goes through `aphyn_render_core`; this crate never names the graphics backend.
//!
//! # How to use
//!
//! The engine inserts a single [`RenderApiResource`] into the world when the window is created, so
//! application code never constructs one itself — it pulls the resource out of the world and asks
//! it to build GPU resources. The three things a user creates are meshes, textures, and
//! materials; each carries optional serializable *asset metadata* describing how it was made.
//! Code that needs the GPU API directly (its own shaders, textures, and passes) gets it with
//! [`core`](RenderApiResource::core), which returns the `aphyn_render_core::RenderApiNew` the
//! resource draws with.
//!
//! #### Uploading a mesh to the GPU
//!
//! Turn a vertex list and a `u16` index list into a [`StandardMesh`] that lives in GPU memory.
//! Most code reaches the resource through the world it is running in:
//!
//! ```ignore
//! use aphyn_render::{RenderApiResource, StandardVertex};
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
//! [`StandardTextureAssetMetadata`] picks the [`StandardTextureFormat`] (linear, sRGB, HDR, or depth)
//! and whether mipmaps are generated:
//!
//! ```ignore
//! use aphyn_render::{FilterMode, RenderApiResource, StandardTextureAssetMetadata};
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
//! A [`StandardTexture`] is a core texture plus its own sampler; [`texture`](StandardTexture::texture)
//! and [`sampler`](StandardTexture::sampler) return them for binding in custom passes. [`Clone`] is
//! shallow: the clone names the same GPU texture and has the same [`id`](StandardTexture::id).
//! [`clear_color_target`] clears a color texture to a color.
//!
//! #### Creating a material
//!
//! A [`StandardMaterial`] is created from a [`StandardMaterialAssetMetadata`]: the PBR parameters
//! plus up to five [`AssetHandle`](aphyn_asset_storage::AssetHandle)s to textures stored in an
//! `AssetStorageResource<StandardTexture>`; an empty or unresolved handle falls back to a built-in
//! white texture (or a flat normal map for the normal slot). Its `shader` is a
//! [`StandardMaterialShader`]: one of the engine variants ([`StandardEngineShader`]: four world-space
//! ones, opaque or transparent, lit or unlit, plus `UiOpaqueUnlit` / `UiTransparentUnlit`, which draw
//! geometry given in window pixels), or `Custom` with a [`StandardShader`] handle. Opaque shaders
//! discard fragments whose alpha is below `alpha_threshold`:
//!
//! ```ignore
//! use aphyn_render::*;
//!
//! // `textures` is the world's `AssetStorageResource<StandardTexture>`.
//! let color_tex = textures.insert(texture);
//!
//! let material = render_api.create_material(StandardMaterialAssetMetadata {
//!     shader: StandardEngineShader::OpaqueLit.into(),
//!     metallic: 0.0,
//!     roughness: 0.8,
//!     color_tex,
//!     ..Default::default()
//! });
//! ```
//!
//! #### Material data
//!
//! A [`Material`] is a template name plus a [`MaterialData`]: an untyped map from parameter name to
//! [`MaterialDataValue`] (`F32`, `Vec4`, or a texture [`AssetHandle`](aphyn_asset_storage::AssetHandle)),
//! kept in insertion order. Read it with the typed accessors (`get_f32`, `get_vec4`, `get_texture`);
//! they return `None` when the name is missing or holds another kind of value. The `get_*_or` variants
//! take the template's default data and fall back to it, then to `0.0`, a zero `Vec4`, or an empty
//! handle. Every `*_mut` accessor bumps the entry's [`snapshot`](MaterialDataEntry::snapshot), so a
//! consumer that remembers the last snapshot it uploaded can update only what changed:
//!
//! ```
//! use aphyn_render::{MaterialData, MaterialDataValue};
//!
//! let mut defaults = MaterialData::new();
//! defaults.insert("wave_speed", MaterialDataValue::F32(0.3));
//!
//! let mut data = MaterialData::new();
//! assert_eq!(data.get_f32_or("wave_speed", &defaults), 0.3);
//!
//! data.insert("wave_speed", MaterialDataValue::F32(1.0));
//! *data.get_f32_mut("wave_speed").unwrap() = 2.0;
//! assert_eq!(data.entry("wave_speed").unwrap().snapshot(), 2);
//! ```
//!
//! #### Creating a custom shader
//!
//! A [`StandardShader`] is a core shader made from composed WGSL with the opaque standard state.
//! The source must have `vs_main` / `fs_main` entry points that take the standard vertex and
//! instance streams and use the standard bind groups (include `aphyn/standard_bindings.wgsl`).
//! Creation returns `None` (and prints the error) when the WGSL is invalid or the fragment inputs
//! don't match the vertex outputs. Bindings or vertex inputs that don't match the standard ones
//! are only found at the first draw, which then fails and prints the error once:
//!
//! ```ignore
//! use aphyn_render::*;
//!
//! let code = WgslComposer::with_engine_libraries().compose("shaders/water.wgsl", source, &[]).unwrap();
//! let shader = render_api.create_shader("shaders/water.wgsl", &code).unwrap();
//! ```
//!
//! #### Drawing instances of a mesh
//!
//! A [`StandardInstanceBuffer`] is a fixed-length GPU array of `local_to_world` matrices. Fill it with
//! [`write_instances`](RenderApiResource::write_instances) from a slice, or with
//! [`write_instances_with`](RenderApiResource::write_instances_with), which asks a callback for each
//! element and writes it straight into the upload staging memory; both clamp the written range to
//! the buffer's length. [`draw_instanced`](RenderApiResource::draw_instanced) then draws a sub-range
//! of the instances (`instance_start` / `instance_count`, clamped inside) with a mesh and a material
//! into a color target and a depth target, both [`StandardTexture`]s. Bind group 0 comes from a
//! [`StandardGlobalBindings`] (camera matrices and lights), bind group 1 from the material and its
//! five textures, resolved from `textures` on every draw. The draw loads and stores both targets;
//! clearing them is up to the caller:
//!
//! ```ignore
//! use aphyn::*;
//!
//! let instances = render_api.create_instance_buffer(1024);
//! let shifted = Mat3::<f32>::IDENTITY.into_4x4_with_offset(Vec3::new(2.0, 0.0, 0.0));
//! render_api.write_instances(&instances, 0, &[Mat4::IDENTITY, shifted]);
//!
//! // Uploads the material uniform if the metadata changed; needs `&mut`, so the draw doesn't do it.
//! material.update_gpu_data_if_dirty(&render_api);
//!
//! render_api.draw_instanced(&StandardInstancedDraw {
//!     target: &current_rendering.target,
//!     depth: &depth_texture, // a `StandardTextureFormat::Depth32Float` texture of the same size
//!     globals: &globals,     // from `render_api.create_global_bindings()`
//!     material: &material,
//!     custom_shader: None,   // the resolved `StandardShader` when `material.meta().shader` is `Custom`
//!     textures: &textures,   // the world's `AssetStorageResource<StandardTexture>`
//!     mesh: &mesh,
//!     instances: &instances,
//!     instance_start: 0,
//!     instance_count: 2,
//! });
//! ```
//!
//! [`draw_batched`](RenderApiResource::draw_batched) draws non-indexed geometry with the same
//! pipeline and bindings: a [`StandardBatchedDraw`] names a core vertex buffer of [`StandardVertex`]s,
//! how many of them to draw, and an instance buffer whose first instance is bound to every vertex.
//! [`material_shader`](RenderApiResource::material_shader) and
//! [`material_textures`](RenderApiResource::material_textures) return what a draw with a material
//! binds, for passes built on the core API directly.
//!
//! #### Runtime-layout GPU buffers
//!
//! When the data layout is only known at runtime (custom shaders), describe it with a list of
//! types instead of a Rust struct. A [`TypedUniformBuffer`] holds one struct, a [`TypedStorageBuffer`]
//! a fixed-length array of them; both use a [`TypedUniformBufferLayout`] (WGSL alignment rules),
//! keep a CPU copy, and upload it only when something changed. Values are set by entry index;
//! `set` returns `false` when the index is out of range or the type doesn't match. Matrices are
//! column-major `[[f32; R]; C]`, like WGSL's `matCxR`:
//!
//! ```ignore
//! use aphyn_render::*;
//!
//! let layout = TypedUniformBufferLayout::new([GpuRuntimeTy::Float4x4, GpuRuntimeTy::Float]);
//! let mut uniform = render_api.create_uniform_buffer(layout.clone());
//! uniform.set(1, 0.5_f32);
//! render_api.write_uniform_buffer_if_dirty(&mut uniform);
//!
//! let mut storage = render_api.create_storage_buffer(64, layout);
//! storage.set(3, 1, 2.0_f32);
//! render_api.write_storage_buffer_if_dirty(&mut storage);
//! ```
//!
//! A [`TypedVertexBuffer`] has no CPU copy: [`write_vertices_with`](RenderApiResource::write_vertices_with)
//! hands a [`TypedVertexWriter`] per element (clamped to the buffer length) that writes attributes by
//! shader location straight into the upload staging memory; unset attributes are zero. Its
//! [`GpuVertexBufferLayout`] carries the step mode, so the same type serves as an instance buffer:
//!
//! ```ignore
//! use aphyn_render::*;
//!
//! let layout = GpuVertexBufferLayout::new(
//!     GpuVertexStepMode::Instance,
//!     [(5, GpuVertexBufferLayoutEntryTy::Float3), (6, GpuVertexBufferLayoutEntryTy::UInt)],
//! )
//! .unwrap(); // `None` on a duplicate location
//!
//! let instances = render_api.create_vertex_buffer(1024, layout);
//! render_api.write_vertices_with(&instances, 0, 2, |w| {
//!     w.set(5, [w.index() as f32, 0.0, 0.0]);
//!     w.set(6, 7_u32);
//! });
//! ```
//!
//! Each typed buffer returns its core buffer from `buffer()`, for binding in a pass.
//! [`write_buffer_elements_with`] fills a range of any core buffer element by element from a callback.
//!
//! #### Describing the standard vertex and instance formats
//!
//! Custom passes that draw standard geometry declare the buffer layouts it is fed through.
//! [`StandardVertex::desc`] describes the per-vertex stream (position, normal, tangent, color, uv
//! at shader locations 0–4) and [`StandardInstance::desc`] the per-instance stream (a
//! `local_to_world` matrix at locations 5–8):
//!
//! ```
//! use aphyn_render::{GpuVertexBufferLayoutEntryTy as Ty, GpuVertexStepMode, StandardInstance, StandardVertex};
//!
//! let vertex_layout = StandardVertex::desc();
//! let instance_layout = StandardInstance::desc();
//!
//! assert_eq!(vertex_layout.step_mode(), GpuVertexStepMode::Vertex);
//! assert_eq!(vertex_layout.total_size(), size_of::<StandardVertex>());
//! assert_eq!(
//!     vertex_layout.entries().iter().map(|e| (e.location, e.ty)).collect::<Vec<_>>(),
//!     [(0, Ty::Float3), (1, Ty::Float3), (2, Ty::Float4), (3, Ty::Float4), (4, Ty::Float2)],
//! );
//!
//! assert_eq!(instance_layout.step_mode(), GpuVertexStepMode::Instance);
//! assert_eq!(
//!     instance_layout.entries().iter().map(|e| (e.location, e.ty)).collect::<Vec<_>>(),
//!     [(5, Ty::Float4), (6, Ty::Float4), (7, Ty::Float4), (8, Ty::Float4)],
//! );
//! ```
//!
//! #### Reading the camera being rendered
//!
//! Systems in the [`RenderCamera`](aphyn_ecs::Schedule::RenderCamera) schedule read the camera and
//! the color target they draw into from [`CurrentRenderingResource`]. It exists only while that
//! schedule runs:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn draw_something(current_rendering: Res<CurrentRenderingResource>) {
//!     let world_to_clip = current_rendering.world_to_clip();
//!     let [width, height] = current_rendering.target_size();
//!     let target: &StandardTexture = &current_rendering.target;
//!     let _ = (world_to_clip, width, height, target);
//! }
//! ```
//!
//! #### Writing a shader with engine includes
//!
//! Shaders include the engine's WGSL structs and functions instead of copying them. A
//! [`WgslComposer`] holds named WGSL *libraries*;
//! [`with_engine_libraries`](WgslComposer::with_engine_libraries) registers the engine ones:
//!
//! - `aphyn/standard_types.wgsl`: `GlobalData`, `MaterialData`, `GenericLight`, `VertexAttributes`,
//!   `InstanceRawAttributes`, `InstanceAttributes`, and `map_instance_data`. `GlobalData` carries
//!   both `matrix_world_to_clip` (the camera) and `matrix_window_to_clip` (window pixels); a custom
//!   shader multiplies by whichever fits the space its geometry is in.
//! - `aphyn/standard_bindings.wgsl`: the standard bind groups (`global_data`, `lights`,
//!   `material_data`, and the five material texture/sampler pairs). Includes `standard_types`.
//! - `aphyn/pbr.wgsl`: `PI`, `FragmentLight`, `light_generic_to_fragment`, `cook_torrance_brdf`
//!   and its terms, and `create_matrix_tbn`. Includes `standard_types`.
//! - `aphyn/fullscreen.wgsl`: `fullscreen_triangle_position(vertex_index)` and `fullscreen_uv(position)`
//!   for drawing a fullscreen triangle with 3 vertices and no vertex buffer.
//!
//! [`compose`](WgslComposer::compose) resolves `#include "<path>"` and the conditional blocks
//! `#ifdef` / `#ifndef` / `#else` / `#endif` against the defines passed in, and returns plain WGSL:
//!
//! ```
//! use aphyn_render::WgslComposer;
//!
//! let source = r#"
//! #include "aphyn/fullscreen.wgsl"
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
//! Register your own libraries with
//! [`add_library`](WgslComposer::add_library); every library is pasted at most once per
//! composition, however many files include it:
//!
//! ```
//! use aphyn_render::WgslComposer;
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
//! use aphyn_render::{WgslComposeErrorKind, WgslComposer};
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
//! matching `aphyn_math` builder; route every projection through it rather than reading the
//! variant fields directly. The ECS component that wraps it is `aphyn_render_ecs::CameraComponent`. [`CurrentRenderingResource`] is
//! inserted by `aphyn_app` right before it executes `RenderCamera` and removed right after. It
//! stores the camera transform as plain math (`camera_position`, `camera_scale_rotation`) because
//! `aphyn_transform` depends on this crate, so `GlobalTransform` cannot be named here. `target` is a
//! shallow [`Clone`] of the color target [`StandardTexture`] (the app's own target, or the editor's
//! view texture from storage). Its optional
//! `light` is a world-space [`StandardLight`] that `aphyn_render_ecs` adds on top of the world's own
//! lights for this rendering only (the editor's Scene window uses it so unlit prefabs still show up
//! shaded); `aphyn_app` passes `None`.
//! [`world_to_clip`](CurrentRenderingResource::world_to_clip) is the perspective projection, using
//! the target's aspect, times the inverse of the camera transform. The target is created by
//! `aphyn_app` in [`RENDER_TARGET_FORMAT`], which `aphyn_render_ecs` also uses for its passes, so
//! both sides must keep agreeing on it. It is not `#[repr(C)]` yet (`todo: support ffi`).
//!
//! #### The resource and FFI
//!
//! [`RenderApiResource`] is the `#[repr(C)]` ECS resource that lives in the world. It is a plain
//! struct of the core API (`aphyn_render_core::RenderApiNew`) plus the engine's own GPU data: the
//! six [`StandardEngineShaders`], the mip fill shader and its `Linear` sampler, and the two 2×2
//! fallback textures (white, and a linear flat normal). It has no vtable of its own: its methods are
//! plain Rust that runs in whichever binary calls it, including a dynamically loaded game library,
//! and reaches the GPU only through core, whose vtable keeps all backend work in the binary that created
//! it. That is why everything this crate stores must be `#[repr(C)]` and FFI-safe: core handles
//! and `aphyn_ffi` types (`FfiVec`, `FfiOption`, `FfiString`), never std containers of non-FFI types.
//! Core handles are cheap reference-counted clones, so [`clone`](RenderApiResource::clone) clones
//! every field and shares the device and resources; a resource is released when its last clone
//! drops.
//!
//! [`new`](RenderApiResource::new) creates the core API from the window and its current size, then
//! the engine shaders, the mip fill shader, and the fallback textures; it panics if any of them
//! fails. [`resize`](RenderApiResource::resize) and [`size`](RenderApiResource::size) map to core's
//! `resize_surface` (which ignores zero sizes) and `surface_size`, converting between this crate's
//! `[u32; 2]` and core's `[usize; 2]`.
//!
//! #### Engine shaders and draws
//!
//! [`StandardEngineShaders`] composes `shader_standard.wgsl` six times: the four combinations of
//! the defines `LIT` and `TRANSPARENT`, plus `UI` and `UI` + `TRANSPARENT` for the UI variants. `UI`
//! only switches the vertex transform from `global_data.matrix_world_to_clip` to
//! `global_data.matrix_window_to_clip`. [`standard_shader_desc`] holds their pipeline state: opaque
//! variants draw with `REPLACE` blending and depth write; transparent ones with additive blending
//! (color `SrcAlpha + One`, alpha `One + One`) and depth test only. All test `LessEqual`, cull back
//! faces, and draw counter-clockwise triangle lists. Opaque variants draw into
//! [`RENDER_TARGET_FORMAT`], transparent ones into [`TRANSPARENT_TARGET_FORMAT`], both with a
//! [`DEPTH_TEXTURE_FORMAT`] depth target; core keys pipelines by these formats, so the draws check
//! them up front. [`material_shader`](RenderApiResource::material_shader) picks the engine shader for
//! an `Engine` material, or the passed custom [`StandardShader`] for `Custom` (`None` when the caller
//! could not resolve it).
//!
//! [`draw_instanced`](RenderApiResource::draw_instanced) returns `false` without drawing when the mesh
//! has no indices, the shader is unresolved, or the target or depth format doesn't match; it returns
//! `true` and draws nothing when the clamped instance range is empty, and otherwise returns what core's
//! `draw_indexed` returns. [`draw_batched`](RenderApiResource::draw_batched) makes the same checks
//! except for the mesh, clamps `vertex_count` to the vertex buffer, returns `true` without drawing
//! when that is zero, and calls core's `draw` with instances `0..1`. Both bind group 0 from
//! [`StandardGlobalBindings`] (uniform at binding 0, lights storage at 1) and group 1 from the
//! material: its uniform at binding 0, then the five texture/sampler pairs (color, roughness,
//! metallic, normal, emission) at bindings 1–10, in the order of `standard_bindings.wgsl`. Textures
//! are resolved from the passed storage on every draw by
//! [`material_textures`](RenderApiResource::material_textures), falling back to the white texture (the
//! flat normal for the normal slot), so a texture replaced under the same handle is picked up without
//! touching the material. Core caches bind groups by value, so this costs no GPU object per draw.
//! Each draw is its own core draw, encoded and submitted on its own. Do not batch them into one
//! encoder: the engine reuses buffers between draws, which only works because each write lands
//! before the next submit.
//!
//! #### GPU assets and buffers
//!
//! The asset types hold core handles directly in `#[repr(C)]` fields and expose them through
//! accessors: [`StandardTexture`] its texture and sampler, [`StandardMesh`] its vertex and index
//! buffers and index count, [`StandardInstanceBuffer`] its vertex buffer, [`StandardGlobalBindings`]
//! its uniform and lights buffers, [`StandardMaterial`] its uniform buffer, and [`StandardShader`]
//! its shader. Meshes and textures keep their optional asset metadata, and materials their metadata,
//! so the asset can be saved back.
//!
//! Core rejects buffer writes whose offset or size is not a multiple of 4 bytes, so every write here
//! must be. All engine structs are; a mesh with an odd number of `u16` indices gets one padding index
//! in its index buffer, while [`indices_count`](StandardMesh::indices_count) keeps the real count.
//! [`write_buffer_elements_with`] works on every core buffer kind through [`GpuWritableBuffer`]. It
//! byte-copies each element into core's staging slice, which has no alignment guarantee for the
//! element type, and produces elements in index order, so a callback may consume an iterator; the
//! callback gets the index relative to `start`.
//!
//! `StandardTexture::new` clamps the dimensions to at least 1 and creates the texture in the
//! metadata's [`StandardTextureFormat`], which converts into core's `GpuTextureFormat`
//! (`Rgba16Float` matches [`RENDER_TARGET_FORMAT`] so a texture can be drawn into, and
//! `Depth32Float` matches [`DEPTH_TEXTURE_FORMAT`]; a depth texture ignores the data and mipmaps).
//! For the 8-bit formats it derives bytes-per-pixel from the data length over the pixel count and
//! pads any source with fewer than four channels out to RGBA with opaque alpha; `Rgba16Float` data
//! must already be 8 bytes per pixel. Empty data skips the upload and leaves the texture zeroed,
//! which is how render targets are created. Its sampler uses the caller's [`FilterMode`], which is
//! core's `GpuFilterMode`. With `should_generate_mipmaps` it allocates the full mip chain and fills
//! each level `n` by drawing a fullscreen triangle with `mip_fill_shader.wgsl` into level `n` while
//! sampling level `n - 1` (instance `n - 1`), with the `Linear` mip fill sampler and no culling. The
//! fill keeps alpha.
//!
//! `StandardMaterial::new` creates its uniform buffer with the default [`StandardUniformMaterial`] and
//! starts dirty. [`meta_mut`](StandardMaterial::meta_mut) marks the material dirty, and
//! [`update_gpu_data_if_dirty`](StandardMaterial::update_gpu_data_if_dirty) rewrites the uniform from
//! the metadata only when dirty (color and emission RGB raised to the power 2.2, sRGB to linear, alpha
//! kept as is so the alpha test sees the authored value) and clears the flag. Textures and the
//! `shader` need no tracking because draws read them. `aphyn_render_ecs` calls it lazily, right
//! before drawing with a material. The material uniform holds no matrix: the world-to-clip and
//! window-to-clip matrices live in [`StandardUniformGlobal`] (written by `aphyn_render_ecs` once per
//! rendering), and the shader decides which one to use. [`RenderSpace`] is only used by
//! `aphyn_render_ecs`'s gizmos. [`StandardLight`] is the serializable light description; it converts
//! into the GPU-side [`StandardGenericLight`], whose `light_type` is `0` point, `1` spot, `2`
//! directional. [`StandardGlobalBindings`] holds the [`StandardUniformGlobal`] buffer and a storage
//! buffer of [`LIGHTS_COUNT_MAX`] [`StandardGenericLight`]s; `write_lights_with` only uploads the first
//! `count`, and the shader reads `lights_count` from the uniform. [`StandardUniformGlobal`],
//! [`StandardUniformMaterial`], [`StandardGenericLight`], and [`StandardVertex`] are `#[repr(C)]` and
//! marked [`AllBitsInit`](aphyn_utils::mem::AllBitsInit) /
//! [`AllBitVariationsValid`](aphyn_utils::mem::AllBitVariationsValid) so they can be uploaded as raw
//! bytes.
//!
//! `StandardShader::new` passes the WGSL to core's `create_shader` with the opaque
//! [`standard_shader_desc`], so custom shaders are opaque only for now:
//! [`StandardMaterialShader::is_transparent`] is `false` for `Custom`. Core checks the WGSL and the
//! fragment inputs at creation, but builds pipelines lazily, so a custom shader whose bindings or
//! vertex inputs don't match the standard ones fails at its first draw (printed once). The material
//! uniform carries `alpha_threshold` for every shader; only the opaque variants use it.
//!
//! [`StandardInstanceBuffer`] is array-like: its length is fixed at creation (zero-initialized) and
//! every write is clamped to it. `buffers.rs` holds [`TypedUniformBuffer`], [`TypedStorageBuffer`],
//! and [`TypedVertexBuffer`] over core buffers. [`TypedUniformBufferLayout`] places each
//! [`GpuRuntimeTy`] at the next offset aligned to its WGSL alignment (`vec3` is 12 bytes aligned to
//! 16, matrix columns are padded to their vector alignment) and rounds the total size up to the
//! largest alignment, which is also the storage array stride; WGSL has no integer matrices, so there
//! are none here. `set` only touches the CPU cache (a plain `FfiVec<u8>`), and `write_*_if_dirty`
//! uploads it when something changed. Vertex attributes are 4-byte aligned and tightly packed; the
//! vertex layout types are defined in `aphyn_render_core` and re-exported here. The vertex writer
//! runs inside core's `write_vertex_buffer_with` callback and borrows one zeroed element of the
//! staging slice at a time. Core allows empty buffers (a draw with one fails, and writes to one do
//! nothing), so zero lengths and empty layouts stay valid.
//!
//! #### Material data
//!
//! `MaterialData` is `#[repr(transparent)]` over `FfiIndexMap<FfiString, MaterialDataEntry>`, the entry
//! is `#[repr(C)]` and the value `#[repr(C, u8)]`, so all of them cross the dylib boundary. The entry
//! snapshot starts at `1` and is advanced by `next_snapshot` (wrapping, never `0`) on every mutable
//! access and on every deserialization into an existing entry. Entries that deserialization creates
//! are reset to snapshot `1` by `MaterialData`, so only entries that already existed are bumped.
//! Inspector edits and reloads do not deserialize in place: `aphyn_asset_loading` loads a fresh
//! `Material` (all entries at `1`) and replaces the stored one under the same handle, first calling
//! [`carry_snapshots_from`](MaterialData::carry_snapshots_from) with the old data so every name that
//! existed before continues from `next_snapshot(old)`. That is what makes the edit visible to a
//! template caching per handle by snapshot. The snapshot is never serialized: `MaterialDataEntry`
//! serializes as its value only, and `MaterialData` delegates to the inner `FfiIndexMap`.
//! Serialization is generic over the context state, so texture handles go through the same
//! handle serializers as `StandardMaterialAssetMetadata`. `MaterialDataValue::default()` is
//! `F32(0.0)` only because the derive and the map need a `Default`; a deserialized value replaces
//! it with the stored variant.
//!
//! #### Helpers and unfinished parts
//!
//! [`clear_color_target`] clears mip level 0 of a color texture through core (used by `aphyn_app`
//! and the editor). `render_graph.rs` is an unfinished render-graph sketch whose methods are
//! `todo!()`; it is compiled but not exported. The `aphyn` facade re-exports
//! `aphyn_render_core` next to this crate.
//!
//! #### WGSL composer
//!
//! [`WgslComposer`] (`wgsl.rs`) is a line-based preprocessor. A directive is a line whose first
//! non-whitespace character is `#`; WGSL has no `#` token, so directives never clash with
//! shader code. Each directive line becomes an empty line in the output. `#include` looks the
//! quoted path up only in the registered libraries (there is no file-system lookup; callers
//! that load files, like the shader asset loader in `aphyn_asset_loading`, collect the paths with
//! [`wgsl_include_paths`], check [`has_library`](WgslComposer::has_library), and register the
//! files first) and is
//! **include-once**: a set of already-pasted paths is kept for the whole
//! [`compose`](WgslComposer::compose) call, because WGSL rejects redefinitions. A path is marked
//! before its library is expanded, and module-scope WGSL declarations do not depend on order,
//! so include cycles terminate and still produce valid code. Conditional blocks nest, test only
//! the defines passed from Rust (there is no `#define`), and skip every directive inside an
//! inactive block, including `#include`. A malformed directive is an error even in an inactive
//! block. Errors carry the file and line of the directive ([`WgslComposeError`]). WGSL
//! compilation errors from core still refer to lines of the composed output.
//!
//! The engine libraries live in `src/assets/wgsl/` and are embedded with `include_str!`. The
//! structs in `standard_types.wgsl` and the bindings in `standard_bindings.wgsl` must stay in
//! sync with [`StandardUniformGlobal`], [`StandardUniformMaterial`], [`StandardGenericLight`],
//! [`StandardVertex::desc`], [`StandardInstance::desc`], and the bind groups that
//! [`draw_instanced`](RenderApiResource::draw_instanced) and
//! [`draw_batched`](RenderApiResource::draw_batched) pass. `shader_standard.wgsl` sits next to them
//! but is not a library; it includes `aphyn/standard_bindings.wgsl` and `aphyn/pbr.wgsl`.
//! `mip_fill_shader.wgsl` uses `aphyn/fullscreen.wgsl`. The `engine_shaders_are_valid` unit test
//! composes every library, `mip_fill_shader.wgsl`, and the six `shader_standard.wgsl` define sets,
//! and `custom_shader_with_user_library_is_valid` composes a custom standard shader with a user
//! library; both validate with `naga`, so no GPU is needed. The composer uses std types because it
//! runs at shader creation and is not stored in the world.

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

mod buffers;
pub use buffers::*;

// todo
// pub use render_graph::*;
