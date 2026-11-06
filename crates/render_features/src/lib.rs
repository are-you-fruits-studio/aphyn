//! # aphyn_render_features
//!
//! Render features and the per-view pass list. A [`RenderRegistry`] owns the render features of
//! one world. Every [`render_view`](RenderRegistry::render_view) asks each feature for its passes,
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
//! # How to maintain
//!
//! #### `render_view` steps
//!
//! In order: insert the view as `CurrentRenderingResource`, run the legacy
//! [`Schedule::RenderCamera`](aphyn_ecs::Schedule) (`// todo: remove after migration`, so the old
//! hardcoded rendering keeps working), then `collect_passes`. That step clears the list, calls
//! every slot's `add_passes` in registration order and unstable-sorts the list by order. Next,
//! run every entry through its slot, then remove the resource. The render API comes from the
//! world's `RenderApiResource`, cloned once per call (shallow handle clones, though `RenderApi`'s
//! FFI clone boxes its `Arc`) so that the pass context can hold it next to a mutable world. Without that resource nothing is collected or
//! run, but the legacy schedule still ran and the resource is still removed.
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
//! share this slot space and the name map.
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
//! collections.
//!
//! #### Dynamic library drop order
//!
//! A slot's drop function and vtable live in the binary that registered the feature. A registry
//! holding slots from a game library must be dropped before that library is unloaded.

mod feature;
mod orders;
mod pass_list;
mod registry;

pub use feature::*;
pub use orders::*;
pub use pass_list::*;
pub use registry::*;
