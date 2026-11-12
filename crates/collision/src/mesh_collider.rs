use aphyn_ffi::{FfiArc, FfiVec};
use aphyn_math::{Mat4, Vec3};
use aphyn_serialization::*;

use crate::{Bvh, CollisionAabb, CollisionLine, CollisionShape, ContactManifold, LineBoundType, RayHit, collide, overlaps, raycast};

pub type Triangle = [Vec3<f32>; 3];

#[repr(C)]
#[derive(Debug, Default)]
struct MeshColliderData {
    triangles: FfiVec<Triangle>,
    bvh: Bvh<u32>,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MeshCollider {
    data: FfiArc<MeshColliderData>,
}

impl Default for MeshCollider {
    fn default() -> Self {
        Self::new([])
    }
}

impl MeshCollider {
    pub fn new(triangles: impl IntoIterator<Item = Triangle>) -> Self {
        let triangles = FfiVec::from_vec(triangles.into_iter().collect());
        let bvh = Bvh::new(
            triangles
                .iter()
                .enumerate()
                .map(|(i, &tri)| (CollisionShape::Triangle(tri), i as u32)),
        );

        Self {
            data: FfiArc::new(MeshColliderData { triangles, bvh }),
        }
    }

    pub fn triangles(&self) -> &[Triangle] {
        self.data.triangles.as_slice()
    }

    pub fn local_aabb(&self) -> Option<CollisionAabb> {
        self.data.bvh.aabb()
    }

    pub fn world_aabb(&self, mesh_to_world: Mat4<f32>) -> Option<CollisionAabb> {
        self.local_aabb().map(|aabb| aabb.transformed(mesh_to_world))
    }

    pub fn query_triangles(&self, local_query: CollisionShape, mut on_hit: impl FnMut(u32, Triangle)) {
        self.data
            .bvh
            .query_with(local_query, |_, &i| on_hit(i, self.triangles()[i as usize]));
    }

    // calls `on_candidate` with world-space triangles that may touch `world_shape`
    pub fn for_each_candidate(
        &self,
        world_shape: CollisionShape,
        mesh_to_world: Mat4<f32>,
        mut on_candidate: impl FnMut(u32, Triangle),
    ) {
        let Some(world_to_mesh) = mesh_to_world.inverse() else {
            return;
        };
        let to_world = |tri: Triangle| tri.map(|p| mesh_to_world.mul_with_projection(p));

        match world_shape {
            CollisionShape::Line(line) if line.bounds != LineBoundType::SEGMENT => {
                let local = CollisionLine {
                    start: world_to_mesh.mul_with_projection(line.start),
                    end: world_to_mesh.mul_with_projection(line.end),
                    bounds: line.bounds,
                };
                self.query_triangles(local.into(), |i, tri| on_candidate(i, to_world(tri)));
            }
            shape => {
                let local = shape.to_aabb().transformed(world_to_mesh);
                self.data
                    .bvh
                    .query_aabb_with(local, |_, &i| on_candidate(i, to_world(self.triangles()[i as usize])));
            }
        }
    }

    pub fn overlaps(&self, world_shape: CollisionShape, mesh_to_world: Mat4<f32>) -> bool {
        let mut hit = false;
        self.for_each_candidate(world_shape, mesh_to_world, |_, tri| {
            hit = hit || overlaps(tri.into(), world_shape);
        });
        hit
    }

    pub fn collide(&self, world_shape: CollisionShape, mesh_to_world: Mat4<f32>, mut on_contact: impl FnMut(u32, ContactManifold)) {
        self.for_each_candidate(world_shape, mesh_to_world, |i, tri| {
            if let Some(manifold) = collide(world_shape, tri.into()) {
                on_contact(i, manifold);
            }
        });
    }

    pub fn raycast(&self, world_line: CollisionLine, mesh_to_world: Mat4<f32>) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        self.for_each_candidate(world_line.into(), mesh_to_world, |_, tri| {
            if let Some(hit) = raycast(world_line, tri.into()) {
                if best.is_none_or(|b| hit.t < b.t) {
                    best = Some(hit);
                }
            }
        });
        best
    }
}

impl<S: Copy> Serializable<S> for MeshCollider {
    // todo: mesh colliders are runtime-only for now
    fn serialize(&self, _ctx: SerializerCtx<S>, _path: &str) -> SerializedValue {
        SerializedValue::Null
    }

    fn deserialize(&mut self, _ctx: SerializerCtx<S>, _path: &str, _serialized: &SerializedValue) {}
}
