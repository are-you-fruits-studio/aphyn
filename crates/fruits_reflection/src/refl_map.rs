use std::{any::Any, collections::HashMap};

use crate::tuple_from_vec_of_any::TupleFromVecOfAny;

// todo: ffi?

pub enum ReflMapType {
    Struct(ReflMapStruct),
}

pub struct ReflMapStruct {
    factory_raw: *const (),
    deconstructor_raw: *const (),
    factory_wrapper: fn(*const (), Vec<Box<dyn Any>>) -> Option<Box<dyn Any>>,
    deconstructor_wrapper: fn(*const (), Box<dyn Any>) -> Option<Vec<Box<dyn Any>>>,
    pub fields: HashMap<&'static str, ReflMapStructField>,
}

impl ReflMapStruct {
    pub fn new<T: 'static, P: TupleFromVecOfAny>(
        factory: fn(P) -> T,
        deconstructor: fn(T) -> P,
        fields: HashMap<&'static str, ReflMapStructField>,
    ) -> Self {
        Self {
            factory_raw: factory as *const (),
            deconstructor_raw: deconstructor as *const (),
            factory_wrapper: move |f, p| {
                let factory_raw = unsafe { std::mem::transmute::<_, fn(P) -> T>(f) };

                Some(Box::new(factory_raw(P::tuple_from_vec_of_any(p)?)) as Box<dyn Any>)
            },
            deconstructor_wrapper: move |f, v| {
                let deconstructor_raw = unsafe { std::mem::transmute::<_, fn(T) -> P>(f) };

                Some(P::tuple_into_vec_of_any(deconstructor_raw(*v.downcast().ok()?)))
            },
            fields,
        }
    }
    pub fn create(&self, parameters: Vec<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        (self.factory_wrapper)(self.factory_raw, parameters)
    }
    pub fn deconstruct(&self, v: Box<dyn Any>) -> Option<Vec<Box<dyn Any>>> {
        (self.deconstructor_wrapper)(self.deconstructor_raw, v)
    }
}

pub struct ReflMapStructField {
    ref_getter_raw: *const (),
    mut_getter_raw: *const (),
    ref_getter_wrapper: fn(*const (), &dyn Any) -> Option<&dyn Any>,
    mut_getter_wrapper: fn(*const (), &mut dyn Any) -> Option<&mut dyn Any>,
}

impl ReflMapStructField {
    pub fn new<T: 'static, F: 'static>(ref_getter: fn(&T) -> &F, mut_getter: fn(&mut T) -> &mut F) -> Self {
        Self {
            ref_getter_raw: ref_getter as *const (),
            mut_getter_raw: mut_getter as *const (),
            ref_getter_wrapper: |raw, v| {
                let ref_getter_raw = unsafe { std::mem::transmute::<*const (), fn(&T) -> &F>(raw) };

                Some(ref_getter_raw(v.downcast_ref()?))
            },
            mut_getter_wrapper: |raw, v| {
                let mut_getter_raw = unsafe { std::mem::transmute::<*const (), fn(&mut T) -> &mut F>(raw) };

                Some(mut_getter_raw(v.downcast_mut()?))
            },
        }
    }
    pub fn get_ref<'a>(&self, v: &'a dyn Any) -> Option<&'a dyn Any> {
        (self.ref_getter_wrapper)(self.ref_getter_raw, v)
    }
    pub fn get_mut<'a>(&self, v: &'a mut dyn Any) -> Option<&'a mut dyn Any> {
        (self.mut_getter_wrapper)(self.mut_getter_raw, v)
    }
}

// todo: enum
// todo: collections
// todo: generics?
