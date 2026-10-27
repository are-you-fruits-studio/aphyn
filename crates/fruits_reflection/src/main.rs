use fruits_reflection::{
    ReflMapStruct, ReflMapStructField, ReflMapType, ReflRepr, ReflReprFields, ReflReprPrimitive, ReflReprStruct, ReflTy, ReflTyId,
    ReflTyStruct, ReflectTy,
};

fn main() {
    fruits_reflection::registry_use_case()
}

fn use_case_real_type() {
    let refl = example_struct_refl_map();

    let ReflMapType::Struct(refl) = refl else {
        return;
    };

    let struct_value = refl.create(vec![Box::new(5_u8), Box::new(String::from("abc"))]).unwrap();

    println!(
        "{}",
        refl.fields["age"]
            .get_ref(&*struct_value)
            .unwrap()
            .downcast_ref::<u8>()
            .unwrap()
    );
    println!(
        "{}",
        refl.fields["name"]
            .get_ref(&*struct_value)
            .unwrap()
            .downcast_ref::<String>()
            .unwrap()
    );

    let mut refl_fields = refl.deconstruct(struct_value).unwrap();

    println!("");
    println!("{}", refl_fields.remove(0).downcast_ref::<u8>().unwrap());
    println!("{}", refl_fields.remove(0).downcast_ref::<String>().unwrap());
}

fn use_case_repr() {
    let mut refl_repr = example_struct_into_refl_repr(&ExampleStruct {
        age: 23,
        name: String::from("Serhii"),
    });

    println!("{:?}", refl_repr);

    if let ReflRepr::Struct(refl_repr) = &mut refl_repr {
        if let ReflReprFields::Named(fields) = &mut refl_repr.fields {
            if let Some(name) = fields.iter_mut().filter(|(n, _)| n == "name").next() {
                if let ReflRepr::Primitive(ReflReprPrimitive::Str(name)) = &mut name.1 {
                    name.clear();
                    name.push_str("Daria");
                }
            }
        }
    }

    let real_struct = example_struct_from_refl_repr(&refl_repr).unwrap();

    println!("{:?}", real_struct);
}

//

#[derive(Debug, ReflectTy)]
struct ExampleStruct {
    pub age: u8,
    pub name: String,
}

//

fn example_struct_refl_map() -> ReflMapType {
    ReflMapType::Struct(ReflMapStruct::new(
        |(age, name)| ExampleStruct { age, name },
        |v| (v.age, v.name),
        [
            (
                "age",
                ReflMapStructField::new(|s: &ExampleStruct| &s.age, |s: &mut ExampleStruct| &mut s.age),
            ),
            (
                "name",
                ReflMapStructField::new(|s: &ExampleStruct| &s.name, |s: &mut ExampleStruct| &mut s.name),
            ),
        ]
        .into_iter()
        .collect(),
    ))
}

fn example_struct_refl_ty() -> ReflTy {
    ReflTy::Struct(ReflTyStruct::Named(vec![
        (String::from("age"), ReflTyId::of::<u8>()),
        (String::from("name"), ReflTyId::of::<String>()),
    ]))
}

fn example_struct_into_refl_repr(v: &ExampleStruct) -> ReflRepr {
    ReflRepr::Struct(ReflReprStruct {
        name: String::from("ExampleStruct"),
        fields: ReflReprFields::Named(
            [
                (
                    String::from("age"),
                    ReflRepr::Primitive(ReflReprPrimitive::Int(v.age as i128)),
                ),
                (
                    String::from("name"),
                    ReflRepr::Primitive(ReflReprPrimitive::Str(v.name.clone())),
                ),
            ]
            .into_iter()
            .collect(),
        ),
    })
}

fn example_struct_from_refl_repr(v: &ReflRepr) -> Option<ExampleStruct> {
    let ReflRepr::Struct(v) = v else {
        return None;
    };
    let ReflReprFields::Named(v) = &v.fields else {
        return None;
    };

    let ReflRepr::Primitive(ReflReprPrimitive::Int(age)) = &v.iter().find(|(n, _)| n == "age")?.1 else {
        return None;
    };
    let ReflRepr::Primitive(ReflReprPrimitive::Str(name)) = &v.iter().find(|(n, _)| n == "name")?.1 else {
        return None;
    };

    Some(ExampleStruct {
        age: (*age) as u8,
        name: name.clone(),
    })
}
