//! # aphyn_render_features
//!
//! The engine's rendering pipeline: render features, the per-view pass list, data-driven
//! materials, lights, gizmos and post-processing. A [`RenderFeatureRegistry`] owns the render
//! features of one world. Every [`render_view`](RenderFeatureRegistry::render_view) asks each
//! feature for its passes, sorts them by an absolute `u64` order, and runs them one after another
//! on the calling thread. Everything the engine draws is such a feature: [`GlobalsRenderFeature`]
//! (camera, lights, depth, scene normals and transparent targets), [`ClusteringRenderFeature`] (assigns lights to
//! view clusters), [`SsaoRenderFeature`] (screen-space ambient occlusion), [`SkyboxRenderFeature`] (sky and environment map), [`MaterialRenderFeature`] (every entity with a material), [`ExposureRenderFeature`], [`BloomRenderFeature`], [`ColorGradingRenderFeature`] and
//! [`GizmosRenderFeature`].
//!
//! # How to use
//!
//! [`add_render_features_module_to`] (called by `add_defult_modules_to`) inserts the asset
//! storages (`Material`, `MaterialTemplate`, `Shader`, `ComputeShader`, `StandardMesh`, `StandardTexture`), the
//! settings resources, registers the engine features and a `Start` system in
//! [`SYSTEM_GROUP_RENDER`] that inserts the built-in shaders and material templates.
//!
//! #### Drawing an entity
//!
//! An entity is drawn when it has a [`MaterialComponent`] plus a [`MeshComponent`] (an instanced
//! `StandardMesh`), a [`BatchedMeshComponent`] (raw vertices and indices, e.g. produced by
//! `aphyn_ui`), or both. A `GlobalTransform` places it (identity when missing), and a set
//! `GlobalDisableableComponent::is_disabled` hides it. Instanced meshes are frustum culled: the
//! mesh's AABB, moved by the `GlobalTransform`, must intersect the camera frustum to be drawn into the
//! view's passes, and a shadow map tile's light frustum to be drawn into that tile (so off-screen
//! casters keep their shadows). Batched meshes and templates with `skip_frustum_culling` (the
//! built-in UI ones) are never culled. Nothing is drawn without a camera: the app
//! renders through the first [`CameraComponent`]. Lit materials are shaded by every
//! [`StandardLightComponent`] (up to `LIGHTS_COUNT_MAX`); point and spot lights sit at their
//! transform's position, spot and directional lights point along their local `-Y` axis. Point and spot
//! lights fall off with `1/d²`, eased smoothly to zero at their `range` (most of the easing happens in
//! the outer part of the range, so nothing is cut off at a cluster's edge). On top of
//! that every lit fragment receives ambient light from the view's environment map (see Skybox below):
//! diffuse along its normal and specular reflections blurred by its roughness. Without a skybox the
//! map is the single color of [`AmbientLightResource`] (linear, zero by default), so surfaces no light
//! reaches are not pitch black. A view can override that color with
//! `CurrentRenderingResource::ambient_light` (the editor's scene view does).
//!
//! #### Skybox
//!
//! [`SkyboxResource::shader`] names a `Shader` asset that draws the sky (empty by default: no sky).
//! It fills every pixel that opaque and alpha-tested geometry left empty, before transparents are
//! drawn, and also renders an `octahedral_size`² (128 by default) octahedral environment map with a
//! full mip chain, which lit materials sample for their ambient light. Both are redrawn for every
//! view, so an animated sky lights the scene as it changes. The shader builds on
//! `aphyn/skybox.wgsl` (see `aphyn_render`): it returns a color for `skybox_direction(in.ndc)`. The
//! engine ships a gradient sky as [`BUILTIN_SHADER_SKYBOX_GRADIENT`]:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn use_gradient_sky(mut skybox: ResMut<SkyboxResource>, shaders: Res<AssetStorageResource<Shader>>) {
//!     skybox.shader = shaders.get_registered(BUILTIN_SHADER_SKYBOX_GRADIENT).unwrap().clone();
//! }
//! ```
//!
//! Order such a `Start` system after [`SYSTEM_GROUP_RENDER`]. While the shader is missing or not
//! loaded, the screen is left as is and the ambient color stands in for the environment map; with a
//! sky, the ambient color is ignored by the standard materials. A custom shader reads the map from
//! `environment` / `environment_sampler` (group 0 bindings 9 and 10 in `aphyn/standard_bindings.wgsl`).
//!
//! #### Clustered lighting
//!
//! Lit fragments don't loop over every light: the view frustum is split into `CLUSTER_DIMS`
//! (16x9 screen tiles times 24 depth slices, spaced exponentially between the camera's near and far),
//! every point and spot light is assigned to the clusters its range (and, for spots, its cone)
//! touches, and a fragment shades only its cluster's lights plus every directional light.
//! [`ClusteredLightingResource::max_light_cluster_pairs`] sizes the light index list all clusters
//! share. A cluster has no cap of its own; pairs past the shared capacity are dropped (those lights go
//! dark in those clusters), which is printed once:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn make_room_for_many_lights(mut clustering: ResMut<ClusteredLightingResource>) {
//!     clustering.max_light_cluster_pairs = 1 << 20;
//! }
//! ```
//!
//! #### Shadows
//!
//! A light casts shadows when its `shadows` field is `true`. Directional lights get cascaded shadow
//! maps fitted to the view's frustum up to [`ShadowSettingsResource::distance`], spot lights one
//! perspective map over their `fov` and `range`; point lights ignore the flag for now. Every map is
//! a tile of one depth atlas ([`ShadowAtlasResource`]): `atlas_size / tile_size` squared tiles, a
//! directional light takes `cascade_count` of them, a spot light one, and a light that does not fit
//! gets no shadow (printed once). The atlas uses the same reversed depth as the view, so its
//! comparison sampler is `GreaterEqual` and the bias is added to the fragment's depth. Bias, normal offset, cascade split distribution and the other
//! knobs are global, in [`ShadowSettingsResource`]. Only templates with a
//! [`MaterialPassTarget::Shadow`](aphyn_render::MaterialPassTarget) pass cast shadows: the built-in
//! opaque, non-UI ones do (without alpha cutout); transparent and UI ones don't. A custom template
//! opts in by adding [`shadow_caster_template_pass`]:
//!
//! ```ignore
//! use aphyn::*;
//!
//! let caster = shaders.get_registered(BUILTIN_SHADER_SHADOW_CASTER).unwrap().clone();
//! template.passes.push(shadow_caster_template_pass(caster));
//! ```
//!
//! #### Depth-normals prepass
//!
//! Opaque geometry is drawn twice: at [`RENDER_ORDER_DEPTH_PREPASS`] a
//! [`MaterialPassTarget::DepthNormals`](aphyn_render::MaterialPassTarget) pass fills the view's
//! [`DepthTextureResource`] and [`SceneNormalsTextureResource`] (an `Rg16Float` texture of
//! octahedral-encoded world-space vertex normals), then the opaque pass shades with the normal it reads
//! back from group 0 binding 7 (`scene_normals` in `aphyn/standard_bindings.wgsl`) and keeps testing
//! depth with `GreaterEqual`. The built-in opaque, non-UI templates have this pass (with
//! [`BUILTIN_SHADER_DEPTH_NORMALS`], which discards like the opaque pass below `alpha_threshold`);
//! transparent and UI ones don't, and transparent lit materials shade with their own interpolated
//! normals. Pixels no prepass covered hold a cleared normal. A custom template whose shader uses the
//! standard inputs opts in with [`depth_normals_template_pass`]:
//!
//! ```ignore
//! use aphyn::*;
//!
//! let prepass = shaders.get_registered(BUILTIN_SHADER_DEPTH_NORMALS).unwrap().clone();
//! template.passes.push(depth_normals_template_pass(prepass));
//! ```
//!
//! #### Ambient occlusion
//!
//! Screen-space ambient occlusion (SSAO) is off by default. When [`SsaoResource::is_enabled`] is set, every
//! view computes it from the depth-normals prepass and opaque lit materials multiply their ambient term
//! by it; direct light, emission, unlit, UI and transparent materials are unaffected. So it shows only
//! with a nonzero ambient color ([`AmbientLightResource`] or the view's override). `radius` is the
//! view-space size of the sampled hemisphere, `bias` the view-space depth a sample must be behind the
//! scene to count, `intensity` scales the darkening and `sample_count` (at most [`SSAO_SAMPLES_MAX`])
//! trades quality for speed:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn enable_ssao(mut ssao: ResMut<SsaoResource>, mut ambient: ResMut<AmbientLightResource>) {
//!     ambient.color = Vec3::splat(0.3);
//!     ssao.is_enabled = true;
//!     ssao.radius = 0.5;
//! }
//! ```
//!
//! A custom shader reads the result from `ambient_occlusion` (group 0 binding 8 in
//! `aphyn/standard_bindings.wgsl`), which holds a 1x1 white texture while SSAO is off.
//!
//! #### Materials
//!
//! A `Material` (`aphyn_render`) references a `MaterialTemplate` asset and stores a value for every
//! input the template declares. Templates contain no code: they list inputs (with defaults) and
//! passes, each pass being a shader asset, an order, a [`MaterialPassTarget`](aphyn_render::MaterialPassTarget)
//! and the mapping from inputs to shader bindings. The engine ships six templates built on the
//! standard shader, registered under [`BUILTIN_TEMPLATE_OPAQUE_UNLIT`], [`BUILTIN_TEMPLATE_OPAQUE_LIT`],
//! [`BUILTIN_TEMPLATE_TRANSPARENT_UNLIT`], [`BUILTIN_TEMPLATE_TRANSPARENT_LIT`],
//! [`BUILTIN_TEMPLATE_UI_OPAQUE_UNLIT`] and [`BUILTIN_TEMPLATE_UI_TRANSPARENT_UNLIT`]. Their inputs are
//! `color`, `emission_color` (`Vec4`, linear), `metallic`, `roughness`, `alpha_threshold` (`F32`) and
//! the textures `color_tex`, `roughness_tex`, `metallic_tex`, `normal_tex`, `emission_tex`. The UI
//! variants draw geometry given in window pixels:
//!
//! ```ignore
//! use aphyn::*;
//!
//! let templates = res.get::<AssetStorageResource<MaterialTemplate>>().unwrap();
//! let shaders = res.get::<AssetStorageResource<Shader>>().unwrap();
//! let template = templates.get_registered(BUILTIN_TEMPLATE_OPAQUE_LIT).unwrap().clone();
//!
//! let mut material = render_api.create_material(templates, shaders, template, MaterialData::new());
//! *material.data.get_f32_mut("roughness").unwrap() = 0.8;
//!
//! let material = res.get_mut::<AssetStorageResource<Material>>().unwrap().insert(material);
//! entities.add_component(entity, MaterialComponent { material }).ok().unwrap();
//! entities.add_component(entity, MeshComponent { mesh }).ok().unwrap();
//! ```
//!
//! Order systems that create materials from the built-ins after [`SYSTEM_GROUP_RENDER`] in `Start`.
//! Changing a value through a `*_mut` accessor is enough; the material feature uploads changed
//! values before drawing.
//!
//! #### Writing and registering a feature
//!
//! A [`RenderFeature`] implements [`add_passes`](RenderFeature::add_passes), which runs once per
//! rendered view with mutable world access and the view's `ThreadPoolAccess`, and adds passes
//! through a [`RenderPassWriter`]. A pass is a `Send + 'static` closure
//! `FnOnce(&mut Self, &mut RenderPassCtx)`. Each added pass runs once, in sorted order, and gets the
//! feature back as `&mut Self` plus a [`RenderPassCtx`] holding the render API, the world and the
//! same `ThreadPoolAccess`. Passes run one after another on the calling thread; the pool is there
//! to split CPU work inside `add_passes` or a pass (e.g. `execute_work_stealing_parallel`). GPU
//! buffers and gathered lists live in the feature and are reused across frames; the closure only
//! captures small per-pass values.
//!
//! ```rust,no_run
//! use aphyn_ecs::*;
//! use aphyn_render_features::*;
//!
//! #[derive(Default)]
//! struct VignetteFeature;
//!
//! impl VignetteFeature {
//!     fn draw(&mut self, ctx: &mut RenderPassCtx) {
//!         let _api = ctx.render_api.core();
//!     }
//! }
//!
//! impl RenderFeature for VignetteFeature {
//!     fn add_passes(&mut self, _world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
//!         passes.add(RENDER_ORDER_POST_PROCESSING, |this, ctx| this.draw(ctx));
//!     }
//! }
//!
//! let mut registry = RenderFeatureRegistry::default();
//! assert!(registry.add_feature("vignette", VignetteFeature));
//! assert!(!registry.add_feature("vignette", VignetteFeature)); // the name is taken
//! ```
//!
//! #### Orders
//!
//! Orders are absolute. The stages are [`RENDER_ORDER_PREPARE`], [`RENDER_ORDER_ENVIRONMENT`], [`RENDER_ORDER_DEPTH_PREPASS`],
//! [`RENDER_ORDER_AMBIENT_OCCLUSION`], [`RENDER_ORDER_SHADOW`], [`RENDER_ORDER_LIGHT_CULLING`], [`RENDER_ORDER_OPAQUE`], [`RENDER_ORDER_SKYBOX`],
//! [`RENDER_ORDER_TRANSPARENT`], [`RENDER_ORDER_TRANSPARENT_COMPOSITE`] and
//! [`RENDER_ORDER_POST_PROCESSING`], followed by the engine's [`RENDER_ORDER_EXPOSURE`],
//! [`RENDER_ORDER_BLOOM`], [`RENDER_ORDER_COLOR_GRADING`] and [`RENDER_ORDER_GIZMOS`]. Passes with
//! equal order run in no defined relative order.
//!
//! #### Rendering a view
//!
//! The owner of the world calls [`render_view`](RenderFeatureRegistry::render_view) with the world,
//! a [`CurrentRenderingResource`](aphyn_render::CurrentRenderingResource) describing the camera
//! and the color target, and the `ThreadPoolAccess` handed to the features (the one the world's
//! schedules run on, or `ThreadPoolAccess::single_threaded()`), any number of times per frame.
//! Passes read the camera and target from that resource, which exists only during the call.
//!
//! #### Post-processing and gizmos
//!
//! Exposure, bloom and color grading are off by default; enable them through [`ExposureResource`],
//! [`BloomResource`] and [`ColorGradingResource`]. [`GizmosResource`] collects lines per
//! `RenderSpace` for the current frame; push them every frame they should appear.
//!
//! # How to maintain
//!
//! #### `render_view` steps
//!
//! Insert the view as `CurrentRenderingResource`, clone the world's `RenderApiResource` (one boxed
//! `Arc` per call), `collect_passes` (clear the list, call every slot's `add_passes` in registration
//! order, unstable-sort by order), call every entry's closure, clear the closures, remove the
//! resource. Without a `RenderApiResource` nothing is collected or run.
//!
//! #### Globals feature
//!
//! [`GlobalsRenderFeature`] works in `add_passes`: it inserts [`GlobalBindingsResource`] on first
//! use, uploads the lights of every `StandardLightComponent` plus `CurrentRenderingResource::light`
//! and the `StandardUniformGlobal` (world-to-clip of the view, window-to-clip for the target size
//! with the [`ScreenSpaceResource`] depth range, `CurrentRenderingResource::ambient_light` when set,
//! else the [`AmbientLightResource`] color, else zero), and replaces [`DepthTextureResource`],
//! [`SceneNormalsTextureResource`] and [`TransparentTargetResource`] when their size differs from the
//! target. Its pass at [`RENDER_ORDER_PREPARE`] clears all three (depth, like the shadow atlas, to 0: depth is reversed, 1 is
//! near and 0 is far, so every engine shader tests with `GreaterEqual`). The uploads are queued writes that land at the next
//! submit, which is before any draw. Lights are uploaded directional first; the uniform's
//! `directional_lights_count` marks where the clustered (point and spot) lights start. It also
//! applies [`ClusteredLightingResource::max_light_cluster_pairs`] to the bindings before writing the
//! uniform, which carries the resulting index capacity.
//!
//! It also lays out the view's shadows (`shadows.rs`): it inserts [`ShadowAtlasResource`] on first
//! use (a 1x1 atlas until a shadow is needed, then `atlas_size`), assigns each shadowed light its
//! tiles while gathering lights (`shadow_start`/`shadow_count` of `StandardGenericLight`), uploads
//! the `StandardShadowView`s (world-to-clip, atlas uv rect, cascade far depth) and writes each
//! view's world-to-clip into its own small caster uniform buffer. Directional cascades split
//! `[camera.near, min(camera.far, distance)]` by blending uniform and logarithmic splits with
//! `split_lambda`, bound each slice's corners (unprojected through the view's clip-to-world) by a
//! sphere of rounded radius and snap its center to the tile's texel grid, so the map neither
//! resizes nor shimmers while the camera moves; the ortho box reaches `caster_distance` behind the
//! slice. Light bases come from `look_rotation` (a proper rotation, so back-face culling holds).
//! Shadows depend on the camera, so they are rebuilt per view. The `PREPARE` pass also clears the
//! atlas when any view uses it.
//!
//! #### Clustering feature
//!
//! [`ClusteringRenderFeature`] runs after the globals feature and adds one pass at
//! [`RENDER_ORDER_LIGHT_CULLING`]. It resets the dropped-pairs counter and dispatches three compute
//! shaders sharing `aphyn/cluster_compute.wgsl` (the cluster math itself is `aphyn/clusters.wgsl` in
//! the render crate):
//!
//! 1. `cluster_prefix_count.wgsl`, one thread per cluster: counts the lights touching it,
//! 2. `cluster_prefix_scan.wgsl`, a single workgroup of 256 threads: each thread sums a contiguous
//!    chunk of clusters, the chunk sums are scanned in shared memory, then each thread writes its
//!    chunk's offsets; counts that don't fit into the index list are clamped and counted as dropped,
//! 3. `cluster_prefix_fill.wgsl`, one thread per cluster: repeats the tests and writes the lights at
//!    the cluster's offset.
//!
//! Each dispatch submits on its own, so every pass sees the previous one's writes. The result is
//! `cluster_grid[cluster] = { index: offset into cluster_light_indices, count }`. A cluster's box is the
//! view-space AABB of its frustum chunk, unprojected through the uniform's clip-to-view; the first and
//! last slices reach the camera's own near and far, because fragments outside
//! `[cluster_near, cluster_far]` are clamped into them. A light is a view-space sphere (its range
//! scaled like the view), and a spot is additionally culled with a cone-vs-sphere test against the
//! box's bounding sphere. Every 120th frame, until it has printed once, the pass reads the dropped
//! pairs back (a blocking GPU wait).
//!
//! #### SSAO feature
//!
//! [`SsaoRenderFeature`] is registered before the material feature, because its `add_passes` sets
//! [`AmbientOcclusionTextureResource`], which every material draw binds at group 0 binding 8: a 1x1
//! texture cleared to white once when [`SsaoResource`] is off or missing (no pass is added), else the
//! feature's target-size `Rgba8Unorm` texture (only `.x` is used), recreated when the size changes. The
//! standard shader clamps its pixel to the texture's size, so both read correctly. The pass at
//! [`RENDER_ORDER_AMBIENT_OCCLUSION`] runs after the depth-normals prepass and draws three fullscreen
//! triangles sharing `aphyn/ssao.wgsl` and one `SsaoUniform` (camera matrices, target size, settings):
//!
//! 1. `shader_ssao.wgsl` rebuilds the view-space position from depth through `clip_to_view` (so both
//!    projections work; view space looks down `+z`) and the normal from the scene normals, rotates a
//!    fixed 64-sample hemisphere kernel (generated once on the CPU, denser near the center) by a
//!    per-pixel interleaved-gradient-noise angle, and counts the samples the scene's depth is in front of,
//!    faded by distance against `radius`. Pixels no prepass covered (depth 0) stay 1,
//! 2. `shader_ssao_blur.wgsl` blurs horizontally into the feature's temp texture (instance 0), then
//!    vertically back (instance 1), weighting 9 taps by view-depth difference so occlusion doesn't
//!    bleed across edges.
//!
//! #### Skybox feature
//!
//! [`SkyboxRenderFeature`] is registered before the material feature and, like SSAO, sets a resource
//! every material draw binds: [`EnvironmentTextureResource`] (texture plus a `Linear` sampler, whose
//! mipmap filter is linear too). When [`SkyboxResource::shader`] resolves in `AssetStorageResource<Shader>`
//! it is the feature's `Rgba16Float` octahedral texture (recreated when `octahedral_size` changes, full
//! mip chain), and two passes are added:
//!
//! 1. at [`RENDER_ORDER_ENVIRONMENT`] (right after `PREPARE`, before anything samples it) the shader
//!    draws mip 0 with `is_octahedral = 1` and no depth, then `RenderApiResource::generate_mipmaps`
//!    box-filters the chain (no GGX prefilter; small seams at the octahedral folds are accepted),
//! 2. at [`RENDER_ORDER_SKYBOX`] it draws into the view's target with [`DepthTextureResource`] loaded,
//!    `is_octahedral = 0` and the view's clip-to-world. `skybox_vertex` puts the triangle at depth 0,
//!    the cleared (reversed) far value, so the shader's own `GreaterEqual` test passes exactly where
//!    nothing was drawn; no special `GpuShaderDesc` is needed.
//!
//! Each pass has its own small `SkyboxUniform` buffer. Otherwise (no resource, empty handle, not
//! loaded) the resource gets a 1x1 texture cleared every view to the view's ambient color (the same
//! rule as the globals feature) and no pass is added; a single level samples the same at any lod, so
//! the standard shader's IBL reduces to that flat color.
//!
//! #### Material feature
//!
//! [`MaterialRenderFeature::add_passes`]:
//!
//! 1. gathers one item per mesh component of every enabled entity whose material resolves
//!    (template handle, material, instanced mesh handle or a raw pointer to the
//!    `BatchedMeshComponent`, `local_to_world`, visibility) and sorts them by `(template, material, instanced
//!    before batched, mesh)`. Visibility is `in_camera` plus a `shadow_mask` bit per
//!    `ShadowAtlasResource` view (at most `SHADOW_VIEWS_MAX` = 64, so a `u64`), tested with
//!    `aphyn_math::Frustum`s built from `CurrentRenderingResource::world_to_clip` and each view's
//!    `world_to_clip` (the globals feature is registered first, so the views are already laid out).
//!    Shadow bits are only set when the template has a `Shadow` pass. Items with nothing to test
//!    (batched, `skip_frustum_culling`, mesh or template not loaded) are visible everywhere; items
//!    visible nowhere are dropped,
//! 2. calls `RenderApiResource::upload_material_changes` once per material (snapshot-driven),
//! 3. uploads the frame's geometry: per `(material, mesh)` run the camera-visible instance matrices,
//!    then, when any item of the run casts, one compacted range per shadow view (kept in
//!    `shadow_ranges`), into one growing `StandardInstanceBuffer` sized for all ranges; per material the CPU-transformed batched vertices (vertex
//!    colors are taken as linear and copied as is) into one growing vertex buffer. All ranges of the frame
//!    coexist in the buffers, so passes may run in any order,
//! 4. builds one draw per (template pass, geometry run) whose material has GPU data for that pass;
//!    a material whose template does not resolve or whose GPU data was never built is skipped and
//!    printed once (tracked in `logged`),
//! 5. stable-sorts the draws by order and adds one pass closure per distinct order, plus the
//!    transparent composite at [`RENDER_ORDER_TRANSPARENT_COMPOSITE`] when any pass targets
//!    `Transparent`.
//!
//! A draw binds group 0 to the globals (uniform at 0, lights at 1, shadow views at 2, shadow atlas at
//! 3, its comparison sampler at 4, cluster grid at 5, cluster light indices at 6, scene normals at 7 except
//! for `DepthNormals` passes, which render into them, [`AmbientOcclusionTextureResource`] at 8, and [`EnvironmentTextureResource`] at 9 (texture) and 10 (sampler)), the material's uniform buffers
//! at their mapped `(group, binding)`, and every texture input resolved from
//! `AssetStorageResource<StandardTexture>` on each draw, falling back to the input's
//! `MaterialTextureFallback` texture of `RenderApiResource`. It draws into the view's target
//! (`Main`), [`TransparentTargetResource`] (`Transparent`, composited with alpha blending by
//! `shader_transparent_final.wgsl`) or [`SceneNormalsTextureResource`] (`DepthNormals`), always with
//! [`DepthTextureResource`]. A `Shadow` pass instead
//! repeats the draw once per shadow view with no color target, the atlas as depth, the tile as
//! viewport, that view's instance range and only that view's caster uniform at group 0 binding 0.
//! Other passes draw the camera range; empty ranges issue no draw. The caster shader declares no
//! other group 0 binding, so the atlas is never sampled by the pass that renders into it. Batched draws pass a
//! one-element identity instance buffer. The batched pointers are only dereferenced inside
//! `add_passes`, while no entity can change; the item vec is cleared before returning.
//!
//! #### Built-ins
//!
//! [`insert_builtin_render_assets`] composes `aphyn_render::STANDARD_SHADER_WGSL` with the defines
//! `LIT` / `TRANSPARENT` / `UI` per variant, creates the shaders with [`standard_shader_desc`] and
//! registers them as `aphyn/shaders/<variant>`, then registers [`standard_material_template`] for
//! each as `aphyn/templates/<variant>`. Before that it registers the depth-only
//! `aphyn_render::SHADOW_CASTER_SHADER_WGSL` as [`BUILTIN_SHADER_SHADOW_CASTER`] and the standard shader
//! composed with `DEPTH_NORMALS` as [`BUILTIN_SHADER_DEPTH_NORMALS`], and `aphyn_render::SKYBOX_GRADIENT_SHADER_WGSL`
//! as [`BUILTIN_SHADER_SKYBOX_GRADIENT`] with the fullscreen desc (each once), and the opaque,
//! non-UI templates get a pass with each, at [`RENDER_ORDER_SHADOW`] and [`RENDER_ORDER_DEPTH_PREPASS`].
//! The standard pass and [`depth_normals_template_pass`] share one input-to-binding mapping
//! (`standard_template_pass`), so the prepass sees the same material data. It needs the `RenderApiResource`, so it runs in `Start`; asset
//! loading must run after it so material files can reference the built-in keys. A variant whose
//! template key is already registered is skipped: registering it again would leave the handles that
//! loaded materials hold unregistered, so they would save with an empty template.
//! [`insert_builtin_render_assets_to`] does the same on a resources holder, for worlds that load
//! assets before their `Start` pass (the editor's edit-mode world).
//!
//! #### Post-processing and gizmos
//!
//! Each post-processing feature reads its settings resource in `add_passes`, adds no pass when the
//! effect is off, and creates its shaders, samplers and uniform buffers lazily in its pass,
//! recreating its intermediate textures when the target size changes. Exposure and color grading
//! draw into their own texture and copy it back onto the target; bloom gathers pixels above the
//! threshold, then per layer downscales, blurs both directions and adds the layer back.
//! [`prepare_gizmos`] (an `Update` system in [`SYSTEM_GROUP_RENDER`]) moves the user's lines into
//! [`GizmosFrameResource`] once per frame, so every view of the frame draws them;
//! [`GizmosRenderFeature`] draws them in chunks of [`GIZMO_LINES_PER_DRAW_MAX`]. Fullscreen draws
//! and gizmo chunks rewrite shared buffers between draws, which is correct only because core
//! submits every draw on its own.
//!
//! #### Slot erasure
//!
//! The registry is `#[repr(C)]` and holds only `aphyn_ffi` types, because a game library
//! registers features into a registry owned by the host. Each feature becomes a `RenderFeatureFfi`,
//! an `FfiDroppable` box holding a `SlotState { value, types }` plus a `&'static` vtable with an
//! `extern "C-unwind"` `add_passes`, monomorphized on the side that called `add_feature`. The
//! registry owns one `FfiFnOnceVec<RenderPassInput, ()>` of pass closures; a [`RenderPassEntry`] is
//! `(order, slot, index)`. `RenderPassWriter::add` wraps the user closure in one taking a by-value
//! `RenderPassInput`, monomorphized in the registering binary, that casts the state pointer back to
//! `SlotState<T>` and rebuilds the `WorldDataMut` from the slot's types cache. A slot builds that
//! cache from the first world it sees, so a registry is paired with one world for its whole life.
//! `ThreadPoolAccess` is `#[repr(C)]` and crosses the vtable by value; `RenderPassInput` stores it
//! lifetime-extended to `'static`, which holds because every pass runs before `render_view` returns.
//!
//! #### Allocation-free pass list
//!
//! The entry list, the closure vec, the material feature's items, geometry, shadow frusta and ranges, matrix scratch and draws, the globals
//! feature's light lists and the atlas's shadow views are cleared, not dropped, so they keep their capacity. Known exceptions: the ECS queries of the
//! material and globals features allocate every frame (`// todo` at the queries), the material
//! feature allocates its bind group lists per draw (`// todo`), and `render_view`, the globals, SSAO and
//! the material features clone `RenderApiResource` once per view.
//!
//! #### Dynamic library drop order
//!
//! A slot's drop function and vtable, and its pass closures, live in the binary that registered
//! the feature. A registry holding slots from a game library must be dropped before that library
//! is unloaded.

mod builtins;
mod clustering;
mod components;
mod feature;
mod gizmos;
mod globals;
mod material;
mod orders;
mod pass_list;
mod post_processing;
mod registry;
mod resources;
mod shadows;
mod skybox;
mod ssao;
mod utils;

pub use builtins::*;
pub use clustering::ClusteringRenderFeature;
pub use components::*;
pub use feature::*;
pub use gizmos::*;
pub use globals::*;
pub use material::*;
pub use orders::*;
pub use pass_list::*;
pub use post_processing::*;
pub use registry::*;
pub use resources::*;
pub use shadows::ShadowAtlasResource;
pub use skybox::SkyboxRenderFeature;
pub use ssao::SsaoRenderFeature;
pub use utils::*;

use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::{Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_render::{ComputeShader, Material, MaterialTemplate, Shader, StandardMesh, StandardTexture};

pub const SYSTEM_GROUP_RENDER: &str = "aphyn_render";

pub const RENDER_FEATURE_GLOBALS: &str = "globals";
pub const RENDER_FEATURE_CLUSTERING: &str = "clustering";
pub const RENDER_FEATURE_SSAO: &str = "ssao";
pub const RENDER_FEATURE_SKYBOX: &str = "skybox";
pub const RENDER_FEATURE_MATERIAL: &str = "material";
pub const RENDER_FEATURE_EXPOSURE: &str = "exposure";
pub const RENDER_FEATURE_BLOOM: &str = "bloom";
pub const RENDER_FEATURE_COLOR_GRADING: &str = "color_grading";
pub const RENDER_FEATURE_GIZMOS: &str = "gizmos";

// todo: bundle registration params
pub fn add_render_features_module_to(
    mut data: WorldDataMut,
    mut behavior: WorldBehaviorBuilderMut,
    render: &mut RenderFeatureRegistry,
) {
    let mut resources = data.resources_mut();
    resources.insert(AssetStorageResource::<Material>::new());
    resources.insert(AssetStorageResource::<MaterialTemplate>::new());
    resources.insert(AssetStorageResource::<Shader>::new());
    resources.insert(AssetStorageResource::<ComputeShader>::new());
    resources.insert(AssetStorageResource::<StandardMesh>::new());
    resources.insert(AssetStorageResource::<StandardTexture>::new());
    resources.insert(GizmosResource::default());
    resources.insert(GizmosFrameResource::default());
    resources.insert(ScreenSpaceResource::default());
    resources.insert(AmbientLightResource::default());
    resources.insert(ShadowSettingsResource::default());
    resources.insert(ExposureResource::default());
    resources.insert(BloomResource::default());
    resources.insert(ColorGradingResource::default());
    resources.insert(ClusteredLightingResource::default());
    resources.insert(SsaoResource::default());
    resources.insert(SkyboxResource::default());

    behavior
        .get_mut(Schedule::Start)
        .group(SYSTEM_GROUP_RENDER)
        .insert_child_system(insert_builtin_render_assets);
    behavior
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP_RENDER)
        .insert_child_system(prepare_gizmos);

    render.add_feature(RENDER_FEATURE_GLOBALS, GlobalsRenderFeature::default());
    render.add_feature(RENDER_FEATURE_CLUSTERING, ClusteringRenderFeature::default());
    render.add_feature(RENDER_FEATURE_SSAO, SsaoRenderFeature::default());
    render.add_feature(RENDER_FEATURE_SKYBOX, SkyboxRenderFeature::default());
    render.add_feature(RENDER_FEATURE_MATERIAL, MaterialRenderFeature::default());
    render.add_feature(RENDER_FEATURE_EXPOSURE, ExposureRenderFeature::default());
    render.add_feature(RENDER_FEATURE_BLOOM, BloomRenderFeature::default());
    render.add_feature(RENDER_FEATURE_COLOR_GRADING, ColorGradingRenderFeature::default());
    render.add_feature(RENDER_FEATURE_GIZMOS, GizmosRenderFeature::default());
}
