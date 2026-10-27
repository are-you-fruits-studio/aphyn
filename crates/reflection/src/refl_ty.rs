// todo: ffi?

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReflTyId(String);

impl ReflTyId {
    pub fn of<T>() -> Self {
        // todo: use something more reliable
        Self(String::from(std::any::type_name::<T>()))
    }
}

pub enum ReflTy {
    Struct(ReflTyStruct),
    Enum(ReflTyEnum),
    Array(ReflTyId, u64),
    Slice(ReflTyId),
}

pub struct ReflTyEnum {
    pub variants: Vec<(String, ReflTyStruct)>,
}

pub enum ReflTyStruct {
    Unit,
    Tuple(Vec<ReflTyId>),
    Named(Vec<(String, ReflTyId)>),
}

// todo: generics
// todo: collections

// example

struct ExampleStruct {
    pub age: u8,
    pub name: String,
}

fn example_struct_refl_ty() -> ReflTy {
    ReflTy::Struct(ReflTyStruct::Named(
        [
            (String::from("age"), ReflTyId::of::<u8>()),
            (String::from("name"), ReflTyId::of::<String>()),
        ]
        .into_iter()
        .collect(),
    ))
}

enum ExampleEnum {
    None,
    Age(u8),
    Creds { email: String, password: String },
}

fn example_enum_refl_ty() -> ReflTy {
    ReflTy::Enum(ReflTyEnum {
        variants: vec![
            (String::from("None"), ReflTyStruct::Unit),
            (String::from("Age"), ReflTyStruct::Tuple(vec![ReflTyId::of::<u8>()])),
            (
                String::from("Creds"),
                ReflTyStruct::Named(vec![
                    (String::from("email"), ReflTyId::of::<String>()),
                    (String::from("password"), ReflTyId::of::<String>()),
                ]),
            ),
        ],
    })
}

fn option_enum_refl_ty(t: ReflTyId) -> ReflTy {
    ReflTy::Enum(ReflTyEnum {
        variants: vec![
            (String::from("None"), ReflTyStruct::Unit),
            (String::from("Age"), ReflTyStruct::Tuple(vec![t])),
        ],
    })
}

fn array_f32_10_refl_ty() -> ReflTy {
    ReflTy::Array(ReflTyId::of::<f32>(), 10)
}

fn vec_f32_refl_ty() -> ReflTy {
    ReflTy::Slice(ReflTyId::of::<f32>())
}
