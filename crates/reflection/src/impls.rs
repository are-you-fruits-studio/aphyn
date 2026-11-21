use aphyn_ffi::{FfiHashMap, FfiIndexMap, FfiOption, FfiSmallString, FfiString, FfiVec};
use aphyn_math::{Mat, Quat, Vec2, Vec3, Vec4};

use crate::{CollectionReflectorBuilder, EnumReflectorBuilder, Reflectable, Reflector, StructReflectorBuilder, ValueReflector};

macro_rules! value_reflectables {
    ($($T: ty),* $(,)?) => {
        $(
            impl Reflectable for $T {
                fn reflector() -> Reflector {
                    ValueReflector::of::<Self>()
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

// maps are not reflected yet
impl<K: 'static, V: 'static> Reflectable for FfiIndexMap<K, V> {
    fn reflector() -> Reflector {
        ValueReflector::of::<Self>()
    }
}

impl<K: 'static + Eq + std::hash::Hash, V: 'static> Reflectable for FfiHashMap<K, V> {
    fn reflector() -> Reflector {
        ValueReflector::of::<Self>()
    }
}

impl<T: 'static> Reflectable for Vec<T> {
    fn reflector() -> Reflector {
        CollectionReflectorBuilder::<Self, T>::new(|v| v.len(), |v, i| v.get(i), |v, i| v.get_mut(i))
            .insert(|v, i, e| {
                if i > v.len() {
                    return Err(e);
                }
                v.insert(i, e);
                Ok(())
            })
            .remove(|v, i| (i < v.len()).then(|| v.remove(i)))
            .build()
    }
}

impl<T: 'static> Reflectable for FfiVec<T> {
    fn reflector() -> Reflector {
        CollectionReflectorBuilder::<Self, T>::new(|v| v.len() as usize, |v, i| v.get(i as u64), |v, i| v.get_mut(i as u64))
            .insert(|v, i, e| {
                if i > v.len() as usize {
                    return Err(e);
                }
                v.push(e);
                v.as_mut_slice()[i..].rotate_right(1);
                Ok(())
            })
            .remove(|v, i| v.remove(i as u64))
            .build()
    }
}

impl<T: 'static, const N: usize> Reflectable for [T; N] {
    fn reflector() -> Reflector {
        CollectionReflectorBuilder::<Self, T>::new(|_| N, |v, i| v.get(i), |v, i| v.get_mut(i)).build()
    }
}

impl<T: 'static> Reflectable for Option<T> {
    fn reflector() -> Reflector {
        EnumReflectorBuilder::<Self>::new()
            .variant("None", |v| v.is_none(), |b| b)
            .variant("Some", |v| v.is_some(), |b| b.field("0", |v| v.as_ref(), |v| v.as_mut()))
            .build()
    }
}

impl<T: 'static> Reflectable for FfiOption<T> {
    fn reflector() -> Reflector {
        EnumReflectorBuilder::<Self>::new()
            .variant("None", |v| matches!(v, FfiOption::None), |b| b)
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
            )
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
        CollectionReflectorBuilder::<Self, [T; N]>::new(|_| N, |m, i| m.try_col(i), |m, i| m.try_col_mut(i)).build()
    }
}
