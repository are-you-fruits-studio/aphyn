# Migrating the renderer onto `aphyn_render_core`

## Goal

`aphyn_render` (`crates/render`) and `aphyn_render_ecs` (`crates/render_ecs`) must use `aphyn_render_core` (`crates/render_core`) for all GPU work and must not depend on the `wgpu` crate. Only `render_core` may name wgpu types.

## Current state

- `render_core` is an independent GPU API: `RenderApiNew` in `src/render_api_new.rs` owns the device, the queue and all resources, and hands out `VersionIndex`-based handles (`GpuTexture`, `GpuSampler`, `GpuShader`, `GpuVertexBuffer`, `GpuUniformStorageBuffer`, `GpuIndexBuffer`). A draw is described by a `GpuRenderPass`. Bind group layouts and vertex attributes are reflected from WGSL with naga. Bind groups and pipelines are created on first use and cached by value.
- The only thing the rest of the engine uses from `render_core` today is the vertex layout types, re-exported from `crates/render/src/buffers.rs`.
- `aphyn_render` and `aphyn_render_ecs` use wgpu directly, in these files:
  - `crates/render/src/`: `render_api.rs`, `utils.rs`, `wgsl.rs`, `rendering.rs`, `buffers.rs`, and `assets/{texture,mesh,instance,material,shader,global_bindings}.rs`
  - `crates/render_ecs/src/`: `systems.rs`, `resources.rs`, `assets/shaders.rs`
- `aphyn_render_ecs`, `aphyn_app` and the editor get the wgpu objects through `RenderApiResource::raw()`, `RenderState::device()/queue()/surface()` and the `native()` accessors of the `Standard*` assets. These bypass the FFI vtable of `RenderApiResource`.

## The migration cannot be done yet

Items 1-14 are prerequisites in `render_core`. Items 15-18 are the migration itself, and need all of the prerequisites.

`aphyn_app` (`crates/app/src/render_target.rs`) has to migrate in the same step as `aphyn_render`, even though it is not one of the two target crates. It calls `device()`, `queue()`, `surface()`, `native()` and the `create_*` helpers, and those APIs cannot exist once `aphyn_render` no longer depends on wgpu.

## Already verified

These were checked with a throwaway headless test on DX12 (Intel UHD Graphics) that used only the public `render_core` API. The test was deleted. It validated API calls and did not inspect rendered pixels. Lit, transparent and depth-tested draws were not run, because depth textures cannot be created yet (item 4).

Working:

- `create_shader` accepts all 15 engine shader variants: six `shader_standard.wgsl` define sets (none, `LIT`, `TRANSPARENT`, `LIT`+`TRANSPARENT`, `UI`, `UI`+`TRANSPARENT`), `mip_fill_shader.wgsl`, and in `render_ecs` the transparent final, exposure, bloom gather/blur/downscale/apply, ACES and gizmo shaders.
- These draws return `true`: the standard opaque shader through `draw_indexed` with vertex and instance buffers and all 11 material bindings, bloom gather with a viewport and instance range `2..3`, and gizmos with `LineList` and storage buffers.
- `GpuVertexBufferLayout` gives `StandardVertex` a size of 64 bytes, matching the Rust struct. Layouts: locations 0-4 are `Float3, Float3, Float4, Float4, Float2` (vertex step); locations 5-8 are four `Float4` (instance step).
- A cached `RenderApiNew::draw` cost 77 µs versus 74 µs for raw wgpu with one encoder and submit per draw, which is what the engine does today. Per-draw overhead is not a blocker. This used a draw with no bind groups or vertex buffers, so real draws cost somewhat more.
- Filtering sampler layouts accept the `Nearest` samplers that the existing code binds through `SamplerNonFiltering` layouts, so `CreateBindGroupLayoutEntry::SamplerNonFiltering` needs no equivalent in core. `Rgba16Float` is filterable.

Failures found by that test are marked **confirmed** below.

## Prerequisites: design decisions

These change the shape of the whole API, so do them first. None of them is decided yet: confirm the choice with the user before implementing, as `CLAUDE.md` requires. Each lists a recommendation.

### 1. Own the device and the surface in `render_core`

**Problem.** `RenderApiNew::new(device: wgpu::Device, queue: wgpu::Queue)` has wgpu types in its public signature, so a crate without a wgpu dependency cannot call it. Core has no surface at all. The device, queue and surface are created by `RenderApi::new` in `crates/render/src/render_api.rs`, and `RenderTarget::render_to_surface` in `crates/app/src/render_target.rs` does the acquire, draw and present.

**Needed in core:**

- Initialization from a window: DX12-only instance, adapter compatible with the surface, device and queue, sRGB surface format if offered, `RENDER_ATTACHMENT` usage, the first reported present and alpha modes, `desired_maximum_frame_latency: 2`. Move this logic from `RenderApi::new`.
- Surface resize that ignores zero sizes (`RenderState::resize`), and a way to read the current size (`RenderState::size`).
- A way to draw into the current surface texture and present it.
- A representation of the surface as a draw target. The surface format is chosen at runtime, is often `Bgra8UnormSrgb`, and is not one of the `GpuTextureFormat` variants. `GpuRenderPassTarget::view` only accepts a `&GpuTexture`, and `PipelineKey::target` is an `Option<GpuTextureFormat>`, so both need extending.

**Recommendation.** Core owns all of this and takes the window through a generic bound on wgpu's window-handle traits, without depending on winit. `aphyn_render` keeps its `Arc<Window>` where it needs it.

### 2. Decide how `RenderApiNew` is shared

**Problem.** Every `RenderApiNew` method takes `&mut self`. The engine shares the renderer: `RenderApiResource` is `Send + Sync`, all of its methods take `&self`, systems take `Res<RenderApiResource>` and can run in parallel, and `aphyn_asset_loading` holds `&RenderApiResource` while loading.

**Recommendation.** Switch `RenderApiNew` to `&self` with interior locking around `RenderApiResources` and `RenderApiCache`, so that parallel systems and all existing call sites keep working. Do not hold a lock while calling the user's `writer` closure in the `write_*_buffer_with` methods if that closure can call back into the API (the `*_with` writers in `aphyn_render` run user callbacks).

The handles also currently take `&mut` in the write methods (`buffer: &mut GpuVertexBuffer`). Existing code writes through shared references (`write_instances(&self, buffer: &StandardInstanceBuffer, ...)`, and buffers in resources read through `Res`), so these should become `&`.

### 3. Decide how GPU resources are released

**Problem.** Resources cannot be destroyed (`todo: destroy resources` at the top of `render_api_new.rs`). The engine depends on automatic release:

- `recreate_*` systems in `crates/render_ecs/src/systems.rs` create new textures on every resize.
- The editor creates view textures in `crates/editor/src/features/scene_window.rs`.
- Assets are dropped from `AssetStorageResource`.

Without release, every resize leaks its textures. Entries in `RenderApiCache::bind_groups` are never evicted either, and they keep referring to the old textures.

**Also needed.** `GpuTexture` and the other handles are not `Clone`, but `StandardTexture` has a shallow `Clone` that names the same GPU texture. `CurrentRenderingResource::target` relies on it.

**Recommendation.** Reference-counted handles that release the resource and evict its cached bind groups and pipelines when the last clone is dropped. This keeps today's shallow `Clone` and automatic cleanup. It interacts with item 2 (a drop needs access to the API's state) and with item 12 (the handles cross the FFI boundary).

## Prerequisites: missing or broken features

### 4. `create_texture` panics for sRGB and depth formats (confirmed)

`RenderApiNew::create_texture` always requests `COPY_SRC | COPY_DST | TEXTURE_BINDING | STORAGE_BINDING | RENDER_ATTACHMENT`. wgpu rejects this with a panic:

```
Texture usages TextureUsages(STORAGE_BINDING) are not allowed on a texture of type Rgba8UnormSrgb
Texture usages TextureUsages(STORAGE_BINDING) are not allowed on a texture of type Depth32Float
```

`Rgba8UnormSrgb` is the default `StandardTextureFormat`, and `Depth32Float` is used for the depth target. `Rgba8Unorm` and `Rgba16Float` work.

**Fix.** Choose usages by format. Storage textures are rejected by shader reflection anyway (`reflect_bind_group_layouts`), so `STORAGE_BINDING` can be removed entirely until they are supported.

For depth, use only `TEXTURE_BINDING | RENDER_ATTACHMENT`, as `StandardTexture::new` in `crates/render/src/assets/texture.rs` does; its docs note that depth formats cannot be copy destinations. The test only saw the first rejected usage, so removing `STORAGE_BINDING` alone will probably not be enough for depth. `write_texture` must return `false` for a depth texture instead of hitting a wgpu error.

Add a test that creates all four formats on a real device.

### 5. Add additive blending and write masks

`GpuBlendMode` only has `Replace`, `AlphaBlending` and `PremultipliedAlphaBlending`, and `get_or_create_pipeline` hardcodes `write_mask: wgpu::ColorWrites::ALL`. The engine also needs:

| Pipeline | Where | Color blend | Alpha blend | Write mask |
|---|---|---|---|---|
| Transparent standard variants | `StandardEnginePipelines::new`, `render_api.rs` | `SrcAlpha + One` | `One + One` | all |
| Bloom apply | `recreate_bloom_render_resource`, `systems.rs` | `One + One` | `One + One` | all |
| Transparent composite | `create_standard_render_resource`, `systems.rs` | alpha blending | alpha blending | `COLOR` only |

Add the two additive blend modes (or a general blend description) and a write mask to `GpuShaderDesc`. `PipelineKey` needs no change, since it is keyed by shader and `desc` belongs to the shader. Without the `COLOR` mask, the composite would change the alpha of the render target.

### 6. Add clearing without a draw

`GpuPassOps::Clear` is only reachable through `draw` and `draw_indexed`, which require a shader. These clear without drawing:

- `clear_depth`, `systems.rs`: depth to `1.0`.
- `clear_transparent_target`, `systems.rs`: color to transparent.
- `clear_color_target`, `crates/render/src/utils.rs`: arbitrary color. Called by `RenderTarget::clear` in `aphyn_app` and from `crates/editor/src/features/scene_window.rs`.

Add methods that clear a color texture and a depth texture.

### 7. Add texture-to-texture copy

`render_exposure_system` and `render_color_grading_system` in `systems.rs` render into a temporary `Rgba16Float` texture while sampling the render target, then call `encoder.copy_texture_to_texture` to copy it back over the whole target. They cannot draw straight into the target, because they sample it in the same pass.

Core has no copy. Add one (whole texture, mip 0 is enough), returning `false` on a size or format mismatch. A fullscreen blit is possible but would add a shader and a pass for no benefit.

### 8. Add mipmap generation (confirmed failure)

`StandardTexture::new` in `crates/render/src/assets/texture.rs` fills mip levels when `should_generate_mipmaps` is set. For each level it renders a fullscreen triangle with `mip_fill_shader.wgsl`, using a single-mip view of the previous level as the source and a single-mip view of the next level as the target.

Core has one default view per texture, covering all mips. Drawing into a texture with `mip_level_count: 3` fails:

```
The color attachment at index 0's texture view is not renderable:
  This texture view has more than one mipmap level. View mipmap levels: 3
```

**Options:**

- Add single-mip view selection to both `GpuRenderPassTarget` and `GpuBinding::Texture`, which also requires putting the mip level in `BindGroupBinding` so the bind group cache keys stay correct.
- Add a `generate_mipmaps(&GpuTexture)` method to core and move the shader and the loop there.

The second is smaller and hides the view concept. Either way, the mip fill sampler uses `Linear` for mag and min but `Nearest` for the mip filter (`RenderData::new`), and `create_sampler_inner` currently uses one filter for all three. Keep that behavior through a separate mip filter parameter or internally in `generate_mipmaps`. The mip fill pipeline uses no culling (`GpuCullMode::None`).

### 9. Guard against empty buffers (confirmed panic)

`encode_pass` calls `buffer.buffer.slice(..)` on every vertex buffer and the index buffer. A zero-sized buffer panics with `buffer slices can not be empty` (confirmed with a zero-length `GpuVertexBuffer`; the index buffer goes through the same call).

Empty meshes are valid in the engine: `RenderState::draw_instanced` returns `false` when `mesh.indices_count == 0`, and its docs mention this exact wgpu panic. `GpuUniformStorageBuffer` with size 0 may also be a problem, since `aphyn_render`'s buffers are created with at least `COPY_BUFFER_ALIGNMENT` bytes for that reason.

**Fix.** In `encode_pass`, return `false` instead of drawing when any bound buffer is empty, and check the behavior of a zero-sized `create_buffer_resource`. Keep the existing engine semantics in mind: `draw_instanced` returns `true` and draws nothing when the clamped instance range is empty.

### 10. Report invalid shaders at creation (confirmed)

`StandardShader::new` in `crates/render/src/assets/shader.rs` builds the pipeline inside a validation error scope and returns `None` when it fails. `aphyn_asset_loading` documents that shader creation fails on invalid WGSL.

Core builds pipelines lazily. A shader whose fragment stage reads `@location(0)` that the vertex stage does not output gets `Some` from `create_shader`. Then every draw returns `false`, prints the error again and retries pipeline creation, because failures are not cached. For a broken custom shader, this would print every frame for every draw.

**Fix, both parts:**

- Cache failed pipelines by `PipelineKey`, so the error prints once and the retry stops.
- Add a way to validate at creation: `create_shader`, or a separate method, takes the expected target format, depth format and vertex layouts and builds the pipeline eagerly. `StandardShader::new` needs this with `RENDER_TARGET_FORMAT`, `DEPTH_TEXTURE_FORMAT`, and the standard vertex and instance layouts.

Also let the caller pass a name. `StandardShader::new` uses it for the label and the error message, and core has `todo: expose labels`.

### 11. Allow shaders to declare a subset of a bind group

`resolve_bind_group_entries` returns `None` when `bindings.len() != layout.len()`, and `encode_pass` returns `false` when `pass.bind_groups.len() != shader.bind_group_layouts.len()`.

Today a custom `StandardShader` is built against the fixed `pipeline_layout_standard`, so it may declare only some of the standard bindings, or skip group 1 entirely. Under core the draw would silently fail. The engine shaders are unaffected, since they include all of `standard_bindings.wgsl`.

**Fix.** Ignore extra bindings and extra groups that the shader does not declare, while still failing when a declared binding is missing or has the wrong type. Add tests.

### 12. Make the API work across the FFI boundary

The FFI requirement is not met (`todo: ffi` in `render_api_new.rs`). Engine data crosses a dynamic library boundary. `RenderApiResource` in `crates/render/src/render_api.rs` is `#[repr(C)]`, holds an `FfiDroppable` plus a static vtable of `extern "C-unwind"` functions, and marshals arguments with `aphyn_ffi` types.

Things that cannot cross as they are:

- `GpuRenderPass` is `#[repr(C)]` but contains `Option<...>` and nested `&[...]` slices. `GpuBinding` is passed inside those slices.
- `draw` and `draw_indexed` take `Range<usize>`.
- The `write_*_buffer_with` methods take `impl FnOnce(&mut [u8])`.
- `GpuPassOps` is generic; check that the instantiations used are ABI-stable.

`aphyn_ffi` provides `FfiOption`, `FfiSliceRef`, `FfiFnMutMut` and others. `VersionIndex` and `FfiVec` are already `#[repr(C)]`.

**Decide** whether core gets its own vtable, or whether the vtable stays in `aphyn_render` with FFI-safe descriptors defined in core. Handles from item 3 must also be safe to drop from another binary.

The vtable can stay as it is while `aphyn_render_ecs` still reaches `RenderState` directly through `raw()`; those direct paths are what items 15-17 remove. Decide then whether `raw()` survives at all.

## Prerequisites: cleanup in `render_core`

### 13. Remove wgpu from the public API

These public items expose wgpu types, and a crate without a wgpu dependency cannot use them:

- In `src/buffers.rs`: `GpuVertexBufferLayoutEntryTy::wgpu_format`, `GpuVertexBufferLayout::to_wgpu_attributes`, `GpuVertexBufferLayout::desc`. Make them `pub(crate)`. `use wgpu::{VertexAttribute, VertexFormat, VertexStepMode}` stays as an internal import.
- In `src/render_api_new.rs`: `RenderApiNew::new`, handled by item 1.

`gpu_size` uses `wgpu_format().size()` internally, which is fine. After this, check that `cargo doc` for the crate shows no wgpu types.

### 14. Add texture identity

The engine compares textures:

- `recreate_exposure_render_resource`, `recreate_bloom_render_resource` and `recreate_color_grading_render_resource` in `systems.rs` compare `render_res.target_texture == current_rendering.target.native().texture` to detect a different target.
- `StandardMaterial::update_gpu_data_if_dirty` in `crates/render/src/assets/material.rs` compares `StandardTexture::id()` values to detect a replaced texture.

Add `PartialEq`, or a public id, to `GpuTexture`. `VersionIndex` already supports equality, but a stale index in a reused slot differs only by version, so compare the whole `VersionIndex`. Keep it consistent with the `Clone` from item 3: clones must compare equal.

The material's dirty tracking of bind groups may become unnecessary, because core resolves bind groups by value on each draw. The uniform upload still needs the dirty flag.

## The migration

### 15. Migrate `aphyn_render`

- Replace the wgpu fields of `StandardTextureNative`, `StandardMeshNative`, `StandardMaterialNative`, `StandardShaderNative`, `StandardInstanceBufferNative` and `StandardGlobalBindingsNative` with core handles. Remove the `native()` accessors. Each `StandardTexture` keeps its own `GpuSampler` beside its `GpuTexture`, since materials bind texture and sampler pairs.
- `StandardVertex::desc` and `StandardInstance::desc` return `wgpu::VertexBufferLayout`. Change them to `GpuVertexBufferLayout`, using the layouts under "Already verified".
- Replace `pub use wgpu::FilterMode` in `assets/texture.rs` with `GpuFilterMode`. It is used through this re-export in `crates/asset_loading/src/texture.rs`, `crates/ui/src/systems.rs`, `crates/editor/src/features/scene_window.rs` and `crates/app/src/render_target.rs`. Re-exporting it as `FilterMode` avoids changing them.
- `RENDER_TARGET_FORMAT`, `DEPTH_TEXTURE_FORMAT` and `TRANSPARENT_TARGET_FORMAT` in `rendering.rs` become `GpuTextureFormat`. `StandardTextureFormat` is `Serializable` and maps directly to `GpuTextureFormat`; keep it for asset metadata and add a conversion.
- `StandardEnginePipelines` becomes six `GpuShader`s made from the composed `shader_standard.wgsl`, with `GpuShaderDesc` holding what is now in `ColorTargetState` and `DepthStencilState`. `RenderData`'s bind group and pipeline layouts disappear, because reflection replaces them; keep the WGSL in `standard_bindings.wgsl` in sync with the Rust uniform structs as today.
- Rebuild `draw_instanced` on `draw_indexed`, keeping its checks and return values. Note that bind group 0 comes from `StandardGlobalBindings` and group 1 from the material. Target and depth format checks stay, using `GpuTexture::format()`. Core's `create_texture` takes `[usize; 2]` and `GpuTexture::size` returns `[usize; 2]`, while `aphyn_render` uses `[u32; 2]`.
- Rebuild `GpuUniformBuffer`, `GpuStorageBuffer` and `GpuVertexBuffer` from `crates/render/src/buffers.rs` on top of core buffers. `aphyn_render::GpuVertexBuffer` has the same name as core's, which is why the `aphyn` facade does not export core. Rename or merge them, then decide whether the facade should export `render_core`.
- Replace `write_buffer_elements_with` in `utils.rs` with a helper over core's byte-offset `write_*_buffer_with`. It must keep byte copies (the staging view has no alignment guarantee) and index-order element production, because callers consume iterators.
- Remove `WgslComposer::create_shader_module`, `create_bind_group_layout`, `create_bind_group`, `create_render_pipeline`, `CreateBindGroupLayoutEntry`, `CreateBindGroupEntry`, and `RenderState::device()/queue()/surface()/surface_config_format()`. Keep `WgslComposer::compose` and `wgsl_include_paths`.
- `create_buffer_init` becomes a create followed by a write.
- `next_texture_id` can go if `StandardTexture::id` is replaced by item 14's identity. `update_gpu_data_if_dirty` relies on real ids starting at 1 and on all-zero meaning "never resolved"; keep that behavior.

### 16. Migrate `aphyn_render_ecs`

Rewrite `crates/render_ecs/src/systems.rs` and `resources.rs`. Pipelines and bind groups stop existing as stored objects: resources hold `GpuShader`, `GpuTexture`, `GpuSampler` and buffer handles, and each pass lists its bindings in `GpuRenderPass::bind_groups`.

- `StandardRenderResource`, `GizmosRenderResource`, `TransparentTargetTextureResource`, `ExposureRenderResource`, `BloomRenderResource`, `ColorGradingRenderResource`: replace all wgpu fields. `MaterialStandardRenderResourceData` holds only a `RenderPipeline`; remove it if unused.
- `create_engine_shader_module` in `assets/shaders.rs` becomes compose plus `create_shader`.
- The `recreate_*` systems get much smaller, since they only need to recreate textures. Shaders need to be created only once. The bloom pass needs the viewport and the instance ranges it uses today; both were verified.
- `submit_render_batched` and `render_transparent_final_system` become plain draws. Batched draws set vertex buffer 0 to the shared batched buffer and vertex buffer 1 to `identity_instance`.
- `render_bloom_system` collects command buffers and submits them together. Per-draw submission through core is fine for this.
- Replace `clear_depth` and `clear_transparent_target` with item 6, and the copies in exposure and color grading with item 7.
- `render_opaque` and the transparent systems call `RenderState::material_pipeline` and read `material.native().bind_group`. Replace these with a helper in `aphyn_render` that draws batched geometry for a material.

### 17. Migrate `aphyn_app` and the editor

- Rewrite `crates/app/src/render_target.rs` with core: `new`, `size`, `clear` and `render_to_surface`, using item 1's surface target. The copy shader is `crates/app/src/assets/copy_texture.wgsl`. Update `crates/app/src/event_loop_handler.rs`, which uses `raw()` and `resize`.
- `crates/editor/src/features/scene_window.rs:300` calls `clear_color_target` with `target.native().view`. Change it to item 6's API.
- Remove `wgpu` from `crates/app/Cargo.toml`.
- Other crates (`aphyn_ui`, `aphyn_asset_loading`, the examples) do not name wgpu and only go through `RenderApiResource`, so they should need only the `FilterMode` change at most.

### 18. Remove the dependencies and update the docs

- Remove `wgpu` from `crates/render/Cargo.toml` and `crates/render_ecs/Cargo.toml`. In `crates/render/Cargo.toml` also remove `pollster`, and move `naga` to `[dev-dependencies]` for the shader validation tests.
- Verify with `cargo tree -p aphyn_render -e normal --depth 1` and the same for `aphyn_render_ecs` that wgpu is not a direct dependency, and with `cargo build` that the whole workspace builds.
- Update the `//!` docs of all changed crates, following `AGENTS.md`. `crates/render/src/lib.rs` has a doctest asserting `wgpu::VertexStepMode`, and its "How to maintain" sections describe `RenderState`, wgpu init and the helpers in detail. `crates/render_core/src/lib.rs` says the crate is not used by the rest of the engine and that the facade does not export it.
- Run the examples, at least `aphyn_example_cubes`, `_bloom`, `_gizmos`, `_ui` and `_text`, and the editor, and compare the output by eye. Nothing so far has checked rendered pixels.

## Do not do during the migration

Do not implement `todo: batch draws into a frame encoder instead of submitting per draw` in `encode_pass` until the migration is finished. The engine reuses buffers between draws (`StandardRenderResource::instances`, `batched_vertex_buffer`, the gizmo buffers), and this only works because `Queue::write_buffer` contents are applied at the next submit and each draw submits. With a shared encoder, all draws in a frame would read the last written data, and nothing would report an error.

## Suggested order

1. Items 1-3, because they change the API shape. Item 13 goes along with item 1.
2. Items 4-11 and 14, in any order. They are independent, except that item 14 depends on item 3.
3. Item 12, once the API has settled.
4. Items 15-18, which only build together: `aphyn_render`, `aphyn_render_ecs` and `aphyn_app` must change as one step, because removing `RenderState::device()` breaks the other two.

Each prerequisite needs a test where it can run without a window. Requesting an adapter with no surface works headless, and the existing tests in `render_api_new.rs` show the reflection-only style that needs no GPU at all.
