use aphyn_ecs::{EntityId, Resource};
use aphyn_math::Mat4;

use crate::*;

#[derive(Clone, Debug)]
pub struct ColliderEntry<T: Clone> {
    pub item: T,
    pub layer: CollisionLayer,
    pub mesh: Option<(MeshCollider, Mat4<f32>)>,
}

impl<T: Clone> ColliderEntry<T> {
    pub fn overlaps(&self, query: CollisionShape) -> bool {
        match &self.mesh {
            Some((mesh, mat)) => mesh.overlaps(query, *mat),
            None => true,
        }
    }
}

// world-space colliders: a primitive leaf holds its shape, a mesh leaf holds its world AABB plus the mesh
#[repr(C)]
#[derive(Debug)]
pub struct ColliderBvh<T: Clone> {
    bvh: Bvh<ColliderEntry<T>>,
}

impl<T: Clone> Default for ColliderBvh<T> {
    fn default() -> Self {
        Self { bvh: Bvh::default() }
    }
}

impl<T: Clone> ColliderBvh<T> {
    pub fn new(values: impl Iterator<Item = (ColliderShape, Mat4<f32>, CollisionLayer, T)>) -> Self {
        let leaves = values.filter_map(|(shape, mat, layer, item)| match shape {
            ColliderShape::Primitive(shape) => Some((shape.apply_matrix_lossy(mat), ColliderEntry { item, layer, mesh: None })),
            ColliderShape::Mesh(mesh) => {
                let aabb = mesh.world_aabb(mat)?;
                Some((
                    aabb.into(),
                    ColliderEntry {
                        item,
                        layer,
                        mesh: Some((mesh, mat)),
                    },
                ))
            }
        });

        Self { bvh: Bvh::new(leaves) }
    }

    pub fn is_empty(&self) -> bool {
        self.bvh.is_empty()
    }

    pub fn bvh(&self) -> &Bvh<ColliderEntry<T>> {
        &self.bvh
    }

    pub fn overlaps_with(&self, query: CollisionShape, mask: u64, mut on_hit: impl FnMut(&ColliderEntry<T>)) {
        self.bvh.query_with(query, |_, entry| {
            if entry.layer.is_in_mask(mask) && entry.overlaps(query) {
                on_hit(entry);
            }
        });
    }

    pub fn raycast(&self, line: CollisionLine, mask: u64) -> Option<(RayHit, &ColliderEntry<T>)> {
        let mut best: Option<(RayHit, &ColliderEntry<T>)> = None;

        self.bvh.query_with(line.into(), |shape, entry| {
            if !entry.layer.is_in_mask(mask) {
                return;
            }
            let hit = match &entry.mesh {
                Some((mesh, mat)) => mesh.raycast(line, *mat),
                None => raycast(line, *shape),
            };
            if let Some(hit) = hit {
                if best.as_ref().is_none_or(|(b, _)| hit.t < b.t) {
                    best = Some((hit, entry));
                }
            }
        });

        best
    }
}

#[repr(transparent)]
#[derive(Resource, Default)]
pub struct CollisionWorldResource {
    colliders: ColliderBvh<EntityId>,
}

impl CollisionWorldResource {
    pub fn new(values: impl Iterator<Item = (ColliderShape, Mat4<f32>, CollisionLayer, EntityId)>) -> Self {
        Self {
            colliders: ColliderBvh::new(values),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.colliders.is_empty()
    }

    pub fn overlaps(&self, query: CollisionShape, mask: u64, results: &mut Vec<EntityId>) {
        self.colliders.overlaps_with(query, mask, |entry| results.push(entry.item));
    }

    pub fn raycast(&self, line: CollisionLine, mask: u64) -> Option<(RayHit, EntityId)> {
        self.colliders.raycast(line, mask).map(|(hit, entry)| (hit, entry.item))
    }
}
