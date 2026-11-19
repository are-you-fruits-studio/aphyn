use aphyn_ecs::Component;
use aphyn_math::{Quat, Vec3};
use aphyn_serialization::*;
use aphyn_transform::LocalTransform;

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serializable)]
pub enum RigidbodyType {
    #[default]
    Dynamic,
    Kinematic,
}

#[repr(C)]
#[derive(Component, Clone, Debug, PartialEq, Serializable)]
pub struct RigidbodyComponent {
    pub body_type: RigidbodyType,
    pub mass: f32,
    pub linear_velocity: Vec3<f32>,
    pub angular_velocity: Vec3<f32>,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub forces: RigidbodyForces,
}

impl Default for RigidbodyComponent {
    fn default() -> Self {
        Self {
            body_type: RigidbodyType::Dynamic,
            mass: 1.0,
            linear_velocity: Vec3::splat(0.0),
            angular_velocity: Vec3::splat(0.0),
            linear_damping: 0.01,
            angular_damping: 0.05,
            gravity_scale: 1.0,
            forces: RigidbodyForces::default(),
        }
    }
}

impl RigidbodyComponent {
    pub fn kinematic() -> Self {
        Self {
            body_type: RigidbodyType::Kinematic,
            ..Default::default()
        }
    }

    pub fn add_force(&mut self, force: Vec3<f32>) {
        self.forces.force += force;
    }

    pub fn add_torque(&mut self, torque: Vec3<f32>) {
        self.forces.torque += torque;
    }

    pub fn add_force_at_point(&mut self, force: Vec3<f32>, world_point: Vec3<f32>) {
        self.forces.point_force += force;
        self.forces.point_moment += world_point.cross(force);
    }
}

// world-space accumulators, consumed and cleared by each physics step
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RigidbodyForces {
    force: Vec3<f32>,
    torque: Vec3<f32>,
    point_force: Vec3<f32>,
    point_moment: Vec3<f32>,
}

impl Default for RigidbodyForces {
    fn default() -> Self {
        Self {
            force: Vec3::splat(0.0),
            torque: Vec3::splat(0.0),
            point_force: Vec3::splat(0.0),
            point_moment: Vec3::splat(0.0),
        }
    }
}

impl RigidbodyForces {
    pub fn total_force(&self) -> Vec3<f32> {
        self.force + self.point_force
    }

    pub fn torque_about(&self, center_of_mass: Vec3<f32>) -> Vec3<f32> {
        self.torque + self.point_moment - center_of_mass.cross(self.point_force)
    }

    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }
}

impl<S: Copy> Serializable<S> for RigidbodyForces {
    // pending forces are transient, they are never saved
    fn serialize(&self, _ctx: SerializerCtx<S>, _path: &str) -> SerializedValue {
        SerializedValue::Null
    }

    fn deserialize(&mut self, _ctx: SerializerCtx<S>, _path: &str, _serialized: &SerializedValue) {}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Serializable)]
pub struct PhysicsMaterial {
    pub friction: f32,
    pub restitution: f32,
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            friction: 0.5,
            restitution: 0.0,
        }
    }
}

impl PhysicsMaterial {
    pub fn combine(self, other: Self) -> Self {
        Self {
            friction: (self.friction * other.friction).max(0.0).sqrt(),
            restitution: self.restitution.max(other.restitution),
        }
    }
}

#[repr(C)]
#[derive(Component, Copy, Clone, Debug, PartialEq, Default, Serializable)]
pub struct PhysicsMaterialComponent(pub PhysicsMaterial);

#[repr(C)]
#[derive(Component, Copy, Clone, Debug, PartialEq, Default, Serializable)]
pub struct RigidbodyInterpolationComponent {
    pub state: RigidbodyInterpolationState,
}

// pose history in the body's parent space, written by the physics step and the interpolation systems
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RigidbodyInterpolationState {
    previous_position: Vec3<f32>,
    previous_rotation: Quat<f32>,
    current_position: Vec3<f32>,
    current_rotation: Quat<f32>,
    shown_position: Vec3<f32>,
    shown_rotation: Quat<f32>,
    has_history: bool,
    is_shown: bool,
}

impl Default for RigidbodyInterpolationState {
    fn default() -> Self {
        Self {
            previous_position: Vec3::splat(0.0),
            previous_rotation: Quat::IDENTITY,
            current_position: Vec3::splat(0.0),
            current_rotation: Quat::IDENTITY,
            shown_position: Vec3::splat(0.0),
            shown_rotation: Quat::IDENTITY,
            has_history: false,
            is_shown: false,
        }
    }
}

impl RigidbodyInterpolationState {
    fn is_current(&self, local: &LocalTransform) -> bool {
        local.position == self.current_position && local.rotation == self.current_rotation
    }

    fn reset(&mut self, local: &LocalTransform) {
        self.previous_position = local.position;
        self.previous_rotation = local.rotation;
        self.current_position = local.position;
        self.current_rotation = local.rotation;
        self.has_history = true;
        self.is_shown = false;
    }

    // `before` is the local pose the step started from; a mismatch means game code moved the body
    pub(crate) fn record_step(&mut self, before: &LocalTransform, after: &LocalTransform) {
        if !self.has_history || !self.is_current(before) {
            self.reset(before);
        }
        self.previous_position = self.current_position;
        self.previous_rotation = self.current_rotation;
        self.current_position = after.position;
        self.current_rotation = after.rotation;
    }

    pub(crate) fn show(&mut self, local: &mut LocalTransform, alpha: f32) {
        if !self.has_history {
            return;
        }
        if !self.is_current(local) {
            self.reset(local);
            return;
        }
        let mut previous = self.previous_rotation;
        if quat_dot(previous, self.current_rotation) < 0.0 {
            previous = Quat::new(-previous.x, -previous.y, -previous.z, -previous.w);
        }
        local.position = self.previous_position.lerp(self.current_position, alpha);
        local.rotation = Quat::new(
            previous.x + (self.current_rotation.x - previous.x) * alpha,
            previous.y + (self.current_rotation.y - previous.y) * alpha,
            previous.z + (self.current_rotation.z - previous.z) * alpha,
            previous.w + (self.current_rotation.w - previous.w) * alpha,
        )
        .normalized();
        self.shown_position = local.position;
        self.shown_rotation = local.rotation;
        self.is_shown = true;
    }

    pub(crate) fn restore(&mut self, local: &mut LocalTransform) {
        if !self.is_shown {
            return;
        }
        self.is_shown = false;
        if local.position == self.shown_position && local.rotation == self.shown_rotation {
            local.position = self.current_position;
            local.rotation = self.current_rotation;
        } else {
            self.reset(local);
        }
    }
}

fn quat_dot(a: Quat<f32>, b: Quat<f32>) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w
}

impl<S: Copy> Serializable<S> for RigidbodyInterpolationState {
    // the pose history is transient, it is never saved
    fn serialize(&self, _ctx: SerializerCtx<S>, _path: &str) -> SerializedValue {
        SerializedValue::Null
    }

    fn deserialize(&mut self, _ctx: SerializerCtx<S>, _path: &str, _serialized: &SerializedValue) {}
}
