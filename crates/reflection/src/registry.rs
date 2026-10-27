use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::{
    refl_repr::{ReflRepr, ReflReprFields, ReflReprStruct},
    refl_repr_reg_entries,
    refl_ty::{ReflTy, ReflTyId},
};

// todo: ffi?

#[derive(Default)]
pub struct ReflTyRegistry {
    pub types: HashMap<ReflTyId, ReflTy>,
}

impl ReflTyRegistry {
    pub fn set(&mut self, id: ReflTyId, ty: ReflTy) {
        self.types.insert(id, ty);
    }

    pub fn get(&self, id: &ReflTyId) -> Option<&ReflTy> {
        self.types.get(id)
    }
}

#[derive(Default)]
pub struct ReflRepresenterRegistry {
    pub representers: HashMap<TypeId, Box<dyn Any>>,
}

impl ReflRepresenterRegistry {
    pub fn set<T: 'static>(&mut self, representer: Box<dyn ReflRepresenter<Item = T>>) {
        let any = Box::new(representer) as Box<dyn Any>;

        self.representers.insert(TypeId::of::<T>(), any);
    }
    pub fn get<T: 'static>(&self) -> Option<&dyn ReflRepresenter<Item = T>> {
        let any = self.representers.get(&TypeId::of::<T>())?;

        Some(&**(any.downcast_ref::<Box<dyn ReflRepresenter<Item = T>>>()?))
    }

    pub fn item_name<T: 'static>(&self) -> Option<String> {
        Some(self.get::<T>()?.item_name(&ReflRepresenterCtx { reg: self }))
    }
    pub fn into_repr<T: 'static>(&self, v: &T) -> Option<ReflRepr> {
        self.get::<T>()?.into_repr(&ReflRepresenterCtx { reg: self }, v)
    }
    pub fn from_repr<T: 'static>(&self, v: &ReflRepr) -> Option<T> {
        self.get::<T>()?.from_repr(&ReflRepresenterCtx { reg: self }, v)
    }
}

pub struct ReflRepresenterCtx<'a> {
    reg: &'a ReflRepresenterRegistry,
}

impl<'a> ReflRepresenterCtx<'a> {
    pub fn item_name<T: 'static>(&self) -> Option<String> {
        Some(self.reg.get::<T>()?.item_name(self))
    }
    pub fn into_repr<T: 'static>(&self, v: &T) -> Option<ReflRepr> {
        self.reg.get::<T>()?.into_repr(self, v)
    }
    pub fn from_repr<T: 'static>(&self, v: &ReflRepr) -> Option<T> {
        self.reg.get::<T>()?.from_repr(self, v)
    }
}

pub trait ReflRepresenter {
    type Item;

    fn item_name(&self, ctx: &ReflRepresenterCtx) -> String;
    fn into_repr(&self, ctx: &ReflRepresenterCtx, v: &Self::Item) -> Option<ReflRepr>;
    fn from_repr(&self, ctx: &ReflRepresenterCtx, v: &ReflRepr) -> Option<Self::Item> {
        None
    }
}

// example

#[derive(Debug)]
pub struct ExampleStruct {
    pub age: u8,
    pub name: Option<String>,
}

pub fn registry_use_case() {
    let mut reg = ReflRepresenterRegistry::default();

    refl_repr_reg_entries::set_common_representers(&mut reg);

    reg.set(Box::new(ExampleStructReflRepresenter::default()));

    let example_struct = ExampleStruct {
        age: 55,
        name: Some(String::from("Barry")),
    };

    let repr = reg.into_repr(&example_struct).unwrap();

    println!("{:?}", repr);
    println!("{:?}", example_struct);
}

#[derive(Default)]
struct ExampleStructReflRepresenter;
impl ReflRepresenter for ExampleStructReflRepresenter {
    type Item = ExampleStruct;

    fn item_name(&self, _ctx: &ReflRepresenterCtx) -> String {
        String::from("ExampleStruct")
    }

    fn into_repr(&self, ctx: &ReflRepresenterCtx, v: &Self::Item) -> Option<ReflRepr> {
        Some(ReflRepr::Struct(ReflReprStruct {
            name: String::from("ExampleStruct"),
            fields: ReflReprFields::Named(vec![
                (String::from("age"), ctx.into_repr(&v.age)?),
                (String::from("name"), ctx.into_repr(&v.name)?),
            ]),
        }))
    }
}
