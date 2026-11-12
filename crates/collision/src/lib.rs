//! # aphyn_collision
//!
//! Decides which entities in a world touch or intersect, answers one-off geometric overlap,
//! contact and raycast tests between shapes, and provides the read-only triangle-mesh collider.
//!
//! # How to use
//!
//! #### Enabling collision in a world
//!
//! The collision subsystem is registered through the engine's default modules, not by
//! registering this crate by hand:
//!
//! ```ignore
//! use aphyn::*;
//!
//! let mut app = App::new();
//! let ctx = app.init_ctx_mut();
//! add_defult_modules_to(ctx.data, ctx.behavior, ctx.render);
//! ```
//!
//! #### Making an entity collidable
//!
//! Attach a [`ColliderComponent`]. Its [`ColliderShape`] is either a primitive [`CollisionShape`]
//! or a [`MeshCollider`]; its [`CollisionLayer`] (0..=63) is what query masks and the physics layer
//! matrix test against:
//!
//! ```ignore
//! let collider = ColliderComponent::new(CollisionSphere { center: Vec3::splat(0.0), radius: 1.0 })
//!     .with_layer(CollisionLayer::new(3).unwrap());
//! ec.add_component(entity, collider).ok().unwrap();
//! ```
//!
//! ```
//! use aphyn_collision::{COLLISION_MASK_ALL, CollisionLayer};
//!
//! assert!(CollisionLayer::new(63).is_some());
//! assert!(CollisionLayer::new(64).is_none());
//! assert!(CollisionLayer::new(5).unwrap().is_in_mask(COLLISION_MASK_ALL));
//! assert!(!CollisionLayer::new(5).unwrap().is_in_mask(1 << 4));
//! ```
//!
//! #### Mesh colliders
//!
//! [`MeshCollider::new`] takes triangles in mesh-local space and builds a [`Bvh`] over them. The
//! collider is immutable afterwards (it can be read through [`triangles`](MeshCollider::triangles)
//! and queried, never written) and cheap to clone: clones share the same data. It does not depend on
//! render meshes. World-space queries take the mesh's local-to-world matrix:
//!
//! ```
//! use aphyn_collision::{CollisionLine, LineBoundType, MeshCollider};
//! use aphyn_math::{Mat4, Vec3};
//!
//! let floor = MeshCollider::new([
//!     [Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)],
//!     [Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0), Vec3::new(-1.0, 0.0, 1.0)],
//! ]);
//! let ray = CollisionLine {
//!     start: Vec3::new(0.2, 5.0, 0.2),
//!     end: Vec3::new(0.2, -5.0, 0.2),
//!     bounds: LineBoundType::SEGMENT,
//! };
//!
//! let hit = floor.raycast(ray, Mat4::IDENTITY).unwrap();
//! assert!((hit.t - 0.5).abs() < 1e-5);
//! assert!((hit.normal.y - 1.0).abs() < 1e-5);
//! ```
//!
//! A mesh collider serializes as nothing for now: it is runtime-only and comes back empty from a
//! prefab or scene.
//!
//! #### Querying the world
//!
//! Read [`CollisionWorldResource`] in a system and probe it with a shape and a layer mask.
//! [`overlaps`](CollisionWorldResource::overlaps) appends every entity whose collider the probe
//! touches and whose layer is in the mask; [`raycast`](CollisionWorldResource::raycast) returns the
//! closest hit:
//!
//! ```no_run
//! use aphyn_collision::{COLLISION_MASK_ALL, CollisionLine, CollisionWorldResource, LineBoundType};
//! use aphyn_ecs::Res;
//! use aphyn_math::Vec3;
//!
//! fn pick(world: Res<CollisionWorldResource>) {
//!     let ray = CollisionLine {
//!         start: Vec3::splat(0.0),
//!         end: Vec3::new(0.0, 0.0, 10.0),
//!         bounds: LineBoundType::UNRESTRICTED,
//!     };
//!     let mut hits = Vec::new();
//!     world.overlaps(ray.into(), COLLISION_MASK_ALL, &mut hits);
//!     // `hits` now lists every entity the ray touches
//! }
//! ```
//!
//! #### Testing two shapes directly
//!
//! For a one-off test without the ECS, call the free [`overlaps`], [`collide`] (contact manifold)
//! or [`raycast`] functions:
//!
//! ```
//! use aphyn_collision::{CollisionAabb, CollisionSphere, collide, overlaps};
//! use aphyn_math::Vec3;
//!
//! let a = CollisionAabb { center: Vec3::splat(0.0), extents: Vec3::splat(1.0) };
//! let b = CollisionAabb { center: Vec3::new(1.5, 0.0, 0.0), extents: Vec3::splat(1.0) };
//! assert!(overlaps(a.into_shape(), b.into_shape()));
//!
//! let manifold = collide(a.into_shape(), b.into_shape()).unwrap();
//! assert!((manifold.normal.x - 1.0).abs() < 1e-5);
//! assert!((manifold.max_depth() - 0.5).abs() < 1e-5);
//! assert_eq!(manifold.points().len(), 4);
//!
//! let s1 = CollisionSphere { center: Vec3::splat(0.0), radius: 1.0 };
//! let s2 = CollisionSphere { center: Vec3::new(0.0, 1.5, 0.0), radius: 1.0 };
//! let manifold = collide(s1.into_shape(), s2.into_shape()).unwrap();
//! assert!((manifold.normal.y - 1.0).abs() < 1e-5);
//! assert!((manifold.points()[0].depth - 0.5).abs() < 1e-5);
//! ```
//!
//! ```
//! use aphyn_collision::{CollisionBox, collide};
//! use aphyn_math::{Quat, Vec3};
//!
//! // a unit box sunk 0.1 into a floor triangle
//! let bx = CollisionBox { center: Vec3::new(0.0, 0.4, 0.0), extents: Vec3::splat(0.5), rotation: Quat::IDENTITY };
//! let tri = [Vec3::new(-5.0, 0.0, -5.0), Vec3::new(5.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 5.0)];
//!
//! let manifold = collide(bx.into_shape(), tri.into()).unwrap();
//! assert!((manifold.normal.y + 1.0).abs() < 1e-5);
//! assert_eq!(manifold.points().len(), 4);
//! assert!(manifold.points().iter().all(|p| (p.depth - 0.1).abs() < 1e-4));
//! ```
//!
//! # How to maintain
//!
//! #### Geometry layer
//!
//! [`overlaps`] and the [`CollisionShape`] variants. `overlaps` matches on the ordered variant
//! pair and forwards to a private primitive routine. Each *unordered* pair has a single
//! implementation that the swapped pair reuses by exchanging its arguments (e.g. AABB-vs-box and
//! box-vs-AABB call the same function). Oriented-box, triangle, and several other tests first
//! move the geometry into an origin-centered AABB's local frame, so one separating-axis routine
//! can back multiple shape pairs.
//!
//! #### Contacts
//!
//! [`collide`] returns a [`ContactManifold`] whose normal points from the first shape to the second
//! (moving the second shape along it separates them), with up to four [`ContactPoint`]s placed
//! halfway between the surfaces. An AABB is first turned into an unrotated box. Sphere pairs use
//! the closest point on the other shape (a sphere centre inside a box exits through the nearest
//! face). Every other pair goes through one convex-hull routine (box, triangle, segment and point
//! are all hulls): SAT over face normals and edge-edge cross products picks the axis of least
//! penetration (edge axes need a 5% better depth to win, for stable resting contacts). A face axis
//! clips the other hull's most anti-parallel face against the reference face's side planes; an
//! edge axis yields one point between the closest points of the two support edges. More than four
//! points are reduced to the deepest one plus the most spread out. Unbounded lines and
//! point/segment-vs-point/segment pairs return `None` (todo). Triangles are two-sided, and mesh
//! contacts are generated per triangle, so internal mesh edges can produce ghost contacts.
//!
//! #### Broad-phase layer
//!
//! [`CollisionWorldResource`] wraps a [`ColliderBvh`], itself backed by [`Bvh`]. The BVH is a
//! binary tree, median-split on a cycling X/Y/Z axis chosen by node depth. Each update,
//! [`update_collision_world`] reads every [`ColliderComponent`] together with the entity's
//! [`GlobalTransform`](aphyn_transform::GlobalTransform) matrix (identity without one). Primitive
//! shapes are transformed via [`CollisionShape::apply_matrix_lossy`]; a mesh leaf stores the mesh's
//! world AABB plus the mesh and its matrix, and is tested exactly against its triangles. Mesh tests
//! map the probe's world AABB into mesh space to find candidate triangles, then transform those
//! triangles to world space, so they stay exact under any affine matrix. The system **rebuilds the
//! whole resource from scratch every update** — there is no incremental update, so if rebuild
//! cost ever matters, that is the place to change. A query prunes by node AABB before testing the
//! exact leaf shapes.
//!
//! #### Caveats before changing anything
//!
//! - [`CollisionShape::to_aabb`] **panics** for a non-segment [`CollisionLine`]: an unbounded
//!   line has no finite box, so nothing that feeds the BVH may contain infinite lines.
//! - [`CollisionShape::apply_matrix_lossy`] is deliberately lossy — an AABB keeps its extents
//!   (only its center moves) and a sphere scales its radius by the *average* lossy scale.

mod components;
mod contacts;
mod layer;
mod line_bound_type;
mod mesh_collider;
mod resources;
mod shapes;
mod shapes_overlap;
mod systems;
mod utils;

pub use components::*;
pub use contacts::*;
pub use layer::*;
pub use line_bound_type::*;
pub use mesh_collider::*;
pub use resources::*;
pub use shapes::*;
pub use shapes_overlap::*;
pub use systems::*;
pub use utils::*;

use aphyn_ecs::{Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_render_features::RenderFeatureRegistry;

pub const SYSTEM_GROUP_COLLISION: &'static str = "aphyn_collision";

// todo: bundle registration params
pub fn add_collision_module_to(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(CollisionWorldResource::default());

    behavior
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP_COLLISION)
        .insert_child_system(update_collision_world);
}
