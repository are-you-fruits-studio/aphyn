use fruits_ecs::Component;
use fruits_serialization::*;

use crate::CollisionShape;

#[repr(C)]
#[derive(Component, Default, Serializable)]
pub struct ColliderComponent {
    pub shape: CollisionShape,
}
