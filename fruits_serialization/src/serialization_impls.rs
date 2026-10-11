use std::{borrow::Cow, fmt::Write};

use fruits_ffi::{FfiIndexMap, FfiOption, FfiSmallString, FfiString, FfiVec};
use fruits_math::{Mat, Quat, Vec2, Vec3, Vec4};

use crate::{Serializable, SerializationError, SerializedComposite, SerializedCompositeValues, SerializedPrimitive, SerializedValue, SerializerCtxState, decompose_serialization_path, normalize_serialization_path, serialization_ctx::SerializerCtx};

// todo: other types

impl<S: Copy> Serializable<S> for SerializedValue {
    fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        self.get_by_path(path).cloned().unwrap_or_default()
    }
    
    fn deserialize(&mut self, _ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        if let Some(field) = self.get_or_insert_by_path(path) {
            *field = value.clone();
        }
    }
}

impl<S: Copy> Serializable<S> for String {
    fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_str(self, path)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }
        
        self.clear();

        match value {
            SerializedValue::Primitive(value) => match value {
                SerializedPrimitive::Bool(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing string from bool".into() });
                    write!(self, "{}", value);
                },
                SerializedPrimitive::Int(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing string from int".into() });
                    write!(self, "{}", value);
                },
                SerializedPrimitive::Float(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing string from float".into() });
                    write!(self, "{}", value);
                },
                SerializedPrimitive::String(value) => {
                    write!(self, "{}", value);
                },
            },
            SerializedValue::Null => {
                ctx.report_err(SerializationError::InvalidInput { message: "Deserializing string from null, using empty string".into() });
            },
            SerializedValue::Composite { .. } => {
                ctx.report_err(SerializationError::InvalidInput { message: "Deserializing string from composite, using empty string".into() });
            },
        }
    }
}

impl<S: Copy> Serializable<S> for FfiString {
    fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_str(self, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        let mut string = String::default();
        string.deserialize(ctx, path, value);
        self.clear();
        self.push_str(&string);
    }
}

impl<S: Copy> Serializable<S> for FfiSmallString {
    fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_str(self, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        let mut string = String::default();
        string.deserialize(ctx, path, value);
        self.clear();
        self.push_str_cut(&string);
    }
}

impl<S: Copy> Serializable<S> for &'static str {
    fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_str(self, path)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, _path: &str, _value: &SerializedValue) {
        ctx.report_err(SerializationError::InvalidInput { message: "&str cannot be deserialized, using empty string".into() });
        *self = "";
    }
}

fn serialize_str(string: &str, path: &str) -> SerializedValue {
    if !normalize_serialization_path(path).is_empty() {
        return SerializedValue::Null;
    }

    SerializedValue::Primitive(SerializedPrimitive::String(string.into()))
}

impl<S: Copy> Serializable<S> for u128 {
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        if !normalize_serialization_path(path).is_empty() {
            return SerializedValue::Null;
        }

        let value = if *self > (i128::MAX as u128) {
            ctx.report_err(SerializationError::InvalidInput { message: "Int value is too high and is clamped.".into() });
            i128::MAX
        } else {
            *self as i128
        };

        SerializedValue::Primitive(SerializedPrimitive::Int(value))
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }

        *self = deserialize_int(ctx, value, 0, i128::MAX) as Self
    }
}

macro_rules! serializable_int_impl {
    ($T: ident) => {
        impl<S: Copy> Serializable<S> for $T {
            fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
                if !normalize_serialization_path(path).is_empty() {
                    return SerializedValue::Null;
                }

                serialize_int(*self as i128)
            }
       
            fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
                if !normalize_serialization_path(path).is_empty() {
                    return;
                }

                *self = deserialize_int(ctx, value, Self::MIN as i128, Self::MAX as i128) as Self;
            }
        }
    };
}
macro_rules! serializable_float_impl {
    ($T: ident) => {
        impl<S: Copy> Serializable<S> for $T {
            fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
                if !normalize_serialization_path(path).is_empty() {
                    return SerializedValue::Null;
                }

                serialize_float(*self as f64)
            }
       
            fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
                if !normalize_serialization_path(path).is_empty() {
                    return;
                }

                *self = deserialize_float(ctx, value) as Self;
            }
        }
    };
}

fn serialize_int(value: i128) -> SerializedValue {
    SerializedValue::Primitive(SerializedPrimitive::Int(value))
}

fn serialize_float(value: f64) -> SerializedValue {
    SerializedValue::Primitive(SerializedPrimitive::Float(value))
}

fn deserialize_int<S: Copy>(mut ctx: SerializerCtx<S>, value: &SerializedValue, min: i128, max: i128) -> i128 {
    match value {
        SerializedValue::Null => {
            ctx.report_err(SerializationError::InvalidInput { message: "Deserializing int from null, using 0".into() });
            0
        },
        SerializedValue::Composite { .. } => {
            ctx.report_err(SerializationError::InvalidInput { message: "Deserializing int from composite, using 0".into() });
            0
        },
        SerializedValue::Primitive(value) => {
            match value {
                SerializedPrimitive::Bool(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing int from bool".into() });
                    if *value { 1 } else { 0 }
                },
                SerializedPrimitive::Int(value) => {
                    let clamped_value = (*value).clamp(min, max);
                    if *value != clamped_value {
                        ctx.report_err(SerializationError::InvalidInput { message: "Int value is too high/low and is clamped.".into() });
                    }
                    clamped_value
                },
                SerializedPrimitive::Float(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing int from float".into() });
                    (*value as i128).clamp(min, max)
                },
                SerializedPrimitive::String(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing int from string".into() });
                    value.parse::<i128>().unwrap_or(0).clamp(min, max)
                },
            }
        },
    }
}

fn deserialize_float<S: Copy>(mut ctx: SerializerCtx<S>, value: &SerializedValue) -> f64 {
    match value {
        SerializedValue::Null => {
            ctx.report_err(SerializationError::InvalidInput { message: "Deserializing float from null, using 0".into() });
            0.0
        },
        SerializedValue::Composite { .. } => {
            ctx.report_err(SerializationError::InvalidInput { message: "Deserializing float from composite, using 0".into() });
            0.0
        },
        SerializedValue::Primitive(value) => {
            match value {
                SerializedPrimitive::Bool(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing float from bool".into() });
                    if *value { 1.0 } else { 0.0 }
                },
                SerializedPrimitive::Int(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing float from int".into() });
                    *value as f64
                },
                SerializedPrimitive::Float(value) => {
                    *value
                },
                SerializedPrimitive::String(value) => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing float from string".into() });
                    value.parse::<f64>().unwrap_or(0.0)
                },
            }
        },
    }
}

serializable_int_impl!(u8);
serializable_int_impl!(i8);
serializable_int_impl!(u16);
serializable_int_impl!(i16);
serializable_int_impl!(u32);
serializable_int_impl!(i32);
serializable_int_impl!(u64);
serializable_int_impl!(i64);
serializable_int_impl!(i128);
serializable_float_impl!(f32);
serializable_float_impl!(f64);

impl<S: Copy> Serializable<S> for bool {
    fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        if !normalize_serialization_path(path).is_empty() {
            return SerializedValue::Null;
        }

        SerializedValue::Primitive(SerializedPrimitive::Bool(*self))
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }

        *self = match value {
            SerializedValue::Null => {
                ctx.report_err(SerializationError::InvalidInput { message: "Deserializing bool from null, using false".into() });
                false
            },
            SerializedValue::Composite { .. } => {
                ctx.report_err(SerializationError::InvalidInput { message: "Deserializing bool from composite, using false".into() });
                false
            },
            SerializedValue::Primitive(value) => {
                match value {
                    SerializedPrimitive::Bool(value) => {
                        *value
                    },
                    SerializedPrimitive::Int(value) => {
                        ctx.report_err(SerializationError::InvalidInput { message: "Deserializing bool from int".into() });
                        *value != 0
                    },
                    SerializedPrimitive::Float(value) => {
                        ctx.report_err(SerializationError::InvalidInput { message: "Deserializing bool from float".into() });
                        *value != 0.0
                    },
                    SerializedPrimitive::String(value) => {
                        ctx.report_err(SerializationError::InvalidInput { message: "Deserializing bool from string".into() });
                        match value.as_str().trim() {
                            "1" | "true" | "True" | "TRUE" => true,
                            _ => false
                        }
                    },
                }
            },
        } 
    }
}

impl<S: Copy> Serializable<S> for char {
    fn serialize(&self, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        if !normalize_serialization_path(path).is_empty() {
            return SerializedValue::Null;
        }

        SerializedValue::Primitive(SerializedPrimitive::String(String::from(*self).into()))
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }

        *self = match value {
            SerializedValue::Null => {
                ctx.report_err(SerializationError::InvalidInput { message: "Deserializing char from null, using default".into() });
                char::default()
            },
            SerializedValue::Composite { .. } => {
                ctx.report_err(SerializationError::InvalidInput { message: "Deserializing char from composite, using default".into() });
                char::default()
            },
            SerializedValue::Primitive(value) => {
                match value {
                    SerializedPrimitive::Bool(value) => {
                        ctx.report_err(SerializationError::InvalidInput { message: "Deserializing char from bool".into() });
                        value.to_string().chars().next().unwrap_or_default()
                    },
                    SerializedPrimitive::Int(value) => {
                        ctx.report_err(SerializationError::InvalidInput { message: "Deserializing char from int".into() });
                        value.to_string().chars().next().unwrap_or_default()
                    },
                    SerializedPrimitive::Float(value) => {
                        ctx.report_err(SerializationError::InvalidInput { message: "Deserializing char from float".into() });
                        value.to_string().chars().next().unwrap_or_default()
                    },
                    SerializedPrimitive::String(value) => {
                        let mut chars = value.chars();

                        if !(chars.next().is_some() && chars.next().is_none()) {
                            ctx.report_err(SerializationError::InvalidInput { message: "Deserializing char from string".into() });
                        }

                        value.chars().next().unwrap_or_default()
                    },
                }
            },
        } 
    }
}

fn serialize_slice<T, S: SerializerCtxState<T>>(slice: &[T], mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
    let Some((field_name, field_path)) = decompose_serialization_path(path) else {
        let mut vec = FfiVec::new();
    
        for element in slice {
            vec.push(ctx.serialize(element, ""));
        }
       
        return SerializedValue::Composite(SerializedComposite {
            is_rigid: false,
            values: SerializedCompositeValues::List(vec),
        });
    };

    if let Ok(idx) = field_name.parse::<usize>() && let Some(element) = slice.get(idx) {
        return ctx.serialize(element, field_path);
    }

    SerializedValue::Null
}

fn pre_deserialize_to_list<'a, S: Copy>(mut ctx: SerializerCtx<S>, value: &'a SerializedValue) -> &'a [SerializedValue] {
    match value {
        SerializedValue::Null => {
            ctx.report_err(SerializationError::InvalidInput { message: "Deserializing vec from null, using default".into() });
            &[]
        },
        SerializedValue::Primitive { .. } => {
            ctx.report_err(SerializationError::InvalidInput { message: "Deserializing vec from primitive".into() });
            std::slice::from_ref(value)
        },
        SerializedValue::Composite(composite) => {
            if composite.is_rigid {
                ctx.report_err(SerializationError::InvalidInput { message: "Deserializing vec from something rigid: like struct/tuple instead of list/map".into() });
            }

            match &composite.values {
                SerializedCompositeValues::Map { .. } => {
                    ctx.report_err(SerializationError::InvalidInput { message: "Deserializing vec from named values".into() });
                    std::slice::from_ref(value)
                },
                SerializedCompositeValues::List(tuple) => {
                    tuple.as_slice()
                },
            }
        },
    }
}

fn deserialize_slice<T: Default, S: SerializerCtxState<T>, C>(
    len: usize,
    mut ctx: SerializerCtx<S>,
    path: &str,
    serialized: &SerializedValue,
    slice: &mut C,
    getter: fn(&mut C, usize) -> &mut T,
    pusher: fn(&mut C, T),
    clearer: fn(&mut C),
) {
    let Some((field_name, field_path)) = decompose_serialization_path(path) else {
        clearer(slice);

        let values = pre_deserialize_to_list(ctx.as_mut(), serialized);

        for element in values {
            pusher(slice, ctx.deserialize_default("", element));
        };

        return;
    };

    let Ok(idx) = field_name.parse::<usize>() else {
        return;
    };

    if idx >= len {
        for _ in 0..=(idx - len) {
            pusher(slice, T::default());
        }
    }

    ctx.deserialize(getter(slice, idx), field_path, serialized);
}

impl<S: SerializerCtxState<T>, T: Default> Serializable<S> for Vec<T> {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_slice(self.as_slice(), ctx, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        // todo
        ctx.deserialize_list_inverted(self, path, serialized, |this, mut ctx, i, path, serialized| {
            this.resize_with(this.len().max(i + 1), || Default::default());
            ctx.deserialize(&mut this[i], path, serialized);
        });
    }
}

impl<S: SerializerCtxState<T>, T: Default> Serializable<S> for FfiVec<T> {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_slice(self.as_slice(), ctx, path)
    }
   
    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        // todo
        deserialize_slice(
            self.len() as usize,
            ctx,
            path,
            value,
            self,
            |v, i| &mut v[i as u64],
            |v, e| v.push(e),
            |v| v.clear(),
        );
    }
}

macro_rules! impl_serializable_ffimap {
    ($key_ty: ident, $key_to_str: expr, $str_to_key: expr) => {
        impl<S: SerializerCtxState<T>, T: Default> Serializable<S> for FfiIndexMap<$key_ty, T> {
            fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
                let mut ctx = ctx.serialize_map(path);

                let key_to_str: for<'a> fn(&'a $key_ty) -> Cow<'a, str> = ($key_to_str);

                for (key, value) in self {
                    let key_as_str = key_to_str(key);
                    ctx = ctx.with_field(&key_as_str, value);
                }
            
                ctx.finish_as_map(false)
            }
        
            fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
                ctx.deserialize_map_inverted(self, path, serialized, |this, mut ctx, field_name, path, serialized| {
                    let key = ($str_to_key)(field_name);
                    
                    if this.get(&key).is_none() {
                        this.insert(key.clone(), Default::default());
                    }
                
                    let field_value = this.get_mut(&key).unwrap();
                    ctx.deserialize(field_value, path, serialized);
                });
            }
        }
    };
}

macro_rules! impl_serializable_ffi_map_primitive {
    ($ty: ident, $default: expr) => {
        impl_serializable_ffimap!($ty, |s: &$ty| Cow::Owned(s.to_string()), |s: &str| s.parse().unwrap_or($default));
    };
}

// todo: make it more generic
impl_serializable_ffimap!(String, |s: &String| Cow::Borrowed(s), |s: &str| String::from(s));
impl_serializable_ffimap!(FfiString, |s: &FfiString| Cow::Borrowed(s), |s: &str| FfiString::from(s));
impl_serializable_ffi_map_primitive!(u8, 0);
impl_serializable_ffi_map_primitive!(i8, 0);
impl_serializable_ffi_map_primitive!(u16, 0);
impl_serializable_ffi_map_primitive!(i16, 0);
impl_serializable_ffi_map_primitive!(u32, 0);
impl_serializable_ffi_map_primitive!(i32, 0);
impl_serializable_ffi_map_primitive!(u64, 0);
impl_serializable_ffi_map_primitive!(i64, 0);
impl_serializable_ffi_map_primitive!(u128, 0);
impl_serializable_ffi_map_primitive!(i128, 0);

fn serialize_option<T, S: SerializerCtxState<T>>(option: Option<&T>, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
    let variants = ["None", "Some"].into_iter().map(|s| s.into()).collect();

    match option {
        None => ctx.serialize_map(path)
            .finish_as_enum(true, "None", variants),
        Some(value) => ctx.serialize_map(path)
            .with_field("0", value)
            .finish_as_enum(true, "Some", variants),
    }
}

macro_rules! deserialize_option_impl {
    ($option: expr, $ctx: expr, $path: expr, $value: expr) => {
        $ctx.deserialize_enum($path, $value)
            .with_variant("None", || Self::None)
            .with_variant("Some", || Self::Some(Default::default()))
            .finish($option, |this, ctx| {
                match this {
                    Self::None => {},
                    Self::Some(v0) => _ = ctx.with_field("0", v0),
                }
            })
    };
}

impl<S: SerializerCtxState<T>, T: Default> Serializable<S> for Option<T> {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_option(self.as_ref(), ctx, path)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        deserialize_option_impl!(self, ctx, path, value)
    }
}

impl<S: SerializerCtxState<T>, T: Default> Serializable<S> for FfiOption<T> {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        serialize_option(self.as_ref(), ctx, path)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        deserialize_option_impl!(self, ctx, path, value)
    }
}

impl<S: Copy> Serializable<S> for () {
    fn serialize(&self, _ctx: SerializerCtx<S>, _path: &str) -> SerializedValue {
        SerializedValue::Null
    }

    fn deserialize(&mut self, _ctx: SerializerCtx<S>, _path: &str, _value: &SerializedValue) {
    }
}

impl<const N: usize, S: SerializerCtxState<T>, T> Serializable<S> for [T; N]
    where [T; N]: Default
{
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        let mut ctx = ctx.serialize_list(path);

        for i in 0..N {
            ctx = ctx.with_element(i, &self[i]);
        }

        ctx.finish_as_list(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        let mut ctx = ctx.deserialize_list(self, path, serialized);
        
        for i in 0..N {
            ctx = ctx.with_element(i, &mut self[i]);
        }
    }
}

// todo: Mat, Quat, VecN
impl<S: SerializerCtxState<T>, T> Serializable<S> for Quat<T>
    where Self: Default
{
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        ctx.serialize_map(path)
            .with_field("x", &self.x)
            .with_field("y", &self.y)
            .with_field("z", &self.z)
            .with_field("w", &self.w)
            .finish_as_map(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        ctx.deserialize_map(self, path, value)
            .with_field("x", &mut self.x)
            .with_field("y", &mut self.y)
            .with_field("z", &mut self.z)
            .with_field("w", &mut self.w);
    }
}

impl<S: SerializerCtxState<T>, T> Serializable<S> for Vec2<T>
    where Self: Default
{
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        ctx.serialize_map(path)
            .with_field("x", &self.x)
            .with_field("y", &self.y)
            .finish_as_map(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        ctx.deserialize_map(self, path, value)
            .with_field("x", &mut self.x)
            .with_field("y", &mut self.y);
    }
}

impl<S: SerializerCtxState<T>, T> Serializable<S> for Vec3<T>
    where Self: Default
{
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        ctx.serialize_map(path)
            .with_field("x", &self.x)
            .with_field("y", &self.y)
            .with_field("z", &self.z)
            .finish_as_map(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        ctx.deserialize_map(self, path, value)
            .with_field("x", &mut self.x)
            .with_field("y", &mut self.y)
            .with_field("z", &mut self.z);
    }
}

impl<S: SerializerCtxState<T>, T> Serializable<S> for Vec4<T>
    where Self: Default
{
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        ctx.serialize_map(path)
            .with_field("x", &self.x)
            .with_field("y", &self.y)
            .with_field("z", &self.z)
            .with_field("w", &self.w)
            .finish_as_map(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        ctx.deserialize_map(self, path, value)
            .with_field("x", &mut self.x)
            .with_field("y", &mut self.y)
            .with_field("z", &mut self.z)
            .with_field("w", &mut self.w);
    }
}

// todo
impl<const N: usize, S: SerializerCtxState<T>, T> Serializable<S> for Mat<N, T>
    where Self: Default
{
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        todo!();
        // let mut columns_list = ctx.serialize_list();

        // for column in self.as_array() {
        //     let mut elements_list = ctx.serialize_list();

        //     for ele in column {
        //         elements_list = elements_list.with_element(ele);
        //     }

        //     let elements_list = elements_list.finish_as_list(true);
        //     columns_list = columns_list.with_serialized_element(elements_list);
        // }

        // columns_list.finish_as_list(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        todo!();
        // ctx.deserialize_list(value, |ctx_list| {
        //     let mut columns = [MaybeUninit::<[T; N]>::uninit(); N];

        //     for column_idx in 0..N {
        //         let mut elements = [MaybeUninit::<T>::uninit(); N];

        //         for i in 0..N {
        //             elements[i].write(ctx.deserialize(ctx.get_serialized_element(idx)));
        //         }
               
        //         columns[column_idx].write(unsafe { transmute(elements) });
        //     }

        //     unsafe { Some(transmute(columns)) }
        // })

        // //

        // ctx.deserialize_list(value, |ctx| {

        //     for i in 0..N {
        //         elements[i].write(ctx.get_element(i)?);
        //     }

        // })
    }
}

// todo: other impls (for tuples, maps, arrays, ...)