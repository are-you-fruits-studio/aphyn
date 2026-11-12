use aphyn_ecs::Component;
use aphyn_math::Vec3;
use aphyn_serialization::*;

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
