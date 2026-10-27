//! # fruits_collision
//!
//! Decides which entities in a world touch or intersect, and answers one-off
//! geometric overlap tests between shapes.
//!
//! # How to use
//!
//! #### Enabling collision in a world
//!
//! The collision subsystem is registered through the engine's default modules, not by
//! registering this crate by hand:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let mut app = App::new();
//! add_defult_modules_to(app.ecs_mut().as_mut());
//! ```
//!
//! #### Making an entity collidable
//!
//! Attach a [`ColliderComponent`] carrying the [`CollisionShape`]:
//!
//! ```ignore
//! ec.add_component(entity, ColliderComponent {
//!     shape: CollisionShape::Sphere(CollisionSphere { center: Vec3::splat(0.0), radius: 1.0 }),
//! }).ok().unwrap();
//! ```
//!
//! #### Querying the world
//!
//! Read [`CollisionWorldResource`] in a system and probe it with a shape (here a ray).
//! [`overlaps`](CollisionWorldResource::overlaps) appends every entity whose collider the probe
//! touches to the passed `Vec`:
//!
//! ```no_run
//! use fruits_collision::{CollisionLine, CollisionWorldResource, LineBoundType};
//! use fruits_ecs::Res;
//! use fruits_math::Vec3;
//!
//! fn pick(world: Res<CollisionWorldResource>) {
//!     let ray = CollisionLine {
//!         start: Vec3::splat(0.0),
//!         end: Vec3::new(0.0, 0.0, 10.0),
//!         bounds: LineBoundType::UNRESTRICTED,
//!     };
//!     let mut hits = Vec::new();
//!     world.overlaps(ray.into(), &mut hits);
//!     // `hits` now lists every entity the ray touches
//! }
//! ```
//!
//! #### Testing two shapes directly
//!
//! For a one-off geometric test without the ECS, call the free [`overlaps`] function on two
//! shapes:
//!
//! ```
//! use fruits_collision::{CollisionAabb, CollisionShape, overlaps};
//! use fruits_math::Vec3;
//!
//! let a = CollisionAabb { center: Vec3::splat(0.0), extents: Vec3::splat(1.0) };
//! let b = CollisionAabb { center: Vec3::new(1.5, 0.0, 0.0), extents: Vec3::splat(1.0) };
//! assert!(overlaps(a.into_shape(), b.into_shape()));
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
//! #### Broad-phase layer
//!
//! [`CollisionWorldResource`], backed by [`Bvh`]. The BVH is a binary tree, median-split on a
//! cycling X/Y/Z axis chosen by node depth. Each update, [`update_collision_world`] reads every
//! [`ColliderComponent`] and, when the entity also has a
//! [`GlobalTransform`](fruits_transform::GlobalTransform), transforms the shape by it via
//! [`CollisionShape::apply_matrix_lossy`] before feeding it in (an entity with no transform
//! contributes its shape as written). The system then **rebuilds the whole resource from scratch
//! every update** (`*world = CollisionWorldResource::new(..)`) — there is no incremental update,
//! so if rebuild cost ever matters, that is the place to change. A query prunes by node AABB
//! before testing the exact leaf shapes.
//!
//! #### Caveats before changing anything
//!
//! - [`CollisionShape::to_aabb`] **panics** for a non-segment [`CollisionLine`]: an unbounded
//!   line has no finite box, so nothing that feeds the BVH may contain infinite lines.
//! - [`CollisionShape::apply_matrix_lossy`] is deliberately lossy — an AABB keeps its extents
//!   (only its center moves) and a sphere scales its radius by the *average* lossy scale.
//! - `shapes_collision.rs` (contact points: normal/penetration/point) is unfinished and is not
//!   yet part of the module tree.

mod components;
mod line_bound_type;
mod resources;
mod shapes;
mod shapes_overlap;
mod systems;
mod utils;
// mod shapes_collision;

pub use line_bound_type::*;
pub use shapes::*;
pub use shapes_overlap::*;
// todo
// pub use shapes_collision::*;
pub use components::*;
pub use resources::*;
pub use systems::*;
pub use utils::*;

use fruits_ecs::{Schedule, WorldBuilderMut};

pub const SYSTEM_GROUP_COLLISION: &'static str = "fruits_collision";

pub fn add_collision_module_to(mut world: WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(CollisionWorldResource::default());

    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP_COLLISION)
        .insert_child_system(update_collision_world);
}
