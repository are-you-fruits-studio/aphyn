use std::marker::PhantomData;

use crate::{refl_repr::*, *};

// todo: ffi?

pub fn set_common_representers(reg: &mut ReflRepresenterRegistry) {
    set_common_generic_item_representers_and_self(reg, U8ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, U16ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, U32ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, U64ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, U128ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, I8ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, I16ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, I32ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, I64ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, I128ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, F32ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, F64ReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, CharReflRepresenter::default());
    set_common_generic_item_representers_and_self(reg, StringReflRepresenter::default());
}

pub fn set_common_generic_representers<T: 'static>(reg: &mut ReflRepresenterRegistry) {
    reg.set(Box::new(OptionReflRepresenter::<T>::default()));
    reg.set(Box::new(VecReflRepresenter::<T>::default()));
}

pub fn set_common_generic_item_representers_and_self<T: 'static + ReflRepresenter>(
    reg: &mut ReflRepresenterRegistry,
    representer: T,
) {
    reg.set(Box::new(representer));
    set_common_generic_representers::<T::Item>(reg);
}

macro_rules! refl_representer_primitive {
    ($name: ident, $refl_prim: ident, $ty: ident) => {
        #[derive(Default)]
        struct $name;
        impl ReflRepresenter for $name {
            type Item = $ty;

            fn item_name(&self, _ctx: &ReflRepresenterCtx) -> String {
                String::from(stringify!($ty))
            }

            fn into_repr(&self, _ctx: &ReflRepresenterCtx, v: &Self::Item) -> Option<ReflRepr> {
                Some(ReflRepr::Primitive(ReflReprPrimitive::$refl_prim(v.clone() as _)))
            }
        }
    };
}

refl_representer_primitive! {U8ReflRepresenter, Int, u8}
refl_representer_primitive! {U16ReflRepresenter, Int, u16}
refl_representer_primitive! {U32ReflRepresenter, Int, u32}
refl_representer_primitive! {U64ReflRepresenter, Int, u64}
refl_representer_primitive! {U128ReflRepresenter, Int, u128}
refl_representer_primitive! {I8ReflRepresenter, Int, i8}
refl_representer_primitive! {I16ReflRepresenter, Int, i16}
refl_representer_primitive! {I32ReflRepresenter, Int, i32}
refl_representer_primitive! {I64ReflRepresenter, Int, i64}
refl_representer_primitive! {I128ReflRepresenter, Int, i128}
refl_representer_primitive! {F32ReflRepresenter, Float, f32}
refl_representer_primitive! {F64ReflRepresenter, Float, f64}
refl_representer_primitive! {CharReflRepresenter, Char, char}
refl_representer_primitive! {StringReflRepresenter, Str, String}

//

pub struct OptionReflRepresenter<T: 'static>(PhantomData<T>);
impl<T: 'static> OptionReflRepresenter<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<T: 'static> Default for OptionReflRepresenter<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: 'static> ReflRepresenter for OptionReflRepresenter<T> {
    type Item = Option<T>;

    fn item_name(&self, ctx: &ReflRepresenterCtx) -> String {
        format!("Option<{}>", ctx.item_name::<T>().unwrap_or(String::from("")))
    }

    fn into_repr(&self, ctx: &ReflRepresenterCtx, v: &Self::Item) -> Option<ReflRepr> {
        Some(match v {
            Some(field) => ReflRepr::Enum(ReflReprEnum {
                name: self.item_name(ctx),
                variant: String::from("Some"),
                fields: ReflReprFields::Tuple(vec![ctx.into_repr(field)?]),
            }),
            None => ReflRepr::Enum(ReflReprEnum {
                name: self.item_name(ctx),
                variant: String::from("None"),
                fields: ReflReprFields::Unit,
            }),
        })
    }
}

pub struct VecReflRepresenter<T: 'static>(PhantomData<T>);
impl<T: 'static> VecReflRepresenter<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<T: 'static> Default for VecReflRepresenter<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: 'static> ReflRepresenter for VecReflRepresenter<T> {
    type Item = Vec<T>;

    fn item_name(&self, ctx: &ReflRepresenterCtx) -> String {
        format!("Vec<{}>", ctx.item_name::<T>().unwrap_or(String::from("")))
    }

    fn into_repr(&self, ctx: &ReflRepresenterCtx, v: &Self::Item) -> Option<ReflRepr> {
        let mut fields = Vec::new();

        for i in v {
            fields.push(ctx.into_repr(i)?);
        }

        Some(ReflRepr::Struct(ReflReprStruct {
            name: self.item_name(ctx),
            fields: ReflReprFields::Tuple(fields),
        }))
    }
}
