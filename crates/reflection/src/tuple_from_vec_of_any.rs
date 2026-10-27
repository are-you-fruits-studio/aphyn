use std::any::Any;

pub trait TupleFromVecOfAny: 'static + Sized {
    fn tuple_from_vec_of_any(v: Vec<Box<dyn Any>>) -> Option<Self>;
    fn tuple_into_vec_of_any(self) -> Vec<Box<dyn Any>>;
}

macro_rules! tuple_from_vec_of_any_impl {
    ($($P: ident),*) => {
        impl<$($P: 'static),*> TupleFromVecOfAny for ($($P,)*) {
            fn tuple_from_vec_of_any(mut _v: Vec<Box<dyn Any>>) -> Option<Self> {
                let boxed_array: Box<[Box<dyn Any>; count!($($P),*)]> = _v.into_boxed_slice().try_into().ok()?;

                #[allow(non_snake_case)]
                let [$($P),*] = *boxed_array;

                Some((
                    $(*$P.downcast().ok()?,)*
                ))
            }
            fn tuple_into_vec_of_any(self) -> Vec<Box<dyn Any>> {
                #[allow(non_snake_case)]
                let ($($P,)*) = self;

                vec![
                    $( Box::new($P), )*
                ]
            }
        }
    };
}

macro_rules! count {
    () => { 0 };
    ($t: ident$(, $ts: ident)*) => { 1 + count!($($ts),*) };
}

tuple_from_vec_of_any_impl!();
tuple_from_vec_of_any_impl!(P0);
tuple_from_vec_of_any_impl!(P0, P1);
tuple_from_vec_of_any_impl!(P0, P1, P2);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14);
tuple_from_vec_of_any_impl!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15);
