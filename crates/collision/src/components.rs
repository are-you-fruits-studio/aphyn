use aphyn_ecs::Component;
use aphyn_serialization::*;

use crate::CollisionShape;

#[repr(C)]
#[derive(Component, Default, Serializable)]
pub struct ColliderComponent {
    pub shape: CollisionShape,
}
