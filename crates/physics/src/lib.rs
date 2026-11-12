//! # aphyn_physics
//!
//! General-purpose 3D rigidbody physics without joints: gravity, linear and angular motion,
//! contacts with friction and restitution, compound bodies, collision layers, and raycast/overlap
//! queries. It reuses the colliders, shapes and contact generation of `aphyn_collision` and runs in
//! the [`FixedUpdate`](aphyn_ecs::Schedule::FixedUpdate) schedule.
//!
//! # How to use
//!
//! #### Enabling physics
//!
//! `aphyn_modules::add_defult_modules_to` registers the module (through
//! [`add_physics_module_to`]), which inserts [`PhysicsResource`] and [`PhysicsWorldResource`] and adds
//! the step system. The step reads `fixed_delta_time` from `aphyn_app`'s `TimeResource`, so the world
//! must contain one (the app and the editor insert it).
//!
//! #### Bodies and colliders
//!
//! Attach a [`RigidbodyComponent`] to make an entity a body, and a `ColliderComponent` on the same
//! entity and/or on entities below it in the hierarchy. Every collider belongs to its nearest
//! ancestor-or-self body; all colliders of one body form a compound whose mass (split by volume),
//! centre of mass and inertia come from its shapes. Colliders with no body above them are static.
//!
//! - [`RigidbodyType::Dynamic`] bodies get gravity and react to contacts. Their colliders must be
//!   spheres, boxes or AABBs (an AABB rotates with the body like a box); other shapes on a dynamic
//!   body are ignored with a one-time warning.
//! - [`RigidbodyType::Kinematic`] bodies move only by their velocities (or by game code moving their
//!   transform), have infinite mass, and may use any collider shape.
//! - Static colliders may use any shape, including `MeshCollider` (unbounded lines are skipped).
//!
//! `linear_velocity` is the velocity of the centre of mass, `angular_velocity` is in world space
//! (radians per second). Both are written back after each step, so game code can read and set them.
//! An optional [`PhysicsMaterialComponent`] on the collider entity (else on the body entity) overrides
//! [`PhysicsResource::default_material`]; a pair combines friction as the geometric mean and
//! restitution as the maximum.
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn spawn(mut world: WorldDataMut) {
//!     let mut ec = world.entities_mut();
//!     let e = ec.create_entity();
//!     ec.add_component(e, LocalTransform { position: Vec3::new(0.0, 5.0, 0.0), ..LocalTransform::IDENTITY }).ok().unwrap();
//!     ec.add_component(e, RigidbodyComponent::default()).ok().unwrap();
//!     ec.add_component(e, ColliderComponent::new(CollisionBox::default())).ok().unwrap();
//! }
//! ```
//!
//! #### Forces
//!
//! [`RigidbodyComponent::add_force`] (through the centre of mass), [`add_force_at_point`]
//! (world-space force and point, so it also produces torque) and [`add_torque`] accumulate into
//! [`RigidbodyComponent::forces`]. The next physics step turns them into velocity changes of dynamic
//! bodies and clears them, so apply continuous forces from a `FixedUpdate` system ordered before
//! [`SYSTEM_GROUP_PHYSICS`]; forces added in `Update` act only on the first fixed step that follows.
//!
//! [`add_force_at_point`]: RigidbodyComponent::add_force_at_point
//! [`add_torque`]: RigidbodyComponent::add_torque
//!
//! ```
//! use aphyn_app::TimeResource;
//! use aphyn_collision::{ColliderComponent, CollisionBox};
//! use aphyn_ecs::*;
//! use aphyn_math::Vec3;
//! use aphyn_physics::*;
//! use aphyn_render_features::RenderFeatureRegistry;
//! use aphyn_transform::LocalTransform;
//!
//! let mut data = WorldData::default();
//! let mut behavior = WorldBehaviorBuilder::new(data.types().clone());
//! add_physics_module_to(data.as_mut(), behavior.as_mut(), &mut RenderFeatureRegistry::default());
//! data.resources_mut().insert(TimeResource::default());
//! data.resources_mut().get_mut::<PhysicsResource>().unwrap().gravity = Vec3::splat(0.0);
//!
//! let body = {
//!     let mut ec = data.entities_mut();
//!     let body = ec.create_entity();
//!     ec.add_component(body, LocalTransform::IDENTITY).ok().unwrap();
//!     let mut rb = RigidbodyComponent { angular_damping: 0.0, linear_damping: 0.0, ..Default::default() };
//!     // pushing the +x face along +z spins the body about -y
//!     rb.add_force_at_point(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.5, 0.0, 0.0));
//!     ec.add_component(body, rb).ok().unwrap();
//!     ec.add_component(body, ColliderComponent::new(CollisionBox::default())).ok().unwrap();
//!     body
//! };
//!
//! let mut behavior = behavior.build();
//! data.resources_mut().get_mut::<TimeResource>().unwrap().begin_fixed_step();
//! behavior.execute_iteration(Schedule::FixedUpdate, data.as_mut());
//!
//! let entities = data.entities();
//! let query = entities.query::<&RigidbodyComponent>();
//! let rb = query.get(body).unwrap();
//! assert!(rb.linear_velocity.z > 0.0);
//! assert!(rb.angular_velocity.y < 0.0);
//! assert!(rb.forces.is_zero());
//! ```
//!
//! #### Layers
//!
//! Each collider has a `CollisionLayer`. [`PhysicsResource::layer_matrix`] (a
//! [`CollisionLayerMatrix`], one symmetric cell per layer pair, all pairs enabled by default) decides
//! which layers collide:
//!
//! ```
//! use aphyn_collision::CollisionLayer;
//! use aphyn_physics::CollisionLayerMatrix;
//!
//! let (player, debris) = (CollisionLayer::new(1).unwrap(), CollisionLayer::new(63).unwrap());
//! let mut matrix = CollisionLayerMatrix::ALL;
//! matrix.set(debris, player, false);
//! assert!(!matrix.get(player, debris));
//! assert!(matrix.get(player, player));
//! ```
//!
//! #### Queries
//!
//! [`PhysicsWorldResource`] holds every collider's pose after the last step.
//! [`raycast`](PhysicsWorldResource::raycast) returns the closest hit among colliders whose layer is in
//! the mask; [`overlap`](PhysicsWorldResource::overlap) lists the colliders a shape touches.
//!
//! ```no_run
//! use aphyn_collision::{COLLISION_MASK_ALL, CollisionLine, LineBoundType};
//! use aphyn_ecs::Res;
//! use aphyn_math::Vec3;
//! use aphyn_physics::PhysicsWorldResource;
//!
//! fn ground_check(physics: Res<PhysicsWorldResource>) {
//!     let ray = CollisionLine { start: Vec3::new(0.0, 1.0, 0.0), end: Vec3::new(0.0, -1.0, 0.0), bounds: LineBoundType::SEGMENT };
//!     if let Some(hit) = physics.raycast(ray, COLLISION_MASK_ALL) {
//!         let _ = (hit.collider, hit.distance);
//!     }
//! }
//! ```
//!
//! #### Stepping a world by hand
//!
//! ```
//! use aphyn_app::TimeResource;
//! use aphyn_collision::{ColliderComponent, CollisionBox, CollisionSphere};
//! use aphyn_ecs::*;
//! use aphyn_math::Vec3;
//! use aphyn_physics::*;
//! use aphyn_render_features::RenderFeatureRegistry;
//! use aphyn_transform::{GlobalTransform, LocalTransform};
//!
//! let mut data = WorldData::default();
//! let mut behavior = WorldBehaviorBuilder::new(data.types().clone());
//! add_physics_module_to(data.as_mut(), behavior.as_mut(), &mut RenderFeatureRegistry::default());
//! data.resources_mut().insert(TimeResource::default());
//!
//! let ball = {
//!     let mut ec = data.entities_mut();
//!
//!     let ground = ec.create_entity();
//!     let ground_box = CollisionBox { extents: Vec3::new(5.0, 0.5, 5.0), ..Default::default() };
//!     ec.add_component(ground, ColliderComponent::new(ground_box)).ok().unwrap();
//!
//!     let ball = ec.create_entity();
//!     let start = LocalTransform { position: Vec3::new(0.0, 3.0, 0.0), ..LocalTransform::IDENTITY };
//!     ec.add_component(ball, start).ok().unwrap();
//!     ec.add_component(ball, GlobalTransform::IDENTITY).ok().unwrap();
//!     ec.add_component(ball, RigidbodyComponent::default()).ok().unwrap();
//!     let sphere = CollisionSphere { center: Vec3::splat(0.0), radius: 0.5 };
//!     ec.add_component(ball, ColliderComponent::new(sphere)).ok().unwrap();
//!     ball
//! };
//!
//! let mut behavior = behavior.build();
//! for _ in 0..200 {
//!     data.resources_mut().get_mut::<TimeResource>().unwrap().begin_fixed_step();
//!     behavior.execute_iteration(Schedule::FixedUpdate, data.as_mut());
//! }
//!
//! let entities = data.entities();
//! let query = entities.query::<(&LocalTransform, &RigidbodyComponent)>();
//! let (transform, rb) = query.get(ball).unwrap();
//! // resting on the ground's top face (y = 0.5) within the solver's slop
//! assert!((transform.position.y - 1.0).abs() < 0.02, "{:?}", transform.position);
//! assert!(rb.linear_velocity.length_sq() < 0.01);
//! ```
//!
//! # How to maintain
//!
//! #### The step
//!
//! [`step_physics`] is one exclusive (`WorldDataMut`) system in the `FixedUpdate` group
//! [`SYSTEM_GROUP_PHYSICS`]. It is stateless between steps apart from the components: every step it
//! rebuilds bodies and colliders from the ECS, so game code may freely teleport bodies or change
//! colliders. One step:
//!
//! 1. **Gather.** A body's world pose is its parent's `GlobalTransform` combined with its
//!    `LocalTransform`, split into position, rotation and lossy scale. A collider's matrix in the body
//!    frame is the body scale times the `LocalTransform`s from the body down to the collider, so
//!    compounds stay correct across several fixed steps within one frame even though child
//!    `GlobalTransform`s are only refreshed by the transform module in `Update`. Static colliders use
//!    their `GlobalTransform`.
//! 2. **Mass properties** of dynamic bodies from their primitive shapes (sphere/box inertia, rotated
//!    and moved with the parallel-axis theorem).
//! 3. **Integrate velocities**: gravity times `gravity_scale`, the accumulated forces (torque taken
//!    about the centre of mass computed in step 2), then damping as `1 / (1 + dt * damping)`.
//! 4. **Contacts**: a `ColliderBvh` over every collider, queried with each dynamic collider's world
//!    AABB; pairs inside one body, filtered out by the layer matrix, or already seen (dynamic-dynamic
//!    pairs run once) are skipped. `aphyn_collision::collide` or `MeshCollider::collide` produce the
//!    manifolds.
//! 5. **Solve**: sequential impulses, `solver_iterations` passes over every contact point, friction
//!    on two tangents clamped by the accumulated normal impulse, a Baumgarte velocity bias for
//!    penetration beyond `linear_slop`, and restitution for approach speeds above
//!    `restitution_threshold`.
//! 6. **Integrate positions** (semi-implicit Euler; rotation by the axis-angle of `ω·dt`).
//! 7. **Write back** velocities, clear the force accumulators, and, through the inverse parent
//!    `GlobalTransform`, `LocalTransform` position/rotation (scale is untouched); the body's own
//!    `GlobalTransform` is refreshed too.
//!    Kinematic bodies with zero velocity are not written, so code can move them by transform.
//! 8. Rebuild [`PhysicsWorldResource`] from the new poses.
//!
//! #### Known gaps (todos)
//!
//! - No warm starting, sleeping, continuous collision detection, joints, or render interpolation
//!   between fixed steps.
//! - Deterministic mode (stable float operations, order-independent resolution) is not implemented.
//! - A body nested under another body reads its parent's `GlobalTransform`, which is stale until the
//!   next transform pass when the parent body moved during the same frame.
//! - Mesh contacts are per triangle, so bodies sliding over internal mesh edges can catch on them.

mod components;
mod resources;
mod step;

pub use components::*;
pub use resources::*;
pub use step::*;

use aphyn_ecs::{Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_render_features::RenderFeatureRegistry;

pub const SYSTEM_GROUP_PHYSICS: &'static str = "aphyn_physics";

// todo: bundle registration params
pub fn add_physics_module_to(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(PhysicsResource::default());
    data.resources_mut().insert(PhysicsWorldResource::default());

    behavior
        .get_mut(Schedule::FixedUpdate)
        .group(SYSTEM_GROUP_PHYSICS)
        .insert_child_system(step_physics);
}
