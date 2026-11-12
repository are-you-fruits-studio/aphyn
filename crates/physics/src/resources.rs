use aphyn_collision::{ColliderBvh, ColliderShape, CollisionLayer, CollisionLine, CollisionShape, RayHit};
use aphyn_ecs::{EntityId, Resource};
use aphyn_math::{Mat4, Vec3};

use crate::PhysicsMaterial;

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CollisionLayerMatrix {
    bits: [u64; Self::WORDS],
}

impl CollisionLayerMatrix {
    const CELLS: usize = CollisionLayer::COUNT * (CollisionLayer::COUNT + 1) / 2;
    const WORDS: usize = Self::CELLS.div_ceil(64);

    pub const ALL: Self = Self {
        bits: [u64::MAX; Self::WORDS],
    };
    pub const NONE: Self = Self { bits: [0; Self::WORDS] };

    fn cell(a: CollisionLayer, b: CollisionLayer) -> usize {
        let (lo, hi) = if a <= b {
            (a.get() as usize, b.get() as usize)
        } else {
            (b.get() as usize, a.get() as usize)
        };
        lo * (2 * CollisionLayer::COUNT - lo + 1) / 2 + (hi - lo)
    }

    pub fn get(&self, a: CollisionLayer, b: CollisionLayer) -> bool {
        let cell = Self::cell(a, b);
        self.bits[cell / 64] & (1 << (cell % 64)) != 0
    }

    pub fn set(&mut self, a: CollisionLayer, b: CollisionLayer, interact: bool) {
        let cell = Self::cell(a, b);
        if interact {
            self.bits[cell / 64] |= 1 << (cell % 64);
        } else {
            self.bits[cell / 64] &= !(1 << (cell % 64));
        }
    }
}

impl Default for CollisionLayerMatrix {
    fn default() -> Self {
        Self::ALL
    }
}

// todo: optional deterministic mode (stable float operations, order-independent contact resolution, fixed iteration order)
// todo: sleeping of resting bodies
#[repr(C)]
#[derive(Resource, Clone, Debug)]
pub struct PhysicsResource {
    pub gravity: Vec3<f32>,
    pub solver_iterations: u32,
    pub baumgarte: f32,
    pub linear_slop: f32,
    pub restitution_threshold: f32,
    pub default_material: PhysicsMaterial,
    pub layer_matrix: CollisionLayerMatrix,
}

impl Default for PhysicsResource {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            solver_iterations: 10,
            baumgarte: 0.2,
            linear_slop: 0.005,
            restitution_threshold: 1.0,
            default_material: PhysicsMaterial::default(),
            layer_matrix: CollisionLayerMatrix::ALL,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PhysicsColliderRef {
    pub collider: EntityId,
    pub body: Option<EntityId>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct RaycastHit {
    pub collider: EntityId,
    pub body: Option<EntityId>,
    pub point: Vec3<f32>,
    pub normal: Vec3<f32>,
    pub distance: f32,
}

#[derive(Resource, Default)]
pub struct PhysicsWorldResource {
    colliders: ColliderBvh<PhysicsColliderRef>,
}

impl PhysicsWorldResource {
    pub fn new(values: impl Iterator<Item = (ColliderShape, Mat4<f32>, CollisionLayer, PhysicsColliderRef)>) -> Self {
        Self {
            colliders: ColliderBvh::new(values),
        }
    }

    pub fn raycast(&self, line: CollisionLine, mask: u64) -> Option<RaycastHit> {
        let (RayHit { t, point, normal }, entry) = self.colliders.raycast(line, mask)?;
        let length = (line.end - line.start).length_sq().sqrt();

        Some(RaycastHit {
            collider: entry.item.collider,
            body: entry.item.body,
            point,
            normal,
            distance: t * length,
        })
    }

    pub fn overlap(&self, shape: CollisionShape, mask: u64, results: &mut Vec<PhysicsColliderRef>) {
        self.colliders.overlaps_with(shape, mask, |entry| results.push(entry.item));
    }
}
