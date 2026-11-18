//! # aphyn_transform
//!
//! Places entities in space and on screen through a parent-child hierarchy: it turns each
//! entity's own local transform or UI rect into a world-space transform or pixel rect that
//! accounts for its ancestors.
//!
//! # How to use
//!
//! The subsystem is registered through the engine's default modules, not by calling
//! [`add_transform_module_to`] by hand:
//!
//! ```ignore
//! use aphyn::*;
//!
//! let mut app = App::new();
//! let ctx = app.init_ctx_mut();
//! add_defult_modules_to(ctx.data, ctx.behavior, ctx.render);
//! ```
//!
//! Once registered, attach the components below to entities. The matching `Global*` companions
//! and a [`ParentComponent`] are added automatically, and every value the engine reads
//! ([`GlobalTransform`], [`GlobalRectComponent`], [`GlobalDisableableComponent`]) is recomputed
//! each frame — you only ever write the `Local*` side.
//!
//! #### Placing an entity in space
//!
//! Give an entity a [`LocalTransform`] (position, rotation, scale). With no parent it is also its
//! world transform; under a parent it is interpreted relative to that parent:
//!
//! ```ignore
//! ec.add_component(entity, LocalTransform {
//!     position: Vec3::new(0.0, 1.0, 0.0),
//!     rotation: Quat::IDENTITY,
//!     scale: Vec3::splat(1.0),
//! }).ok().unwrap();
//! ```
//!
//! #### Parenting one entity to another
//!
//! Call [`set_parent`]. The child's transform/rect/disabled state then compose with the parent's.
//! It writes both sides of the link: the child's [`ChildComponent`] and the parent's
//! [`ParentComponent`] child list (added if missing). [`set_parent_at`] inserts at a position in
//! the list instead of appending (also to reorder children of the same parent), and
//! [`remove_parent`] makes the entity a root again. Parenting that would create a cycle is refused
//! (the functions return `false`):
//!
//! ```
//! use aphyn_ecs::WorldData;
//! use aphyn_transform::*;
//!
//! let mut data = WorldData::default();
//! let mut ec = data.entities_mut();
//! let [parent, a, b] = [(); 3].map(|_| ec.create_entity());
//!
//! assert!(set_parent(ec.as_mut(), a, parent));
//! assert!(set_parent_at(ec.as_mut(), b, parent, 0)); // becomes the first child
//! assert_eq!(ec.get_component::<ParentComponent>(parent).unwrap().children(), &[b, a]);
//! assert_eq!(ec.get_component::<ChildComponent>(a).unwrap().parent(), parent);
//!
//! assert!(!set_parent(ec.as_mut(), parent, a)); // would create a cycle
//!
//! assert!(remove_parent(ec.as_mut(), a));
//! assert_eq!(ec.get_component::<ParentComponent>(parent).unwrap().children(), &[b]);
//!
//! destroy_entity_and_children(ec.as_mut(), b); // also unlinks b from its parent
//! assert!(ec.get_component::<ParentComponent>(parent).unwrap().children().is_empty());
//! ```
//!
//! Both components only expose read access ([`ChildComponent::parent`],
//! [`ParentComponent::children`]); their fields are private.
//!
//! #### Laying out a UI element
//!
//! Use a [`LocalRectComponent`] instead of a transform for screen-space UI. Sizes and offsets are
//! [`UiVal`]s — a value plus a [`UiUnit`] such as pixels (`px`), a fraction of the parent
//! (`pw`/`ph`), or a fraction of the viewport (`vw`/`vh`). [`anchor`](LocalRectComponent::anchor)
//! and [`pivot`](LocalRectComponent::pivot) place it within the parent rect; the resolved pixel
//! rect lands in [`GlobalRectComponent`]:
//!
//! ```ignore
//! ec.add_component(panel, LocalRectComponent {
//!     scale: Vec2::splat(UiVal::px(200.0).into()),
//!     offset: Vec2::new(UiVal::px(10.0), UiVal::px(10.0)),
//!     anchor: Vec2::splat(0.0),
//!     ..Default::default()
//! }).ok().unwrap();
//! ```
//!
//! #### Arranging children in a row or column
//!
//! Add a [`RectChildAlignComponent`] to a parent rect to flow its children along one
//! [`UiDirection`] with a [`UiSpacing`] policy, instead of positioning each child individually:
//!
//! ```ignore
//! ec.add_component(list, RectChildAlignComponent {
//!     anchor: Vec2::splat(0.5),
//!     direction: UiDirection::Vertical,
//!     min_gap: UiVal::px(8.0),
//!     spacing: UiSpacing::SpaceBetween,
//! }).ok().unwrap();
//! ```
//!
//! #### Disabling a subtree
//!
//! A [`LocalDisableableComponent`] marks an entity disabled; the propagated
//! [`GlobalDisableableComponent`] is set when the entity *or any ancestor* is disabled, so other
//! systems can skip a whole branch by reading the global flag:
//!
//! ```ignore
//! ec.add_component(menu, LocalDisableableComponent { is_disabled: true }).ok().unwrap();
//! ```
//!
//! #### Resolving a UI value to pixels
//!
//! [`UiVal::into_px`] converts a value to pixels against a parent size and viewport size, the same
//! way the layout systems do:
//!
//! ```
//! use aphyn_transform::UiVal;
//! use aphyn_math::Vec2;
//!
//! let parent = Vec2::new(200.0, 100.0);
//! let view = Vec2::new(800.0, 600.0);
//! assert_eq!(UiVal::pw(0.5).into_px(parent, view).x, 100.0); // half the parent width
//! assert_eq!(UiVal::px(20.0).into_px(parent, view).x, 20.0);
//! ```
//!
//! #### Destroying a subtree
//!
//! [`destroy_entity_and_children`] removes an entity together with everything reachable through
//! its [`ParentComponent`] links and unlinks it from its own parent, so a parented hierarchy is
//! torn down in one call:
//!
//! ```ignore
//! destroy_entity_and_children(world.entities_mut(), root);
//! ```
//!
//! # How to maintain
//!
//! #### Local vs global split
//!
//! Authoring lives in the `Local*` components and the parent link; the engine derives the
//! `Global*` components and never expects them to be written by hand. The three derived outputs
//! are [`GlobalTransform`] (world position + combined scale/rotation matrix),
//! [`GlobalRectComponent`] (pixel center/scale/z of a UI rect), and
//! [`GlobalDisableableComponent`] (effective disabled state). All three are rebuilt from scratch
//! each frame rather than patched incrementally.
//!
//! #### The two-way parent/child link
//!
//! [`ChildComponent`] (a child naming its parent) and [`ParentComponent`] (a parent's ordered list
//! of children) must always agree. The invariant is kept by the mutating functions in `utils.rs`
//! ([`set_parent`], [`set_parent_at`], [`remove_parent`], [`destroy_entity_and_children`]), which
//! write both sides at once; there is no per-frame reconciliation that adds missing children (it
//! was O(n²) for a parent with many children). The order of the child list is the order children
//! are traversed and laid out in (e.g. by [`RectChildAlignComponent`]).
//!
//! The only frame-time repair is [`update_parents_remove_invalid_children`], a linear pass that
//! drops list entries whose child is gone, points at itself, or no longer names this parent
//! (e.g. after a plain `destroy_entity`). It never adds entries, so a [`ChildComponent`] written
//! without the matching list entry stays unlisted and is not traversed as that parent's child.
//!
//! The fields are private so links cannot drift by accident. The raw escape hatches
//! ([`ChildComponent::new_unchecked`], [`ParentComponent::new_unchecked`],
//! [`ChildComponent::parent_mut`], [`ParentComponent::children_mut`]) are associated functions,
//! not methods, so each use is explicit; they are meant for code that writes both sides itself,
//! such as deserialization (prefabs and scenes serialize both components and remap ids together).
//!
//! #### Companion-component injection
//!
//! [`adjust_component_sets`] runs first and keeps component sets consistent: any entity with a
//! `Local*` component but no matching `Global*` gets the global inserted, and any entity carrying
//! any `Global*` but no [`ParentComponent`] gets an empty one (so roots are traversable). The
//! commented-out blocks in that function are deliberately disabled removal/insertion experiments,
//! not dead leftovers — leave them as documentation of what was tried.
//!
//! #### Hierarchy traversal
//!
//! All propagation goes through the hierarchy-walk helpers. Roots are the entities with no
//! `ChildComponent`, or whose named parent is not in the query; passing `Some(root)` instead walks
//! only that entity's subtree, treating it as the single root (nothing is walked if it is not in the query).
//! [`hierarchy_iter_depth_first`] is the core depth-first walk;
//! [`hierarchy_iter_depth_first_parent_to_child`]
//! visits each parent before its children (used where a value flows *down* — transforms, disabled
//! state, parent-based rect sizing, final positioning), while
//! [`hierarchy_iter_depth_first_child_to_parent`]
//! visits children first (used where a parent's size depends on its children). The traversal does
//! not guard against cycles itself; [`set_parent_at`] refuses to create one.
//!
//! #### Why rect layout takes four passes
//!
//! A rect's size and position cannot be solved in one walk because the two have opposite data-flow
//! directions, so the work is split across ordered systems:
//! [`calculate_global_rect_scale_hierarchy_independent`] resolves sizes that need no parent (pixel
//! and viewport units; parent-relative units resolve to 0 here, matching
//! [`UiVal::into_px_without_parent`] returning `None`);
//! [`calculate_global_rect_scale_children_based`] walks bottom-up to size a parent from its
//! children plus its [`RectChildAlignComponent`] gaps; [`calculate_global_rect_scale_parent_based`]
//! walks top-down to resolve parent-relative sizes and apply padding/anchor/pivot; and
//! [`calculate_global_rect_pos`] places each child, either individually or, when the parent has a
//! [`RectChildAlignComponent`], distributed along the layout axis by the [`UiSpacing`] rule. When a
//! rect has no parent, the parent frame defaults to the whole window centered on screen.
//!
//! #### UI units
//!
//! [`UiVal::into_px`] resolves every [`UiUnit`]: `Px` is literal pixels; `Pw`/`Ph`/`Pd` are
//! fractions of parent width/height/(both axes); `Pmin`/`Pmax` use the smaller/larger parent
//! dimension; and `Vw`/`Vh`/`Vd`/`Vmin`/`Vmax` are the same against the viewport size.
//! [`UiVal::into_px_without_parent`] resolves only the units that need no parent and returns `None`
//! for the rest.
//!
//! #### System group and ordering
//!
//! Every system is registered into the [`SYSTEM_GROUP_TRANSFORM`] group on both the
//! [`Start`](aphyn_ecs::Schedule::Start) and [`Update`](aphyn_ecs::Schedule::Update) schedules by
//! [`add_transform_module_to`], with explicit `before_system` constraints that fix the order:
//! prune invalid child-list entries, then disabled state and transforms, then the four rect passes. The
//! group's run order relative to collision and rendering is set by the default-module assembly in
//! `aphyn_modules`.

mod components;
mod systems;
mod utils;

pub use self::{components::*, systems::*, utils::*};

use aphyn_ecs::{Schedule, SystemsHolderBuilderMut, WorldBehaviorBuilderMut, WorldDataMut};

pub const SYSTEM_GROUP_TRANSFORM: &'static str = "aphyn_transform";

// todo: bundle registration params; RenderFeatureRegistry is not passed here because aphyn_render_features depends on this crate
pub fn add_transform_module_to(_data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut) {
    add_module_to_schedule(behavior.get_mut(Schedule::Start));
    add_module_to_schedule(behavior.get_mut(Schedule::Update));
}

fn add_module_to_schedule(mut schedule: SystemsHolderBuilderMut) {
    schedule
        .group(SYSTEM_GROUP_TRANSFORM)
        .insert_child_system(adjust_component_sets)
        .insert_child_system(update_parents_remove_invalid_children)
        .insert_child_system(calculate_global_disableable)
        .insert_child_system(calculate_global_transform)
        .insert_child_system(calculate_global_rect_scale_hierarchy_independent)
        .insert_child_system(calculate_global_rect_scale_children_based)
        .insert_child_system(calculate_global_rect_scale_parent_based)
        .insert_child_system(calculate_global_rect_pos);

    schedule
        .order_system(adjust_component_sets)
        .before_system(update_parents_remove_invalid_children);
    schedule
        .order_system(update_parents_remove_invalid_children)
        .before_system(calculate_global_disableable);
    schedule
        .order_system(update_parents_remove_invalid_children)
        .before_system(calculate_global_transform);
    schedule
        .order_system(update_parents_remove_invalid_children)
        .before_system(calculate_global_rect_scale_hierarchy_independent);
    schedule
        .order_system(update_parents_remove_invalid_children)
        .before_system(calculate_global_rect_scale_children_based);
    schedule
        .order_system(update_parents_remove_invalid_children)
        .before_system(calculate_global_rect_scale_parent_based);
    schedule
        .order_system(update_parents_remove_invalid_children)
        .before_system(calculate_global_rect_pos);
    schedule
        .order_system(calculate_global_rect_scale_hierarchy_independent)
        .before_system(calculate_global_rect_scale_children_based);
    schedule
        .order_system(calculate_global_rect_scale_children_based)
        .before_system(calculate_global_rect_scale_parent_based);
    schedule
        .order_system(calculate_global_rect_scale_parent_based)
        .before_system(calculate_global_rect_pos);
}
