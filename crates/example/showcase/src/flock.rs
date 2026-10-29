use std::f32::consts::TAU;

use aphyn::*;

use crate::{LoopClock, SIM_DT, mesh::smoothstep};

pub const FLOCKS: usize = 3;
pub const BOIDS_PER_FLOCK: usize = 150;

const SEEK_FREQUENCY: f32 = 3.0;
const SEEK_DAMPING_RATIO: f32 = 1.0;
const SEPARATION_RADIUS: f32 = 0.3;
const SEPARATION_STRENGTH: f32 = 2.0;

#[derive(Component)]
pub struct FlockMember {
    pub flock: usize,
    pub lag: f32,
    pub offset: Vec3<f32>,
    pub swirl_turns: f32,
}

#[derive(Component, Default)]
pub struct Velocity(pub Vec3<f32>);

#[derive(Component, Default)]
pub struct Seek(pub Vec3<f32>);

#[derive(Component, Default)]
pub struct Separation(pub Vec3<f32>);

#[derive(Component)]
pub struct FlockLight(pub usize);

// Every path is a closed curve with integer frequencies in the loop phase, so the flocks' targets repeat exactly each loop.
pub fn flock_center(flock: usize, phase: f32) -> Vec3<f32> {
    let t = TAU * phase;
    match flock {
        0 => {
            let a = t;
            let r = 3.7 + 0.6 * (2.0 * a).cos();
            Vec3::new(r * a.cos(), 0.35 + 0.55 * (3.0 * a).sin(), r * a.sin())
        }
        1 => {
            let a = -t + 2.1;
            let r = 2.5 + 0.45 * (3.0 * a).sin();
            Vec3::new(r * a.cos(), -0.55 + 0.6 * (2.0 * a).sin(), r * a.sin())
        }
        _ => {
            let a = t + 4.2;
            let (x, z) = (3.1 * a.cos(), 3.1 * a.sin());
            let tilt = 0.6_f32;
            Vec3::new(x, 0.5 + z * tilt.sin(), z * tilt.cos())
        }
    }
}

pub fn boid_target(member: &FlockMember, phase: f32) -> Vec3<f32> {
    let (sin, cos) = (TAU * phase * member.swirl_turns).sin_cos();
    let o = member.offset;
    flock_center(member.flock, phase - member.lag) + Vec3::new(o.x * cos - o.z * sin, o.y, o.x * sin + o.z * cos)
}

pub fn seek_targets(clock: Res<LoopClock>, mut query: WorldQuery<(&FlockMember, &GlobalTransform, &Velocity, &mut Seek)>) {
    let phase = clock.phase();
    let stiffness = SEEK_FREQUENCY * SEEK_FREQUENCY;
    let damping = 2.0 * SEEK_DAMPING_RATIO * SEEK_FREQUENCY;

    for (member, transform, velocity, seek) in query.iter_mut() {
        seek.0 = (boid_target(member, phase) - transform.position) * stiffness - velocity.0 * damping;
    }
}

pub fn accumulate_separation(
    clock: Res<LoopClock>,
    neighbours: WorldQuery<(EntityId, &GlobalTransform, &FlockMember)>,
    mut query: WorldQuery<(EntityId, &GlobalTransform, &mut Separation)>,
) {
    // Separation fades out before the loop seam, so the damped seek spring settles every boid onto the same periodic path each loop.
    let weight = smoothstep(0.0, 0.12, clock.phase()) * (1.0 - smoothstep(0.6, 0.7, clock.phase()));

    for (entity, transform, separation) in query.iter_mut() {
        let mut push = Vec3::splat(0.0_f32);

        for (other, other_transform, _) in neighbours.iter() {
            if other == entity {
                continue;
            }

            let difference = transform.position - other_transform.position;
            let distance = difference.length() as f32;

            if distance < SEPARATION_RADIUS && distance > 1e-5 {
                push += difference * ((1.0 - distance / SEPARATION_RADIUS) / distance);
            }
        }

        separation.0 = push * (SEPARATION_STRENGTH * weight);
    }
}

pub fn apply_steering(mut query: WorldQuery<(&Seek, &Separation, &mut Velocity)>) {
    for (seek, separation, velocity) in query.iter_mut() {
        velocity.0 += (seek.0 + separation.0) * SIM_DT;
    }
}

pub fn integrate_boids(mut query: WorldQuery<(&Velocity, &FlockMember, &mut GlobalTransform)>) {
    for (velocity, _, transform) in query.iter_mut() {
        transform.position += velocity.0 * SIM_DT;

        if velocity.0.length() > 1e-4 {
            transform.scale_rotation = Quat::look_rotation(velocity.0, Vec3::Y).to_matrix();
        }
    }
}

pub fn move_flock_lights(clock: Res<LoopClock>, mut query: WorldQuery<(&FlockLight, &mut GlobalTransform)>) {
    for (light, transform) in query.iter_mut() {
        transform.position = flock_center(light.0, clock.phase()) + Vec3::new(0.0, 0.4, 0.0);
    }
}
