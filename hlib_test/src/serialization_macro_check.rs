use fruits_engine::*;

#[derive(Serializable)]
pub struct SomeStruct<'a, 'b, T>
    where T : Copy
{
    name: &'b str,
    age: &'a u32,
    data: T,
    unit: SomeUnit,
}

impl<'a, 'b, T: Copy + Default> Default for SomeStruct<'a, 'b, T> {
    fn default() -> Self {
        Self {
            name: Default::default(),
            age: &0,
            data: Default::default(),
            unit: Default::default(),
        }
    }
}

#[derive(Serializable, Default)]
pub struct SomeTuple(String, u32);

#[derive(Serializable, Default)]
pub struct SomeUnit;

#[derive(Serializable, Default)]
pub enum SomeEnum {
    #[default]
    A,
    B(u32),
    C { name: String },
    D(String, String, u32, Option<u32>),
    E { name1: String, password: String, age: Option<u8> }
}
