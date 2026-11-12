use aphyn_ecs::*;
use aphyn_math::Mat4;
use aphyn_transform::GlobalTransform;

use crate::{ColliderComponent, CollisionWorldResource};

pub fn update_collision_world(
    mut collision_world: ResMut<CollisionWorldResource>,
    q: WorldQuery<(EntityId, &ColliderComponent, Option<&GlobalTransform>)>,
) {
    let iter = q.iter().map(|(e, c, t)| {
        let mat = t.map_or(Mat4::IDENTITY, |t| t.scale_rotation.into_4x4_with_offset(t.position));

        (c.shape.clone(), mat, c.layer, e)
    });

    *collision_world = CollisionWorldResource::new(iter);
}
