use fruits_ecs::*;

use fruits_transform::GlobalTransform;

use crate::{
    ColliderComponent, CollisionWorldResource,
};

pub fn update_collision_world(
    mut collision_world: ResMut<CollisionWorldResource>,
    q: WorldQuery<(EntityId, &ColliderComponent, Option<&GlobalTransform>)>,
) {
    let iter = q.iter().map(|(e, c, t)| {
        let mut shape = c.shape;

        if let Some(t) = t {
            shape = shape.apply_matrix_lossy(t.scale_rotation.into_4x4_with_offset(t.position));
        }

        (shape, e)
    });

    //if collision_world.is_empty() {
    *collision_world = CollisionWorldResource::new(iter);
    // collision_world.refill().extend(iter);
    //}
}
