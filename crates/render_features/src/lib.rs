//! # aphyn_render_features
//!
//! Render features, material templates and the per-view pass list. A [`RenderRegistry`] owns the
//! render features and material templates of one world. Every
//! [`render_view`](RenderRegistry::render_view) asks each template and feature for its passes,
//! sorts them by an absolute `u64` order, and runs them one after another on the calling thread.
//!
//! # How to use
//!
//! #### Writing and registering a feature
//!
//! A [`RenderFeature`] has two halves. [`add_passes`](RenderFeature::add_passes) runs once per
//! rendered view with mutable world access and adds passes through a [`RenderPassWriter`].
//! [`run_pass`](RenderFeature::run_pass) runs once per added pass, in sorted order, with a
//! [`RenderPassCtx`] holding the render API and the world. A pass is not a closure. It is a value
//! of the feature's own `Pass` type, and the feature gets `&mut self` back when the pass runs. GPU
//! buffers, cached bind groups and gathered entity lists therefore live in the feature and are
//! reused across frames.
//!
//! ```rust,no_run
//! use aphyn_ecs::*;
//! use aphyn_render_features::*;
//!
//! #[derive(Resource)]
//! struct BloomSettings {
//!     enabled: bool,
//! }
//!
//! enum BloomPass {
//!     Downsample(u8),
//!     Composite,
//! }
//!
//! #[derive(Default)]
//! struct BloomFeature;
//!
//! impl RenderFeature for BloomFeature {
//!     type Pass = BloomPass;
//!
//!     fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<BloomPass>) {
//!         let enabled = world.resources().get::<BloomSettings>().is_some_and(|s| s.enabled);
//!         if !enabled {
//!             return;
//!         }
//!
//!         // Equal orders have no defined relative order, so each step gets its own.
//!         for mip in 0..4 {
//!             passes.add(RENDER_ORDER_POST_PROCESSING + mip as u64, BloomPass::Downsample(mip));
//!         }
//!         passes.add(RENDER_ORDER_POST_PROCESSING + 4, BloomPass::Composite);
//!     }
//!
//!     fn run_pass(&mut self, pass: &BloomPass, ctx: &mut RenderPassCtx) {
//!         let _api = ctx.render_api.core();
//!         match pass {
//!             BloomPass::Downsample(_mip) => {}
//!             BloomPass::Composite => {}
//!         }
//!     }
//! }
//!
//! let mut registry = RenderRegistry::default();
//! assert!(registry.add_feature("bloom", BloomFeature));
//! assert!(!registry.add_feature("bloom", BloomFeature)); // the name is taken
//! ```
//!
//! [`add_feature`](RenderRegistry::add_feature) returns `false` and changes nothing when the
//! name is already taken.
//!
//! #### Rendering a view
//!
//! The owner of the world calls [`render_view`](RenderRegistry::render_view) with a
//! [`CurrentRenderingResource`](aphyn_render::CurrentRenderingResource) that describes the
//! camera and the color target. It may do so any number of times per frame, for different
//! targets. Passes read the camera and the target from that resource, which exists only during
//! the call:
//!
//! ```ignore
//! registry.render_view(&mut world, CurrentRenderingResource {
//!     camera_position,
//!     camera_scale_rotation,
//!     camera,
//!     light: None,
//!     target: target_texture.clone(),
//! });
//! ```
//!
//! #### Orders
//!
//! Orders are absolute. The engine stages are [`RENDER_ORDER_DEPTH_PREPASS`],
//! [`RENDER_ORDER_SHADOW`], [`RENDER_ORDER_OPAQUE`], [`RENDER_ORDER_SKYBOX`],
//! [`RENDER_ORDER_TRANSPARENT`] and [`RENDER_ORDER_POST_PROCESSING`], one million apart. A custom
//! pass picks any order equal to, near or far from them. Passes with equal order run in no
//! defined relative order.
//!
//! #### Sharing GPU resources between features
//!
//! Features never reach into each other. A GPU resource used by several features (a shadow map
//! sampled by the opaque pass, an HDR target read by post-processing) is an ECS resource. The
//! feature that creates it inserts it into the world from `add_passes`, and the others read it
//! from `RenderPassCtx::world`.
//!
//! #### Drawing entities with materials
//!
//! An entity is drawn when it has a [`MeshComponent`], a [`MaterialComponent`] and a
//! `GlobalTransform`. The material is an [`aphyn_render::Material`] in
//! `AssetStorageResource<Material>` (inserted by [`add_render_features_module_to`], which
//! `add_defult_modules_to` calls): a template name plus a
//! [`MaterialData`](aphyn_render::MaterialData) map of named values. Create one from a
//! registered template with [`create_material`](RenderRegistry::create_material), which fills
//! the data with the template's defaults. [`template_default_data`](RenderRegistry::template_default_data) returns those
//! defaults alone (the editor uses it to show a template's fields). Both return `None` for a name
//! that is unknown or belongs to a feature.
//!
//! ```ignore
//! let material = registry.create_material("water").unwrap();
//! let material = world.resources_mut().get_mut::<AssetStorageResource<Material>>().unwrap().insert(material);
//! entities.add_component(entity, MaterialComponent { material }).ok().unwrap();
//! entities.add_component(entity, MeshComponent { mesh }).ok().unwrap();
//! ```
//!
//! A material whose template is not registered is skipped, and
//! `unknown material template '<name>'` is printed to stderr once per material.
//!
//! #### Writing a material template
//!
//! A [`MaterialTemplate`] draws every material that names it. It is registered with
//! [`add_material_template`](RenderRegistry::add_material_template), which shares one name space
//! with features: a name taken by either is rejected with `false`. Like a feature it adds passes
//! and runs them, but its `add_passes` also gets the frame's [`MaterialBatch`]es for that template:
//! runs of instances with the same material and mesh, sorted by material then mesh. It is called
//! every rendered view, with an empty slice when nothing uses the template. Every batch names a
//! range in the instance buffer of [`MaterialInstancesResource`], which holds the
//! `local_to_world` matrix of every drawn entity. The resource exists once a frame had instances
//! and the world had a `RenderApiResource`.
//!
//! [`default_data`](MaterialTemplate::default_data) returns the template's own defaults (water
//! starts blue, not white). Read a value with the `get_*_or` accessors of `MaterialData`: a missing
//! entry, or one with another value type, falls back to the default data. A batch's mesh handle may
//! not resolve (the material step does not check meshes); skip such a batch.
//!
//! ```rust,no_run
//! use aphyn_asset_storage::AssetStorageResource;
//! use aphyn_ecs::*;
//! use aphyn_math::Vec4;
//! use aphyn_render::*;
//! use aphyn_render_features::*;
//!
//! struct WaterTemplate {
//!     defaults: MaterialData,
//! }
//!
//! struct WaterPass(MaterialBatch);
//!
//! impl MaterialTemplate for WaterTemplate {
//!     type Pass = WaterPass;
//!
//!     fn default_data(&self) -> MaterialData {
//!         let mut data = MaterialData::new();
//!         data.insert("color", MaterialDataValue::Vec4(Vec4::new(0.1, 0.3, 0.8, 1.0)));
//!         data.insert("wave_speed", MaterialDataValue::F32(0.3));
//!         data
//!     }
//!
//!     fn add_passes(&mut self, _world: WorldDataMut, batches: &[MaterialBatch], passes: &mut RenderPassWriter<WaterPass>) {
//!         for batch in batches {
//!             passes.add(RENDER_ORDER_OPAQUE, WaterPass(batch.clone()));
//!         }
//!     }
//!
//!     fn run_pass(&mut self, pass: &WaterPass, ctx: &mut RenderPassCtx) {
//!         let resources = ctx.world.resources();
//!         let Some(mesh) = resources.get::<AssetStorageResource<StandardMesh>>().and_then(|s| s.get(&pass.0.mesh)) else {
//!             return;
//!         };
//!         let Some(material) = resources.get::<AssetStorageResource<Material>>().and_then(|s| s.get(&pass.0.material)) else {
//!             return;
//!         };
//!         let Some(instances) = resources.get::<MaterialInstancesResource>() else {
//!             return;
//!         };
//!
//!         let _wave_speed = material.data.get_f32_or("wave_speed", &self.defaults);
//!         let _range = pass.0.instance_start..pass.0.instance_start + pass.0.instance_count;
//!         let _ = (mesh, &instances.buffer); // draw `_range` of the instances with the mesh
//!     }
//! }
//!
//! let template = WaterTemplate { defaults: MaterialData::new() };
//! let template = WaterTemplate { defaults: template.default_data() };
//!
//! let mut registry = RenderRegistry::default();
//! assert!(registry.add_material_template("water", template));
//! let water = registry.create_material("water").unwrap();
//! assert_eq!(water.data.get_f32("wave_speed"), Some(&0.3));
//! ```
//!
//! A template owns the GPU data of the materials it draws (uniform buffers, bind groups), cached
//! per material handle. Every mutable access to a `MaterialData` entry bumps its
//! [`snapshot`](aphyn_render::MaterialDataEntry::snapshot), so the template remembers the last
//! snapshot it uploaded per entry and rewrites only entries whose snapshot changed. Reloading a
//! material asset under its key (an inspector edit) replaces the stored `Material` but carries
//! the snapshots forward, so the same check picks the change up.
//!
//! # How to maintain
//!
//! #### `render_view` steps
//!
//! In order: insert the view as `CurrentRenderingResource`, run the legacy
//! [`Schedule::RenderCamera`](aphyn_ecs::Schedule) (`// todo: remove after migration`, so the old
//! hardcoded rendering keeps working), then `collect_passes`. That step clears the list, runs the
//! material step (below), calls every feature slot's `add_passes` in registration order and
//! unstable-sorts the list by order. Next, run every entry through its slot, then remove the
//! resource. The render API comes from the world's `RenderApiResource`, cloned once per call
//! (shallow handle clones, though `RenderApi`'s FFI clone boxes its `Arc`) so that the pass
//! context can hold it next to a mutable world. Without that resource nothing is collected or
//! run, but the legacy schedule still ran and the resource is still removed.
//!
//! #### Material step
//!
//! The registry owns the material step (`MaterialStep` in `material.rs`, the spec's
//! `MaterialRenderFeature`). It is not a slot. At the start of `collect_passes` it:
//!
//! 1. queries `(&MeshComponent, &MaterialComponent, &GlobalTransform)` into a reused vec of
//!    `GatheredInstance { template_slot, material, mesh, local_to_world }`, skipping materials that
//!    are not in `AssetStorageResource<Material>` and materials whose template name is unknown or
//!    names a feature (each such handle is printed once, tracked in a reused `logged` set),
//! 2. `build_batches`: unstable-sorts the gathered vec by `(template_slot, material, mesh)` and
//!    groups equal runs into a reused `MaterialBatch` vec, whose instance ranges index the sorted
//!    gathered vec,
//! 3. writes every `local_to_world` into `MaterialInstancesResource` with `write_instances_with`,
//!    first replacing the resource with a bigger buffer when it is missing or too small
//!    (`next_instance_capacity`: at least 64, at least double). Without a `RenderApiResource` only
//!    this GPU write is skipped, so tests can gather without a GPU,
//! 4. calls every template slot's `add_passes` in registration order with its contiguous batch
//!    run; a batch's template is the `template_slot` of its first gathered instance.
//!
//! The gather does not check that the mesh handle resolves: a `StandardMesh` can only be built
//! with a GPU, so tests could not build a valid entity, and templates look meshes up anyway when
//! they draw. Entities with an unresolved mesh still take instance slots. The gather runs on the
//! host side with the world's own types cache, so the material step needs no cache of its own.
//!
//! #### Slot erasure
//!
//! The registry is `#[repr(C)]` and holds only `aphyn_ffi` types, because a game library
//! registers features into a registry owned by the host. Each feature becomes a `RenderSlot`, an
//! `FfiDroppable` box holding the typed state (the feature, its `FfiVec<Pass>` and a lazily built
//! `TypesRegistryCache`) plus a `&'static` vtable of `extern "C-unwind"` functions
//! (`add_passes`, `run_pass(index)`, `clear_passes`). Those functions are monomorphized on the
//! side that called `add_feature`, so the feature's code and memory stay in its own binary. The
//! vtable passes only FFI pointers: `*mut WorldDataUnsafeFfi`, the world's
//! `*const TypesRegistryAccessFfi` (from `WorldDataUnsafeFfi::types`), the shared
//! `*mut FfiVec<RenderPassEntry>`, and `*const RenderApiResource`. The slot rebuilds a
//! `WorldDataMut` from its own cache. A [`RenderPassEntry`] is `(order, slot, index)`, where
//! `slot` indexes the registry's slot vec and `index` the slot's pass vec. Material templates
//! share this slot space and the name map. A template slot's state holds the template instead of
//! a feature, its `add_passes` also takes its batches as `*const MaterialBatch` plus a `u64`
//! length (null and `0` for features), and its vtable sets `default_data`, which writes the
//! template's `MaterialData` into an out pointer. That function being `Some` is what marks a slot
//! as a template.
//!
//! #### One world per registry
//!
//! A slot builds its `TypesRegistryCache` from the types registry of the first world it sees and
//! keeps it, so a registry is paired with one world (one types registry) for its whole life. Using
//! it with another world gives wrong type ids.
//!
//! #### Allocation-free pass list
//!
//! The shared entry list and every slot's pass vec are cleared, not dropped, on each collect, so
//! they keep their capacity. Sorting is an in-place unstable sort. After warm-up, collecting and
//! sorting passes does not allocate. The only lazy allocation is a slot's types cache on first
//! use. Keep per-pass data in the `Pass` values or the feature, never in fresh per-frame
//! collections. The material step's gathered vec, batch vec and logged set are reused the same
//! way. **Known exception:** the material step's ECS query allocates every frame (it builds its
//! archetype list on each call). This is accepted for phase 1 and stays until the ECS has a
//! reusable, allocation-free query (`// todo` at the query in `material.rs`). Likewise
//! `render_view` clones `RenderApiResource` once per call, which is one boxed `Arc` allocation
//! per rendered view (see `render_view` steps).
//!
//! #### Dynamic library drop order
//!
//! A slot's drop function and vtable live in the binary that registered the feature. A registry
//! holding slots from a game library must be dropped before that library is unloaded.

mod feature;
mod material;
mod orders;
mod pass_list;
mod registry;

pub use feature::*;
pub use material::*;
pub use orders::*;
pub use pass_list::*;
pub use registry::*;

pub fn add_render_features_module_to(mut world: aphyn_ecs::WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(aphyn_asset_storage::AssetStorageResource::<aphyn_render::Material>::new());
}
