//! # aphyn_render_features
//!
//! The engine's rendering pipeline: render features, the per-view pass list, data-driven
//! materials, lights, gizmos and post-processing. A [`RenderFeatureRegistry`] owns the render
//! features of one world. Every [`render_view`](RenderFeatureRegistry::render_view) asks each
//! feature for its passes, sorts them by an absolute `u64` order, and runs them one after another
//! on the calling thread. Everything the engine draws is such a feature: [`GlobalsRenderFeature`]
//! (camera, lights, depth and transparent targets), [`MaterialRenderFeature`] (every entity with a
//! material), [`ExposureRenderFeature`], [`BloomRenderFeature`], [`ColorGradingRenderFeature`] and
//! [`GizmosRenderFeature`].
//!
//! # How to use
//!
//! [`add_render_features_module_to`] (called by `add_defult_modules_to`) inserts the asset
//! storages (`Material`, `MaterialTemplate`, `Shader`, `StandardMesh`, `StandardTexture`), the
//! settings resources, registers the engine features and a `Start` system in
//! [`SYSTEM_GROUP_RENDER`] that inserts the built-in shaders and material templates.
//!
//! #### Drawing an entity
//!
//! An entity is drawn when it has a [`MaterialComponent`] plus a [`MeshComponent`] (an instanced
//! `StandardMesh`), a [`BatchedMeshComponent`] (raw vertices and indices, e.g. produced by
//! `aphyn_ui`), or both. A `GlobalTransform` places it (identity when missing), and a set
//! `GlobalDisableableComponent::is_disabled` hides it. Nothing is drawn without a camera: the app
//! renders through the first [`CameraComponent`]. Lit materials are shaded by every
//! [`StandardLightComponent`] (up to `LIGHTS_COUNT_MAX`); point and spot lights sit at their
//! transform's position, spot and directional lights point along their local `-Y` axis.
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
//! rendered view with mutable world access and adds passes through a [`RenderPassWriter`]. A pass
//! is a `Send + 'static` closure `FnOnce(&mut Self, &mut RenderPassCtx)`. Each added pass runs once,
//! in sorted order, and gets the feature back as `&mut Self` plus a [`RenderPassCtx`] holding the
//! render API and the world. GPU buffers and gathered lists live in the feature and are reused
//! across frames; the closure only captures small per-pass values.
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
//!     fn add_passes(&mut self, _world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
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
//! Orders are absolute. The stages are [`RENDER_ORDER_PREPARE`], [`RENDER_ORDER_DEPTH_PREPASS`],
//! [`RENDER_ORDER_SHADOW`], [`RENDER_ORDER_OPAQUE`], [`RENDER_ORDER_SKYBOX`],
//! [`RENDER_ORDER_TRANSPARENT`], [`RENDER_ORDER_TRANSPARENT_COMPOSITE`] and
//! [`RENDER_ORDER_POST_PROCESSING`], followed by the engine's [`RENDER_ORDER_EXPOSURE`],
//! [`RENDER_ORDER_BLOOM`], [`RENDER_ORDER_COLOR_GRADING`] and [`RENDER_ORDER_GIZMOS`]. Passes with
//! equal order run in no defined relative order.
//!
//! #### Rendering a view
//!
//! The owner of the world calls [`render_view`](RenderFeatureRegistry::render_view) with the world
//! and a [`CurrentRenderingResource`](aphyn_render::CurrentRenderingResource) describing the camera
//! and the color target, any number of times per frame. Passes read both from that resource, which
//! exists only during the call.
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
//! with the [`ScreenSpaceResource`] depth range), and replaces [`DepthTextureResource`] and
//! [`TransparentTargetResource`] when their size differs from the target. Its pass at
//! [`RENDER_ORDER_PREPARE`] clears both. The uploads are queued writes that land at the next
//! submit, which is before any draw.
//!
//! #### Material feature
//!
//! [`MaterialRenderFeature::add_passes`]:
//!
//! 1. gathers one item per mesh component of every enabled entity whose material resolves
//!    (template handle, material, instanced mesh handle or a raw pointer to the
//!    `BatchedMeshComponent`, `local_to_world`) and sorts them by `(template, material, instanced
//!    before batched, mesh)`,
//! 2. calls `RenderApiResource::upload_material_changes` once per material (snapshot-driven),
//! 3. uploads the frame's geometry: per `(material, mesh)` run the instance matrices into one
//!    growing `StandardInstanceBuffer`, per material the CPU-transformed batched vertices (vertex
//!    colors converted from sRGB to linear) into one growing vertex buffer. All ranges of the frame
//!    coexist in the buffers, so passes may run in any order,
//! 4. builds one draw per (template pass, geometry run) whose material has GPU data for that pass;
//!    a material whose template does not resolve or whose GPU data was never built is skipped and
//!    printed once (tracked in `logged`),
//! 5. stable-sorts the draws by order and adds one pass closure per distinct order, plus the
//!    transparent composite at [`RENDER_ORDER_TRANSPARENT_COMPOSITE`] when any pass targets
//!    `Transparent`.
//!
//! A draw binds group 0 to the globals (uniform at 0, lights at 1), the material's uniform buffers
//! at their mapped `(group, binding)`, and every texture input resolved from
//! `AssetStorageResource<StandardTexture>` on each draw, falling back to the input's
//! `MaterialTextureFallback` texture of `RenderApiResource`. It draws into the view's target
//! (`Main`) or [`TransparentTargetResource`] (`Transparent`, composited with alpha blending by
//! `shader_transparent_final.wgsl`), always with [`DepthTextureResource`]. Batched draws pass a
//! one-element identity instance buffer. The batched pointers are only dereferenced inside
//! `add_passes`, while no entity can change; the item vec is cleared before returning.
//!
//! #### Built-ins
//!
//! [`insert_builtin_render_assets`] composes `aphyn_render::STANDARD_SHADER_WGSL` with the defines
//! `LIT` / `TRANSPARENT` / `UI` per variant, creates the shaders with [`standard_shader_desc`] and
//! registers them as `aphyn/shaders/<variant>`, then registers [`standard_material_template`] for
//! each as `aphyn/templates/<variant>`. It needs the `RenderApiResource`, so it runs in `Start`; asset
//! loading must run after it so material files can reference the built-in keys.
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
//!
//! #### Allocation-free pass list
//!
//! The entry list, the closure vec, and the material feature's items, geometry and draws are
//! cleared, not dropped, so they keep their capacity. Known exceptions: the ECS queries of the
//! material and globals features allocate every frame (`// todo` at the queries), the material
//! feature allocates its bind group lists per draw (`// todo`), and `render_view`, the globals and
//! the material features clone `RenderApiResource` once per view.
//!
//! #### Dynamic library drop order
//!
//! A slot's drop function and vtable, and its pass closures, live in the binary that registered
//! the feature. A registry holding slots from a game library must be dropped before that library
//! is unloaded.

mod builtins;
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
mod utils;

pub use builtins::*;
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
pub use utils::*;

use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::{Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_render::{Material, MaterialTemplate, Shader, StandardMesh, StandardTexture};

pub const SYSTEM_GROUP_RENDER: &str = "aphyn_render";

pub const RENDER_FEATURE_GLOBALS: &str = "globals";
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
    resources.insert(AssetStorageResource::<StandardMesh>::new());
    resources.insert(AssetStorageResource::<StandardTexture>::new());
    resources.insert(GizmosResource::default());
    resources.insert(GizmosFrameResource::default());
    resources.insert(ScreenSpaceResource::default());
    resources.insert(ExposureResource::default());
    resources.insert(BloomResource::default());
    resources.insert(ColorGradingResource::default());

    behavior
        .get_mut(Schedule::Start)
        .group(SYSTEM_GROUP_RENDER)
        .insert_child_system(insert_builtin_render_assets);
    behavior
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP_RENDER)
        .insert_child_system(prepare_gizmos);

    render.add_feature(RENDER_FEATURE_GLOBALS, GlobalsRenderFeature);
    render.add_feature(RENDER_FEATURE_MATERIAL, MaterialRenderFeature::default());
    render.add_feature(RENDER_FEATURE_EXPOSURE, ExposureRenderFeature::default());
    render.add_feature(RENDER_FEATURE_BLOOM, BloomRenderFeature::default());
    render.add_feature(RENDER_FEATURE_COLOR_GRADING, ColorGradingRenderFeature::default());
    render.add_feature(RENDER_FEATURE_GIZMOS, GizmosRenderFeature::default());
}
