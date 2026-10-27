//! # fruits_transform
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
//! use fruits_engine::*;
//!
//! let mut app = App::new();
//! add_defult_modules_to(app.ecs_mut().as_mut());
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
//! Point a [`ChildComponent`] at the parent entity. The child's transform/rect/disabled state
//! then compose with the parent's; the parent's [`ParentComponent`] child list is maintained for
//! you from this link:
//!
//! ```ignore
//! ec.add_component(parent, LocalTransform::IDENTITY).ok().unwrap();
//! ec.add_component(child, LocalTransform::IDENTITY).ok().unwrap();
//! ec.add_component(child, ChildComponent { parent }).ok().unwrap();
//! ```
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
//! use fruits_transform::UiVal;
//! use fruits_math::Vec2;
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
//! its [`ParentComponent`] links, so a parented hierarchy is torn down in one call:
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
//! [`ChildComponent`] (a child naming its parent) is the source of truth; [`ParentComponent`]
//! (a parent's list of children) is a cache reconciled against it every frame.
//! [`update_parents_remove_invalid_children`] drops entries whose child no longer points back at
//! this parent (or points at itself), and [`update_parents_add_missing_children`] appends children
//! that name this parent but are absent from its list. Editing `ParentComponent.children` directly
//! is pointless — it is overwritten from the child links on the next run.
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
//! not guard against cycles in the parent links beyond the self-reference check during
//! reconciliation.
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
//! [`Start`](fruits_ecs::Schedule::Start) and [`Update`](fruits_ecs::Schedule::Update) schedules by
//! [`add_transform_module_to`], with explicit `before_system` constraints that fix the order:
//! reconcile parent links, then disabled state and transforms, then the four rect passes. The
//! group's run order relative to collision and rendering is set by the default-module assembly in
//! `fruits_modules`.

mod components;
mod systems;
mod utils;

pub use self::{components::*, systems::*, utils::*};

use fruits_ecs::{Schedule, SystemsHolderBuilderMut, WorldBuilderMut};

pub const SYSTEM_GROUP_TRANSFORM: &'static str = "fruits_transform";

pub fn add_transform_module_to(mut world: WorldBuilderMut) {
    add_module_to_schedule(world.behavior_mut().get_mut(Schedule::Start));
    add_module_to_schedule(world.behavior_mut().get_mut(Schedule::Update));
}

fn add_module_to_schedule(mut schedule: SystemsHolderBuilderMut) {
    schedule
        .group(SYSTEM_GROUP_TRANSFORM)
        .insert_child_system(adjust_component_sets)
        .insert_child_system(update_parents_remove_invalid_children)
        .insert_child_system(update_parents_add_missing_children)
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
        .before_system(update_parents_add_missing_children);
    schedule
        .order_system(update_parents_add_missing_children)
        .before_system(calculate_global_disableable);
    schedule
        .order_system(update_parents_add_missing_children)
        .before_system(calculate_global_transform);
    schedule
        .order_system(update_parents_add_missing_children)
        .before_system(calculate_global_rect_scale_hierarchy_independent);
    schedule
        .order_system(update_parents_add_missing_children)
        .before_system(calculate_global_rect_scale_children_based);
    schedule
        .order_system(update_parents_add_missing_children)
        .before_system(calculate_global_rect_scale_parent_based);
    schedule
        .order_system(update_parents_add_missing_children)
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
