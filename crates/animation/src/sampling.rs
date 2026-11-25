use std::any::type_name;

use aphyn_ffi::{FfiAny, FfiAnyRef};
use aphyn_math::Quat;
use aphyn_reflection::{ReflectionRegistry, reflect_clone};

use crate::{AnimationKey, AnimationTrack, KeyInterpolation};

macro_rules! for_numeric_types {
    ($m:ident) => {
        $m!(f32, f64, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize)
    };
}

pub fn is_numeric_type(name: &str) -> bool {
    macro_rules! check {
        ($($t:ty),*) => { $(name == type_name::<$t>())||* };
    }
    for_numeric_types!(check)
}

pub fn numeric_to_f64(value: FfiAnyRef) -> Option<f64> {
    macro_rules! convert {
        ($($t:ty),*) => {
            $(if let Some(v) = value.downcast_ref::<$t>() {
                return Some(*v as f64);
            })*
        };
    }
    for_numeric_types!(convert);
    None
}

// integers are rounded to the nearest value; `as` saturates at the type's range
pub fn numeric_from_f64(type_name_: &str, value: f64) -> Option<FfiAny> {
    macro_rules! convert {
        ($($t:ty),*) => {
            $(if type_name_ == type_name::<$t>() {
                let rounded = if type_name::<$t>().starts_with('f') { value } else { value.round() };
                return Some(FfiAny::new(rounded as $t));
            })*
        };
    }
    for_numeric_types!(convert);
    None
}

pub fn is_quat_type(name: &str) -> bool {
    name == type_name::<Quat<f32>>() || name == type_name::<Quat<f64>>()
}

pub fn quat_to_f64(value: FfiAnyRef) -> Option<Quat<f64>> {
    if let Some(q) = value.downcast_ref::<Quat<f32>>() {
        return Some(Quat::new(q.x as f64, q.y as f64, q.z as f64, q.w as f64));
    }
    value.downcast_ref::<Quat<f64>>().copied()
}

pub fn quat_from_f64(type_name_: &str, q: Quat<f64>) -> Option<FfiAny> {
    if type_name_ == type_name::<Quat<f32>>() {
        Some(FfiAny::new(Quat::new(q.x as f32, q.y as f32, q.z as f32, q.w as f32)))
    } else if type_name_ == type_name::<Quat<f64>>() {
        Some(FfiAny::new(q))
    } else {
        None
    }
}

// the key in effect at `time` for stepped values: the last one at or before it, else the first
pub fn key_at_or_before(keys: &[AnimationKey], time: f32) -> Option<&AnimationKey> {
    let idx = keys.partition_point(|k| k.time <= time);
    keys.get(idx.saturating_sub(1))
}

pub fn sample_numeric(keys: &[AnimationKey], time: f32) -> Option<f64> {
    let idx = keys.partition_point(|k| k.time <= time);

    if idx == 0 || idx == keys.len() {
        return numeric_to_f64(keys.get(idx.saturating_sub(1))?.value.as_any_ref());
    }

    let (k0, k1) = (&keys[idx - 1], &keys[idx]);
    let v0 = numeric_to_f64(k0.value.as_any_ref())?;
    let v1 = numeric_to_f64(k1.value.as_any_ref())?;

    let dt = (k1.time - k0.time) as f64;
    if dt <= 0.0 {
        return Some(v1);
    }
    let s = (time - k0.time) as f64 / dt;

    Some(match k0.interpolation {
        KeyInterpolation::Constant => v0,
        KeyInterpolation::Linear => v0 + (v1 - v0) * s,
        KeyInterpolation::Hermite => hermite(v0, k0.out_tangent as f64 * dt, v1, k1.in_tangent as f64 * dt, s),
    })
}

fn hermite(p0: f64, m0: f64, p1: f64, m1: f64, s: f64) -> f64 {
    let (s2, s3) = (s * s, s * s * s);

    (2.0 * s3 - 3.0 * s2 + 1.0) * p0 + (s3 - 2.0 * s2 + s) * m0 + (-2.0 * s3 + 3.0 * s2) * p1 + (s3 - s2) * m1
}

// numbers interpolate, anything else steps and is cloned through the registry
pub fn sample_track(registry: &ReflectionRegistry, track: &AnimationTrack, time: f32) -> Option<FfiAny> {
    if track.is_numeric() {
        numeric_from_f64(&track.value_type, sample_numeric(&track.keys, time)?)
    } else {
        reflect_clone(registry, key_at_or_before(&track.keys, time)?.value.as_any_ref())
    }
}

// the angle equal to `angle` modulo 360 that is closest to `reference`, so recorded Euler keys don't flip
pub fn unwrap_degrees_near(angle: f64, reference: f64) -> f64 {
    angle + ((reference - angle) / 360.0).round() * 360.0
}
