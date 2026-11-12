use aphyn_ecs::Component;
use aphyn_serialization::*;

use crate::{CollisionLayer, CollisionShape, MeshCollider};

#[repr(C)]
#[derive(Component, Clone, Default, Debug, Serializable)]
pub struct ColliderComponent {
    pub shape: ColliderShape,
    pub layer: CollisionLayer,
}

impl ColliderComponent {
    pub fn new(shape: impl Into<ColliderShape>) -> Self {
        Self {
            shape: shape.into(),
            layer: CollisionLayer::DEFAULT,
        }
    }

    pub fn with_layer(mut self, layer: CollisionLayer) -> Self {
        self.layer = layer;
        self
    }
}

#[repr(C, u8)]
#[derive(Clone, Debug, Serializable)]
pub enum ColliderShape {
    Primitive(CollisionShape),
    Mesh(MeshCollider),
}

impl Default for ColliderShape {
    fn default() -> Self {
        Self::Primitive(CollisionShape::default())
    }
}

impl<T: Into<CollisionShape>> From<T> for ColliderShape {
    fn from(value: T) -> Self {
        Self::Primitive(value.into())
    }
}

impl From<MeshCollider> for ColliderShape {
    fn from(value: MeshCollider) -> Self {
        Self::Mesh(value)
    }
}
