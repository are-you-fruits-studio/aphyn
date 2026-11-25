use aphyn_any::{AnyBox, AnyRef, AnyTypeId, RegistrySpecificTypeId, TypesRegistryCache};
use aphyn_ecs::Resource;
use aphyn_math::Quat;
use aphyn_reflection::{ReflectionRegistry, reflect_clone};

use crate::{AnimationKey, AnimationTrack, KeyInterpolation};

macro_rules! for_numeric_types {
    ($m:ident) => {
        $m!(f32, f64, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize)
    };
}

macro_rules! numeric_ids {
    ($($t:ident),*) => {
        #[derive(Clone, Copy)]
        #[allow(non_snake_case)]
        struct NumericIds {
            $($t: RegistrySpecificTypeId<$t>,)*
        }

        impl NumericIds {
            fn new(types: &TypesRegistryCache) -> Self {
                Self { $($t: types.type_id_of::<$t>(),)* }
            }
        }
    };
}
numeric_ids!(f32, f64, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

// the ids of the types animation converts through f64, in one world's types registry; the animation
// module keeps one as a resource
#[derive(Clone)]
pub struct AnimationTypes {
    types: TypesRegistryCache,
    numeric: NumericIds,
    quat_f32: RegistrySpecificTypeId<Quat<f32>>,
    quat_f64: RegistrySpecificTypeId<Quat<f64>>,
}

impl Resource for AnimationTypes {}

impl AnimationTypes {
    pub fn new(types: &TypesRegistryCache) -> Self {
        Self {
            types: types.clone(),
            numeric: NumericIds::new(types),
            quat_f32: types.type_id_of(),
            quat_f64: types.type_id_of(),
        }
    }

    pub fn types(&self) -> &TypesRegistryCache {
        &self.types
    }

    pub fn is_numeric(&self, id: AnyTypeId) -> bool {
        macro_rules! check {
            ($($t:ident),*) => { $(id == self.numeric.$t.any_type_id())||* };
        }
        for_numeric_types!(check)
    }

    pub fn is_quat(&self, id: AnyTypeId) -> bool {
        id == self.quat_f32.any_type_id() || id == self.quat_f64.any_type_id()
    }

    pub fn f64_type_id(&self) -> AnyTypeId {
        self.numeric.f64.any_type_id()
    }

    pub fn numeric_to_f64(&self, value: AnyRef) -> Option<f64> {
        macro_rules! convert {
            ($($t:ident),*) => {
                $(if let Some(v) = value.downcast_ref(self.numeric.$t) {
                    return Some(*v as f64);
                })*
            };
        }
        for_numeric_types!(convert);
        None
    }

    // integers are rounded to the nearest value; `as` saturates at the type's range
    pub fn numeric_from_f64(&self, id: AnyTypeId, value: f64) -> Option<AnyBox> {
        macro_rules! convert {
            ($($t:ident),*) => {
                $(if id == self.numeric.$t.any_type_id() {
                    let rounded = if stringify!($t).starts_with('f') { value } else { value.round() };
                    return Some(AnyBox::new(rounded as $t, &self.types));
                })*
            };
        }
        for_numeric_types!(convert);
        None
    }

    pub fn f64_value(&self, value: f64) -> AnyBox {
        AnyBox::new(value, &self.types)
    }

    pub fn quat_to_f64(&self, value: AnyRef) -> Option<Quat<f64>> {
        if let Some(q) = value.downcast_ref(self.quat_f32) {
            return Some(Quat::new(q.x as f64, q.y as f64, q.z as f64, q.w as f64));
        }
        value.downcast_ref(self.quat_f64).copied()
    }

    pub fn quat_from_f64(&self, id: AnyTypeId, q: Quat<f64>) -> Option<AnyBox> {
        if id == self.quat_f32.any_type_id() {
            Some(AnyBox::new(
                Quat::new(q.x as f32, q.y as f32, q.z as f32, q.w as f32),
                &self.types,
            ))
        } else if id == self.quat_f64.any_type_id() {
            Some(AnyBox::new(q, &self.types))
        } else {
            None
        }
    }
}

// the key in effect at `time` for stepped values: the last one at or before it, else the first
pub fn key_at_or_before(keys: &[AnimationKey], time: f32) -> Option<&AnimationKey> {
    let idx = keys.partition_point(|k| k.time <= time);
    keys.get(idx.saturating_sub(1))
}

pub fn sample_numeric(types: &AnimationTypes, keys: &[AnimationKey], time: f32) -> Option<f64> {
    let idx = keys.partition_point(|k| k.time <= time);

    if idx == 0 || idx == keys.len() {
        return types.numeric_to_f64(keys.get(idx.saturating_sub(1))?.value.as_any_ref());
    }

    let (k0, k1) = (&keys[idx - 1], &keys[idx]);
    let v0 = types.numeric_to_f64(k0.value.as_any_ref())?;
    let v1 = types.numeric_to_f64(k1.value.as_any_ref())?;

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
pub fn sample_track(registry: &ReflectionRegistry, types: &AnimationTypes, track: &AnimationTrack, time: f32) -> Option<AnyBox> {
    if track.is_numeric(types) {
        types.numeric_from_f64(track.value_type, sample_numeric(types, &track.keys, time)?)
    } else {
        reflect_clone(registry, key_at_or_before(&track.keys, time)?.value.as_any_ref())
    }
}

// the angle equal to `angle` modulo 360 that is closest to `reference`, so recorded Euler keys don't flip
pub fn unwrap_degrees_near(angle: f64, reference: f64) -> f64 {
    angle + ((reference - angle) / 360.0).round() * 360.0
}
