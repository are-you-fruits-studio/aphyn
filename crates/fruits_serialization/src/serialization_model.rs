use fruits_ffi::{FfiIndexMap, FfiOption, FfiString, FfiVec};

use crate::decompose_serialization_path;

#[repr(C)]
#[derive(Clone, Debug, Default)]
pub enum SerializedValue {
    #[default]
    Null,
    Primitive(SerializedPrimitive),
    Composite(SerializedComposite),
}

impl Default for &SerializedValue {
    fn default() -> Self {
        &SerializedValue::Null
    }
}

impl SerializedValue {
    pub fn similar(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Primitive(lhs), Self::Primitive(rhs)) => lhs.similar(rhs),
            (Self::Composite(lhs), Self::Composite(rhs)) => lhs.similar(rhs),
            _ => false,
        }
    }

    pub fn similar_option(lhs: Option<&Self>, rhs: Option<&Self>) -> bool {
        match (lhs, rhs) {
            (None, None) => true,
            (Some(lhs), Some(rhs)) => lhs.similar(rhs),
            _ => false,
        }
    }

    pub fn get_by_path(&self, path: &str) -> Option<&SerializedValue> {
        let Some((field_name, field_path)) = decompose_serialization_path(path) else {
            return Some(self);
        };

        match self {
            SerializedValue::Null => None,
            SerializedValue::Primitive(_) => None,
            SerializedValue::Composite(value) => match &value.values {
                SerializedCompositeValues::Map(value_map) => value_map.values.get(field_name)?.get_by_path(field_path),
                SerializedCompositeValues::List(value_list) => value_list.get(field_name.parse::<u64>().ok()?)?.get_by_path(field_path),
            },
        }
    }

    pub fn get_or_insert_by_path(&mut self, path: &str) -> Option<&mut SerializedValue> {
        let Some((field_name, field_path)) = decompose_serialization_path(path) else {
            return Some(self);
        };

        match self {
            SerializedValue::Null => None,
            SerializedValue::Primitive(_) => None,
            SerializedValue::Composite(value) => match &mut value.values {
                SerializedCompositeValues::Map(value_map) => {
                    // todo: optimize
                    if value_map.values.get(field_name).is_none() {
                        value_map.values.insert(field_name.into(), Default::default());
                    }

                    value_map.values.get_mut(field_name).unwrap().get_or_insert_by_path(field_path)
                },
                SerializedCompositeValues::List(value_list) => {
                    let idx = field_name.parse::<u64>().ok()?;
                    while value_list.len() <= idx {
                        value_list.push(Default::default());
                    }
                    value_list.get_mut(idx).unwrap().get_or_insert_by_path(field_path)
                },
            },
        }
    }
}

impl From<SerializedPrimitive> for SerializedValue {
    fn from(value: SerializedPrimitive) -> Self {
        Self::Primitive(value)
    }
}

impl From<SerializedComposite> for SerializedValue {
    fn from(value: SerializedComposite) -> Self {
        Self::Composite(value)
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub enum SerializedPrimitive {
    Bool(bool),
    Int(i128),
    Float(f64),
    String(FfiString),
}
impl SerializedPrimitive {
    pub fn similar(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(lhs), Self::Bool(rhs)) => lhs == rhs,
            (Self::Int(lhs), Self::Int(rhs)) => lhs == rhs,
            (Self::Float(lhs), Self::Float(rhs)) => lhs == rhs,
            (Self::String(lhs), Self::String(rhs)) => lhs == rhs,
            _ => false
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct SerializedComposite {
    pub values: SerializedCompositeValues,
    pub is_rigid: bool,
}
impl SerializedComposite {
    pub fn similar(&self, other: &Self) -> bool {
        self.values.similar(&other.values)
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct SerializedEnumMetadata {
    pub variant: FfiString,
    pub variants: FfiVec<FfiString>,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub enum SerializedCompositeValues {
    Map(SerializedMap),
    List(FfiVec<SerializedValue>),
}
impl SerializedCompositeValues {
    pub fn similar(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Map(lhs), Self::Map(rhs)) => lhs.similar(rhs),
            (Self::List(lhs), Self::List(rhs)) => Self::similar_lists(lhs, rhs),
            _ => false
        }
    }

    fn similar_lists(lhs: &[SerializedValue], rhs: &[SerializedValue]) -> bool {
        if lhs.len() != rhs.len() {
            return false;
        }

        for i in 0..lhs.len() {
            if !lhs[i].similar(&rhs[i]) {
                return false;
            }
        }

        true
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct SerializedMap {
    pub values: FfiIndexMap<FfiString, SerializedValue>,
    pub enum_metadata: FfiOption<SerializedEnumMetadata>,
}
impl SerializedMap {
    pub fn similar(&self, other: &Self) -> bool {
        if self.enum_metadata.as_ref().map(|m| &m.variant) != other.enum_metadata.as_ref().map(|m| &m.variant) {
            return false;
        }

        if self.values.len() != other.values.len() {
            return false;
        }

        for (key, val) in &other.values {
            if !SerializedValue::similar_option(self.values.get(key.as_str()), Some(val)) {
                return false;
            }
        }

        true
    }
}

impl SerializedValue {
    pub fn to_json(&self) -> serde_json::Value {
        let value = self;

        match value {
            Self::Null => serde_json::Value::Null,
            Self::Primitive(value) => {
                match value {
                    SerializedPrimitive::Bool(value) => serde_json::Value::Bool(*value),
                    SerializedPrimitive::Int(value) => serde_json::Number::from_i128((*value).clamp(i64::MIN as i128, u64::MAX as i128)).map(serde_json::Value::Number).unwrap_or_else(|| serde_json::Value::Null),
                    SerializedPrimitive::Float(value) => serde_json::Number::from_f64(*value).map(serde_json::Value::Number).unwrap_or_else(|| serde_json::Value::Null),
                    SerializedPrimitive::String(value) => serde_json::Value::String(value.to_string()),
                }
            },
            Self::Composite(value) => {
                match &value.values {
                    SerializedCompositeValues::List(serialized_values) => {
                        serde_json::Value::Array(serialized_values.iter().map(|v| v.to_json()).collect())
                    },
                    SerializedCompositeValues::Map(serialized_map) => {
                        let mut map = serde_json::Map::new();

                        if let Some(serialized_enum_metadata) = serialized_map.enum_metadata.as_ref() {
                            map.insert(String::from("$enum_variant"), serde_json::Value::String(serialized_enum_metadata.variant.to_string()));
                        }

                        for (key, serialized_value) in &serialized_map.values {
                            if key.as_str() == "$enum_variant" {
                                eprintln!("Reserverd name \"$enum_variant\" is used in SerializedValue, skipped");
                                continue;
                            }

                            map.insert(key.to_string(), serialized_value.to_json());
                        }

                        serde_json::Value::Object(map)
                    },
                }
            },
        }
    }

    pub fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(value) => Self::Primitive(SerializedPrimitive::Bool(*value)),
            serde_json::Value::Number(value) => {
                Self::Primitive(
                    if value.is_u64() {
                        SerializedPrimitive::Int(value.as_u64().unwrap() as i128)
                    } else if value.is_i64() {
                        SerializedPrimitive::Int(value.as_i64().unwrap() as i128)
                    } else {
                        SerializedPrimitive::Float(value.as_f64().unwrap())
                    }
                )
            },
            serde_json::Value::String(value) => Self::Primitive(SerializedPrimitive::String(value.to_string().into())),
            serde_json::Value::Array(value) => {
                Self::Composite(SerializedComposite {
                    is_rigid: false,
                    values: SerializedCompositeValues::List(value.iter().map(Self::from_json).collect())
                })
            },
            serde_json::Value::Object(value) => {
                let mut enum_variant = None;
                let mut map = FfiIndexMap::new();

                for (key, json_value) in value {
                    if key.as_str() == "$enum_variant" {
                        if let serde_json::Value::String(json_value) = json_value {
                            enum_variant = Some(json_value.clone());
                        } else {
                            eprintln!("Tried to deserialize invalid \"$enum_variant\"");
                        }

                        continue;
                    }

                    map.insert(FfiString::from(key.to_string()), SerializedValue::from_json(json_value));
                }

                Self::Composite(SerializedComposite {
                    values: SerializedCompositeValues::Map(SerializedMap {
                        values: map,
                        enum_metadata: enum_variant.map(|v| SerializedEnumMetadata {
                            variant: v.into(),
                            variants: FfiVec::new(),
                        }).into()
                    }),
                    is_rigid: false,
                })
            },
        }
    }
}
