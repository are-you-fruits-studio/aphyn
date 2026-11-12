use std::sync::atomic::{AtomicBool, Ordering};

use aphyn_app::TimeResource;
use aphyn_collision::{
    ColliderBvh, ColliderComponent, ColliderShape, CollisionLayer, CollisionShape, ContactManifold, LineBoundType, collide,
};
use aphyn_ecs::{EntitiesHolderMut, EntitiesHolderRef, EntityId, WorldDataMut};
use aphyn_math::{Mat, Mat3, Mat4, Quat, Vec3};
use aphyn_transform::{ChildComponent, GlobalTransform, LocalTransform};

use crate::{
    PhysicsColliderRef, PhysicsMaterial, PhysicsMaterialComponent, PhysicsResource, PhysicsWorldResource, RigidbodyComponent,
    RigidbodyForces, RigidbodyType,
};

static WARNED_UNSUPPORTED_SHAPE: AtomicBool = AtomicBool::new(false);
const MAX_HIERARCHY_DEPTH: usize = 256;

pub fn step_physics(mut world: WorldDataMut) {
    let (mut res, mut ent, _) = world.as_tuple_mut();

    let Some(dt) = res.get::<TimeResource>().map(|t| t.fixed_delta_time) else {
        return;
    };
    let Some(settings) = res.get::<PhysicsResource>().cloned() else {
        return;
    };
    if dt <= 0.0 {
        return;
    }

    let (mut bodies, colliders) = gather(ent.as_ref(), &settings);

    integrate_velocities(&mut bodies, &settings, dt);
    let mut contacts = find_contacts(&bodies, &colliders, &settings, dt);
    solve(&mut bodies, &mut contacts, settings.solver_iterations);
    integrate_positions(&mut bodies, dt);
    write_back(&mut ent, &bodies);

    if let Some(physics_world) = res.get_mut::<PhysicsWorldResource>() {
        *physics_world = PhysicsWorldResource::new(colliders.iter().map(|c| {
            (
                c.shape.clone(),
                c.world_matrix(&bodies),
                c.layer,
                PhysicsColliderRef {
                    collider: c.entity,
                    body: c.body.map(|b| bodies[b].entity),
                },
            )
        }));
    }
}

// gathering

struct Body {
    entity: EntityId,
    body_type: RigidbodyType,
    parent: GlobalTransform,
    position: Vec3<f32>,
    rotation: Quat<f32>,
    scale: Vec3<f32>,
    com_local: Vec3<f32>,
    com: Vec3<f32>,
    velocity: Vec3<f32>,
    angular_velocity: Vec3<f32>,
    inv_mass: f32,
    inv_inertia_local: M3,
    inv_inertia: M3,
    linear_damping: f32,
    angular_damping: f32,
    gravity_scale: f32,
    forces: RigidbodyForces,
}

impl Body {
    fn is_dynamic(&self) -> bool {
        self.body_type == RigidbodyType::Dynamic
    }

    fn pose_matrix(&self) -> Mat4<f32> {
        self.rotation.to_matrix().into_4x4_with_offset(self.position)
    }

    fn update_world_inertia(&mut self) {
        let r = M3::from_rotation(self.rotation);
        self.inv_inertia = r.mul(self.inv_inertia_local).mul(r.transpose());
    }
}

struct Collider {
    entity: EntityId,
    body: Option<usize>,
    shape: ColliderShape,
    // body frame -> collider for body colliders, world matrix for static ones
    matrix: Mat4<f32>,
    layer: CollisionLayer,
    material: PhysicsMaterial,
}

impl Collider {
    fn world_matrix(&self, bodies: &[Body]) -> Mat4<f32> {
        match self.body {
            Some(b) => bodies[b].pose_matrix() * self.matrix,
            None => self.matrix,
        }
    }

    fn world_primitive(&self, bodies: &[Body]) -> Option<CollisionShape> {
        match &self.shape {
            ColliderShape::Primitive(shape) => Some(shape.apply_matrix_lossy(self.world_matrix(bodies))),
            ColliderShape::Mesh(_) => None,
        }
    }
}

fn local_matrix(local: &LocalTransform) -> Mat4<f32> {
    (local.rotation.to_matrix() * Mat3::scale(local.scale)).into_4x4_with_offset(local.position)
}

fn gather(ent: EntitiesHolderRef, settings: &PhysicsResource) -> (Vec<Body>, Vec<Collider>) {
    let hierarchy = ent.query::<(
        Option<&ChildComponent>,
        Option<&LocalTransform>,
        Option<&GlobalTransform>,
        Option<&RigidbodyComponent>,
        Option<&PhysicsMaterialComponent>,
    )>();

    let parent_global = |e: EntityId| -> GlobalTransform {
        hierarchy
            .get(e)
            .and_then(|(child, ..)| child)
            .and_then(|child| hierarchy.get(child.parent))
            .and_then(|(_, _, global, ..)| global.copied())
            .unwrap_or(GlobalTransform::IDENTITY)
    };

    let mut bodies = Vec::new();
    let body_q = ent.query::<(
        EntityId,
        &RigidbodyComponent,
        Option<&LocalTransform>,
        Option<&GlobalTransform>,
    )>();
    for (entity, rb, local, global) in body_q.iter() {
        let parent = parent_global(entity);
        let (position, scale_rotation) = match local {
            Some(local) => (
                parent.scale_rotation * local.position + parent.position,
                parent.scale_rotation * (local.rotation.to_matrix() * Mat3::scale(local.scale)),
            ),
            None => {
                let global = global.copied().unwrap_or(GlobalTransform::IDENTITY);
                (global.position, global.scale_rotation)
            }
        };
        let (scale, rotation) = scale_rotation.to_lossy_scale_rotation();

        bodies.push(Body {
            entity,
            body_type: rb.body_type,
            parent,
            position,
            rotation: rotation.normalized(),
            scale,
            com_local: Vec3::splat(0.0),
            com: position,
            velocity: rb.linear_velocity,
            angular_velocity: rb.angular_velocity,
            inv_mass: 0.0,
            inv_inertia_local: M3::ZERO,
            inv_inertia: M3::ZERO,
            linear_damping: rb.linear_damping,
            angular_damping: rb.angular_damping,
            gravity_scale: rb.gravity_scale,
            forces: rb.forces,
        });
    }

    let body_index = |e: EntityId| bodies.iter().position(|b| b.entity == e);

    let mut colliders = Vec::new();
    let collider_q = ent.query::<(EntityId, &ColliderComponent, Option<&GlobalTransform>)>();
    for (entity, collider, global) in collider_q.iter() {
        if let ColliderShape::Primitive(CollisionShape::Line(line)) = collider.shape {
            if line.bounds != LineBoundType::SEGMENT {
                continue;
            }
        }

        // nearest ancestor-or-self body, and the local transforms from that body down to the collider
        let mut owner = None;
        let mut chain = Mat::IDENTITY;
        let mut current = entity;
        for _ in 0..MAX_HIERARCHY_DEPTH {
            let Some((child, local, _, rb, _)) = hierarchy.get(current) else {
                break;
            };
            if rb.is_some() {
                owner = body_index(current);
                break;
            }
            if let Some(local) = local {
                chain = local_matrix(local) * chain;
            }
            match child {
                Some(child) => current = child.parent,
                None => break,
            }
        }

        let material = [Some(entity), owner.map(|b| bodies[b].entity)]
            .into_iter()
            .flatten()
            .find_map(|e| hierarchy.get(e).and_then(|(.., m)| m.copied()))
            .map_or(settings.default_material, |m| m.0);

        let matrix = match owner {
            Some(b) => Mat3::scale(bodies[b].scale).into_4x4() * chain,
            None => global.map_or(Mat::IDENTITY, |g| g.scale_rotation.into_4x4_with_offset(g.position)),
        };

        if let Some(b) = owner {
            let supported = match &collider.shape {
                ColliderShape::Primitive(shape) => {
                    matches!(
                        shape,
                        CollisionShape::Sphere(_) | CollisionShape::Box(_) | CollisionShape::Aabb(_)
                    )
                }
                ColliderShape::Mesh(_) => false,
            };
            if bodies[b].is_dynamic() && !supported {
                if !WARNED_UNSUPPORTED_SHAPE.swap(true, Ordering::Relaxed) {
                    eprintln!("aphyn_physics: dynamic rigidbodies support only sphere, box and aabb colliders; others are ignored");
                }
                continue;
            }
        }

        colliders.push(Collider {
            entity,
            body: owner,
            shape: collider.shape.clone(),
            matrix,
            layer: collider.layer,
            material,
        });
    }

    for (i, body) in bodies.iter_mut().enumerate() {
        let mass = hierarchy
            .get(body.entity)
            .and_then(|(.., rb, _)| rb)
            .map_or(0.0, |rb| rb.mass);
        if body.is_dynamic() && mass > 0.0 {
            let shapes = colliders.iter().filter(|c| c.body == Some(i)).filter_map(|c| match &c.shape {
                ColliderShape::Primitive(shape) => Some(shape.apply_matrix_lossy(c.matrix)),
                ColliderShape::Mesh(_) => None,
            });
            let (com_local, inertia) = mass_properties(shapes, mass);
            body.inv_mass = 1.0 / mass;
            body.com_local = com_local;
            body.inv_inertia_local = inertia.inverse().unwrap_or(M3::ZERO);
        }
        body.com = body.position + body.rotation.to_matrix() * body.com_local;
        body.update_world_inertia();
    }

    (bodies, colliders)
}

fn mass_properties(shapes: impl Iterator<Item = CollisionShape>, mass: f32) -> (Vec3<f32>, M3) {
    // (volume, center, inertia per unit mass about the center)
    let parts: Vec<(f32, Vec3<f32>, M3)> = shapes
        .filter_map(|shape| match shape {
            CollisionShape::Sphere(s) => {
                let volume = 4.0 / 3.0 * std::f32::consts::PI * s.radius.powi(3);
                Some((volume, s.center, M3::diag(Vec3::splat(0.4 * s.radius * s.radius))))
            }
            CollisionShape::Box(b) => {
                let e = b.extents;
                let r = M3::from_rotation(b.rotation);
                let local = M3::diag(Vec3::new(e.y * e.y + e.z * e.z, e.x * e.x + e.z * e.z, e.x * e.x + e.y * e.y) * (1.0 / 3.0));
                Some((8.0 * e.x * e.y * e.z, b.center, r.mul(local).mul(r.transpose())))
            }
            CollisionShape::Aabb(a) => {
                let e = a.extents;
                let local = M3::diag(Vec3::new(e.y * e.y + e.z * e.z, e.x * e.x + e.z * e.z, e.x * e.x + e.y * e.y) * (1.0 / 3.0));
                Some((8.0 * e.x * e.y * e.z, a.center, local))
            }
            _ => None,
        })
        .filter(|(volume, ..)| *volume > 0.0)
        .collect();

    let total_volume: f32 = parts.iter().map(|(v, ..)| v).sum();
    if total_volume <= 0.0 {
        return (Vec3::splat(0.0), M3::diag(Vec3::splat(0.1 * mass)));
    }

    let com = parts
        .iter()
        .fold(Vec3::splat(0.0), |acc, (v, c, _)| acc + *c * (v / total_volume));

    let inertia = parts.iter().fold(M3::ZERO, |acc, &(v, c, unit_inertia)| {
        let m = mass * v / total_volume;
        let d = c - com;
        let parallel = M3::diag(Vec3::splat(d.dot(d))).add(M3::outer(d, d).scale(-1.0));
        acc.add(unit_inertia.scale(m)).add(parallel.scale(m))
    });

    (com, inertia)
}

// simulation

fn integrate_velocities(bodies: &mut [Body], settings: &PhysicsResource, dt: f32) {
    for body in bodies.iter_mut().filter(|b| b.is_dynamic()) {
        body.velocity += settings.gravity * (body.gravity_scale * dt);
        body.velocity += body.forces.total_force() * (body.inv_mass * dt);
        body.angular_velocity += body.inv_inertia.mul_vec(body.forces.torque_about(body.com)) * dt;
        body.velocity = body.velocity * (1.0 / (1.0 + dt * body.linear_damping.max(0.0)));
        body.angular_velocity = body.angular_velocity * (1.0 / (1.0 + dt * body.angular_damping.max(0.0)));
    }
}

struct Contact {
    a: Option<usize>,
    b: Option<usize>,
    normal: Vec3<f32>,
    tangents: [Vec3<f32>; 2],
    ra: Vec3<f32>,
    rb: Vec3<f32>,
    normal_mass: f32,
    tangent_mass: [f32; 2],
    bias: f32,
    friction: f32,
    normal_impulse: f32,
    tangent_impulse: [f32; 2],
}

fn find_contacts(bodies: &[Body], colliders: &[Collider], settings: &PhysicsResource, dt: f32) -> Vec<Contact> {
    let bvh = ColliderBvh::new(
        colliders
            .iter()
            .enumerate()
            .map(|(i, c)| (c.shape.clone(), c.world_matrix(bodies), c.layer, i)),
    );
    let is_dynamic = |c: &Collider| c.body.is_some_and(|b| bodies[b].is_dynamic());

    let mut contacts = Vec::new();

    // todo: warm starting from the previous step's contacts
    for (i, ci) in colliders.iter().enumerate() {
        if !is_dynamic(ci) {
            continue;
        }
        let Some(shape_i) = ci.world_primitive(bodies) else {
            continue;
        };

        let mut candidates = Vec::new();
        bvh.bvh()
            .query_with(shape_i.to_aabb().into(), |_, entry| candidates.push(entry.item));

        for j in candidates {
            let cj = &colliders[j];
            if i == j || (ci.body.is_some() && ci.body == cj.body) {
                continue;
            }
            if is_dynamic(cj) && j < i {
                continue;
            }
            if !settings.layer_matrix.get(ci.layer, cj.layer) {
                continue;
            }

            let material = ci.material.combine(cj.material);
            let mut push = |manifold: ContactManifold| {
                add_contacts(&mut contacts, bodies, ci.body, cj.body, manifold, material, settings, dt);
            };

            match &cj.shape {
                ColliderShape::Mesh(mesh) => mesh.collide(shape_i, cj.world_matrix(bodies), |_, m| push(m)),
                ColliderShape::Primitive(_) => {
                    if let Some(manifold) = cj.world_primitive(bodies).and_then(|shape_j| collide(shape_i, shape_j)) {
                        push(manifold);
                    }
                }
            }
        }
    }

    contacts
}

fn add_contacts(
    contacts: &mut Vec<Contact>,
    bodies: &[Body],
    a: Option<usize>,
    b: Option<usize>,
    manifold: ContactManifold,
    material: PhysicsMaterial,
    settings: &PhysicsResource,
    dt: f32,
) {
    let n = manifold.normal;
    let tangents = tangent_basis(n);

    for point in manifold.points() {
        let ra = a.map_or(Vec3::splat(0.0), |a| point.position - bodies[a].com);
        let rb = b.map_or(Vec3::splat(0.0), |b| point.position - bodies[b].com);

        let effective_mass = |dir: Vec3<f32>| {
            let k = inv_mass(bodies, a) + inv_mass(bodies, b) + angular_term(bodies, a, ra, dir) + angular_term(bodies, b, rb, dir);
            if k > 0.0 { 1.0 / k } else { 0.0 }
        };

        let relative = point_velocity(bodies, b, rb) - point_velocity(bodies, a, ra);
        let normal_velocity = relative.dot(n);

        let mut bias = settings.baumgarte / dt * (point.depth - settings.linear_slop).max(0.0);
        if normal_velocity < -settings.restitution_threshold {
            bias = bias.max(-material.restitution * normal_velocity);
        }

        contacts.push(Contact {
            a,
            b,
            normal: n,
            tangents,
            ra,
            rb,
            normal_mass: effective_mass(n),
            tangent_mass: tangents.map(effective_mass),
            bias,
            friction: material.friction,
            normal_impulse: 0.0,
            tangent_impulse: [0.0; 2],
        });
    }
}

fn solve(bodies: &mut [Body], contacts: &mut [Contact], iterations: u32) {
    for _ in 0..iterations {
        for c in contacts.iter_mut() {
            for k in 0..2 {
                let relative = point_velocity(bodies, c.b, c.rb) - point_velocity(bodies, c.a, c.ra);
                let t = c.tangents[k];
                let max = c.friction * c.normal_impulse;
                let old = c.tangent_impulse[k];
                c.tangent_impulse[k] = (old - relative.dot(t) * c.tangent_mass[k]).clamp(-max, max);
                apply_impulse(bodies, c.a, c.b, c.ra, c.rb, t * (c.tangent_impulse[k] - old));
            }

            let relative = point_velocity(bodies, c.b, c.rb) - point_velocity(bodies, c.a, c.ra);
            let old = c.normal_impulse;
            c.normal_impulse = (old + (c.bias - relative.dot(c.normal)) * c.normal_mass).max(0.0);
            apply_impulse(bodies, c.a, c.b, c.ra, c.rb, c.normal * (c.normal_impulse - old));
        }
    }
}

fn integrate_positions(bodies: &mut [Body], dt: f32) {
    // todo: continuous collision detection for fast bodies
    for body in bodies.iter_mut() {
        body.com += body.velocity * dt;

        let w = body.angular_velocity;
        let speed = w.length_sq().sqrt();
        if speed > 1e-9 {
            let step = Quat::rotation_axis_angle(w * (1.0 / speed), (speed * dt) as f64);
            body.rotation = (step * body.rotation).normalized();
        }

        body.position = body.com - body.rotation.to_matrix() * body.com_local;
        body.update_world_inertia();
    }
}

fn write_back(ent: &mut EntitiesHolderMut, bodies: &[Body]) {
    let mut q = ent.query_mut::<(
        &mut RigidbodyComponent,
        Option<&mut LocalTransform>,
        Option<&mut GlobalTransform>,
    )>();

    for body in bodies {
        let valid = [body.position, body.velocity, body.angular_velocity]
            .iter()
            .all(|v| v.into_array().iter().all(|x| x.is_finite()));
        if !valid {
            continue;
        }

        let Some((rb, local, global)) = q.get_mut(body.entity) else {
            continue;
        };

        rb.forces = RigidbodyForces::default();
        rb.linear_velocity = body.velocity;
        rb.angular_velocity = body.angular_velocity;

        if body.body_type == RigidbodyType::Kinematic
            && body.velocity == Vec3::splat(0.0)
            && body.angular_velocity == Vec3::splat(0.0)
        {
            continue;
        }

        let Some(local) = local else {
            continue;
        };
        let Some(parent_inverse) = body.parent.scale_rotation.inverse() else {
            continue;
        };
        let (_, parent_rotation) = body.parent.scale_rotation.to_lossy_scale_rotation();

        local.position = parent_inverse * (body.position - body.parent.position);
        local.rotation = (conjugate(parent_rotation.normalized()) * body.rotation).normalized();

        if let Some(global) = global {
            global.position = body.position;
            global.scale_rotation = body.rotation.to_matrix() * Mat3::scale(body.scale);
        }
    }
}

// helpers

fn conjugate(q: Quat<f32>) -> Quat<f32> {
    Quat::new(-q.x, -q.y, -q.z, q.w)
}

fn tangent_basis(n: Vec3<f32>) -> [Vec3<f32>; 2] {
    let t1 = if n.x.abs() >= 0.57735 {
        Vec3::new(n.y, -n.x, 0.0)
    } else {
        Vec3::new(0.0, n.z, -n.y)
    }
    .normalized();
    [t1, n.cross(t1)]
}

fn inv_mass(bodies: &[Body], body: Option<usize>) -> f32 {
    body.map_or(0.0, |b| bodies[b].inv_mass)
}

fn angular_term(bodies: &[Body], body: Option<usize>, r: Vec3<f32>, dir: Vec3<f32>) -> f32 {
    body.map_or(0.0, |b| bodies[b].inv_inertia.mul_vec(r.cross(dir)).cross(r).dot(dir))
}

fn point_velocity(bodies: &[Body], body: Option<usize>, r: Vec3<f32>) -> Vec3<f32> {
    body.map_or(Vec3::splat(0.0), |b| bodies[b].velocity + bodies[b].angular_velocity.cross(r))
}

fn apply_impulse(bodies: &mut [Body], a: Option<usize>, b: Option<usize>, ra: Vec3<f32>, rb: Vec3<f32>, impulse: Vec3<f32>) {
    if let Some(a) = a {
        let body = &mut bodies[a];
        body.velocity -= impulse * body.inv_mass;
        body.angular_velocity -= body.inv_inertia.mul_vec(ra.cross(impulse));
    }
    if let Some(b) = b {
        let body = &mut bodies[b];
        body.velocity += impulse * body.inv_mass;
        body.angular_velocity += body.inv_inertia.mul_vec(rb.cross(impulse));
    }
}

// small column-major 3x3 matrix for inertia tensors
#[derive(Copy, Clone, Debug)]
struct M3([Vec3<f32>; 3]);

impl M3 {
    const ZERO: Self = Self([Vec3::splat(0.0); 3]);

    fn diag(d: Vec3<f32>) -> Self {
        Self([Vec3::new(d.x, 0.0, 0.0), Vec3::new(0.0, d.y, 0.0), Vec3::new(0.0, 0.0, d.z)])
    }

    fn outer(a: Vec3<f32>, b: Vec3<f32>) -> Self {
        Self([a * b.x, a * b.y, a * b.z])
    }

    fn from_rotation(q: Quat<f32>) -> Self {
        let m = q.to_matrix();
        Self([
            m * Vec3::new(1.0, 0.0, 0.0),
            m * Vec3::new(0.0, 1.0, 0.0),
            m * Vec3::new(0.0, 0.0, 1.0),
        ])
    }

    fn mul_vec(&self, v: Vec3<f32>) -> Vec3<f32> {
        self.0[0] * v.x + self.0[1] * v.y + self.0[2] * v.z
    }

    fn mul(&self, o: Self) -> Self {
        Self(o.0.map(|c| self.mul_vec(c)))
    }

    fn add(&self, o: Self) -> Self {
        Self([self.0[0] + o.0[0], self.0[1] + o.0[1], self.0[2] + o.0[2]])
    }

    fn scale(&self, s: f32) -> Self {
        Self(self.0.map(|c| c * s))
    }

    fn rows(&self) -> [Vec3<f32>; 3] {
        let [a, b, c] = self.0;
        [Vec3::new(a.x, b.x, c.x), Vec3::new(a.y, b.y, c.y), Vec3::new(a.z, b.z, c.z)]
    }

    fn transpose(&self) -> Self {
        Self(self.rows())
    }

    fn inverse(&self) -> Option<Self> {
        let [a, b, c] = self.0;
        let det = a.dot(b.cross(c));
        if det.abs() <= 1e-12 {
            return None;
        }
        // rows of the inverse are the cross products of the columns
        let rows = [b.cross(c) * (1.0 / det), c.cross(a) * (1.0 / det), a.cross(b) * (1.0 / det)];
        Some(Self(rows).transpose())
    }
}
