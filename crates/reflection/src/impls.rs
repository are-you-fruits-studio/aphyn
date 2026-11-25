use std::collections::{BTreeMap, HashMap};

use aphyn_ffi::{FfiIndexMap, FfiOption, FfiSmallString, FfiString, FfiVec};
use aphyn_math::{Mat, Quat, Vec2, Vec3, Vec4};

use crate::{
    EnumReflectorBuilder, ListReflectorBuilder, MapReflectorBuilder, Reflectable, Reflector, StructReflectorBuilder,
    ValueReflectorBuilder,
};

macro_rules! value_reflectables {
    ($($T: ty),* $(,)?) => {
        $(
            impl Reflectable for $T {
                fn reflector() -> Reflector {
                    ValueReflectorBuilder::<Self>::new()
                        .default_fn(Some(<$T>::default))
                        .clone_fn(Some(<$T>::clone))
                        .build()
                }
            }
        )*
    };
}

value_reflectables!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    String,
    FfiString,
    FfiSmallString,
);

impl<K: 'static + Eq + std::hash::Hash, V: 'static> Reflectable for FfiIndexMap<K, V> {
    fn reflector() -> Reflector {
        MapReflectorBuilder::<Self, K, V>::new(
            |m| m.len() as usize,
            |m| m.keys().collect(),
            |m, k| m.get(k),
            |m, k| m.get_mut(k),
            Some((
                |m, k, v| {
                    if m.contains_key(&k) {
                        return Err((k, v));
                    }
                    m.insert(k, v);
                    Ok(())
                },
                |m, k| m.remove_shift(k),
            )),
        )
        .default_fn(Some(FfiIndexMap::new))
        .build()
    }
}

impl<K: 'static + Eq + std::hash::Hash, V: 'static> Reflectable for HashMap<K, V> {
    fn reflector() -> Reflector {
        MapReflectorBuilder::<Self, K, V>::new(
            |m| m.len(),
            |m| m.keys().collect(),
            |m, k| m.get(k),
            |m, k| m.get_mut(k),
            Some((
                |m, k, v| {
                    if m.contains_key(&k) {
                        return Err((k, v));
                    }
                    m.insert(k, v);
                    Ok(())
                },
                |m, k| m.remove(k),
            )),
        )
        .default_fn(Some(HashMap::new))
        .build()
    }
}

impl<K: 'static + Ord, V: 'static> Reflectable for BTreeMap<K, V> {
    fn reflector() -> Reflector {
        MapReflectorBuilder::<Self, K, V>::new(
            |m| m.len(),
            |m| m.keys().collect(),
            |m, k| m.get(k),
            |m, k| m.get_mut(k),
            Some((
                |m, k, v| {
                    if m.contains_key(&k) {
                        return Err((k, v));
                    }
                    m.insert(k, v);
                    Ok(())
                },
                |m, k| m.remove(k),
            )),
        )
        .default_fn(Some(BTreeMap::new))
        .build()
    }
}

impl<T: 'static> Reflectable for Vec<T> {
    fn reflector() -> Reflector {
        ListReflectorBuilder::<Self, T>::new(
            |v| v.len(),
            |v, i| v.get(i),
            |v, i| v.get_mut(i),
            Some((
                |v, i, e| {
                    if i > v.len() {
                        return Err(e);
                    }
                    v.insert(i, e);
                    Ok(())
                },
                |v, i| (i < v.len()).then(|| v.remove(i)),
            )),
        )
        .default_fn(Some(Vec::new))
        .build()
    }
}

impl<T: 'static> Reflectable for FfiVec<T> {
    fn reflector() -> Reflector {
        ListReflectorBuilder::<Self, T>::new(
            |v| v.len() as usize,
            |v, i| v.get(i as u64),
            |v, i| v.get_mut(i as u64),
            Some((
                |v, i, e| {
                    if i > v.len() as usize {
                        return Err(e);
                    }
                    v.push(e);
                    v.as_mut_slice()[i..].rotate_right(1);
                    Ok(())
                },
                |v, i| v.remove(i as u64),
            )),
        )
        .default_fn(Some(FfiVec::new))
        .build()
    }
}

impl<T: 'static, const N: usize> Reflectable for [T; N] {
    fn reflector() -> Reflector {
        ListReflectorBuilder::<Self, T>::new(|_| N, |v, i| v.get(i), |v, i| v.get_mut(i), None).build()
    }
}

impl<T: 'static> Reflectable for Option<T> {
    fn reflector() -> Reflector {
        EnumReflectorBuilder::<Self>::new()
            .variant("None", |v| v.is_none(), |b| b, |_| Some(None))
            .variant(
                "Some",
                |v| v.is_some(),
                |b| b.field("0", |v| v.as_ref(), |v| v.as_mut()),
                |fields| Some(Some(unsafe { fields.take()? })),
            )
            .default_fn(Some(|| None))
            .build()
    }
}

impl<T: 'static> Reflectable for FfiOption<T> {
    fn reflector() -> Reflector {
        EnumReflectorBuilder::<Self>::new()
            .variant("None", |v| matches!(v, FfiOption::None), |b| b, |_| Some(FfiOption::None))
            .variant(
                "Some",
                |v| matches!(v, FfiOption::Some(_)),
                |b| {
                    b.field(
                        "0",
                        |v| match v {
                            FfiOption::Some(v) => Some(v),
                            FfiOption::None => None,
                        },
                        |v| match v {
                            FfiOption::Some(v) => Some(v),
                            FfiOption::None => None,
                        },
                    )
                },
                |fields| Some(FfiOption::Some(unsafe { fields.take()? })),
            )
            .default_fn(Some(|| FfiOption::None))
            .build()
    }
}

impl<T: 'static> Reflectable for Vec2<T> {
    fn reflector() -> Reflector {
        StructReflectorBuilder::<Self>::new()
            .field("x", |v| Some(&v.x), |v| Some(&mut v.x))
            .field("y", |v| Some(&v.y), |v| Some(&mut v.y))
            .build()
    }
}

impl<T: 'static> Reflectable for Vec3<T> {
    fn reflector() -> Reflector {
        StructReflectorBuilder::<Self>::new()
            .field("x", |v| Some(&v.x), |v| Some(&mut v.x))
            .field("y", |v| Some(&v.y), |v| Some(&mut v.y))
            .field("z", |v| Some(&v.z), |v| Some(&mut v.z))
            .build()
    }
}

impl<T: 'static> Reflectable for Vec4<T> {
    fn reflector() -> Reflector {
        StructReflectorBuilder::<Self>::new()
            .field("x", |v| Some(&v.x), |v| Some(&mut v.x))
            .field("y", |v| Some(&v.y), |v| Some(&mut v.y))
            .field("z", |v| Some(&v.z), |v| Some(&mut v.z))
            .field("w", |v| Some(&v.w), |v| Some(&mut v.w))
            .build()
    }
}

impl<T: 'static> Reflectable for Quat<T> {
    fn reflector() -> Reflector {
        StructReflectorBuilder::<Self>::new()
            .field("x", |v| Some(&v.x), |v| Some(&mut v.x))
            .field("y", |v| Some(&v.y), |v| Some(&mut v.y))
            .field("z", |v| Some(&v.z), |v| Some(&mut v.z))
            .field("w", |v| Some(&v.w), |v| Some(&mut v.w))
            .build()
    }
}

// columns
impl<T: 'static, const N: usize> Reflectable for Mat<N, T> {
    fn reflector() -> Reflector {
        ListReflectorBuilder::<Self, [T; N]>::new(|_| N, |m, i| m.try_col(i), |m, i| m.try_col_mut(i), None).build()
    }
}
