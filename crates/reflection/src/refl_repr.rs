use std::fmt::Debug;

// todo: ffi?

#[derive(Clone)]
pub enum ReflRepr {
    Struct(ReflReprStruct),
    Enum(ReflReprEnum),
    Primitive(ReflReprPrimitive),
}

#[derive(Clone)]
pub enum ReflReprPrimitive {
    Int(i128),
    Float(f64),
    Char(char),
    Str(String),
    Bool(bool),
    Unit,
}

#[derive(Clone)]
pub struct ReflReprStruct {
    pub name: String,
    pub fields: ReflReprFields,
}

#[derive(Clone)]
pub enum ReflReprFields {
    Unit,
    Tuple(Vec<ReflRepr>),
    Named(Vec<(String, ReflRepr)>),
}

#[derive(Clone)]
pub struct ReflReprEnum {
    pub name: String,
    pub variant: String,
    pub fields: ReflReprFields,
}

impl Debug for ReflRepr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReflRepr::Struct(repr) => {
                write!(f, "{} {:?}", &repr.name, &repr.fields)?;
            }
            ReflRepr::Enum(repr) => {
                write!(f, "{}::{} {:?}", &repr.name, &repr.variant, &repr.fields)?;
            }
            ReflRepr::Primitive(repr) => match repr {
                ReflReprPrimitive::Int(repr) => write!(f, "{}", repr)?,
                ReflReprPrimitive::Float(repr) => write!(f, "{}", repr)?,
                ReflReprPrimitive::Char(repr) => write!(f, "'{}'", repr)?,
                ReflReprPrimitive::Str(repr) => write!(f, "\"{}\"", repr)?,
                ReflReprPrimitive::Bool(repr) => write!(f, "{}", repr)?,
                ReflReprPrimitive::Unit => write!(f, "()")?,
            },
        }

        Ok(())
    }
}

impl Debug for ReflReprFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReflReprFields::Unit => (),
            ReflReprFields::Tuple(fields) => {
                write!(f, "(")?;
                for (i, field_repr) in fields.iter().enumerate() {
                    write!(f, "{:?}", field_repr)?;
                    if i < fields.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ")")?;
            }
            ReflReprFields::Named(fields) => {
                write!(f, "{{ ")?;
                for (i, (field_name, field_repr)) in fields.iter().enumerate() {
                    write!(f, "{}: {:?}", field_name, field_repr)?;
                    if i < fields.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, " }}")?;
            }
        }

        Ok(())
    }
}

// todo: generics?
// todo: collections
