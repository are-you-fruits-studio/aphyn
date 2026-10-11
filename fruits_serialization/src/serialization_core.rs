use std::{error::Error, fmt::{Debug, Display}};

use fruits_ffi::{FfiIndexMap, FfiOption, FfiString, FfiVec};

use crate::{
    SerializedComposite, SerializedCompositeValues, SerializedEnumMetadata, SerializedMap, SerializedValue,
    normalize_serialization_path,
};

#[repr(C)]
#[derive(Clone)]
pub enum SerializationError {
    MissingSerializer { type_name: FfiString },
    InvalidInput { message: FfiString },
}
impl Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSerializer { type_name } => {
                write!(f, "missing serializer for type {}", type_name)
            }
            Self::InvalidInput { message } => write!(f, "invalid serialization input: {}", message),
        }
    }
}
impl Debug for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <SerializationError as Display>::fmt(&self, f)
    }
}
impl Error for SerializationError {}

// todo: ffi
enum CoreMapSerializerCtxState<'a> {
    Root {
        fields: FfiIndexMap<FfiString, SerializedValue>,
    },
    Field {
        field_name: &'a str,
        field_path: &'a str,
        serialized: Option<SerializedValue>,
    },
}

// todo: ffi
pub struct CoreMapSerializerCtx<'a> {
    state: CoreMapSerializerCtxState<'a>,
}

impl<'a> CoreMapSerializerCtx<'a> {
    pub fn new(path: Option<(&'a str, &'a str)>) -> Self {
        Self {
            state: match path {
                Some((field_name, field_path)) => CoreMapSerializerCtxState::Field {
                    field_name,
                    field_path,
                    serialized: None,
                },
                None => CoreMapSerializerCtxState::Root {
                    fields: Default::default(),
                },
            },
        }
    }

    pub fn add_field(&mut self, name: &str, serializer: impl FnOnce(&str) -> SerializedValue) {
        match &mut self.state {
            CoreMapSerializerCtxState::Root { fields } => {
                fields.insert(name.into(), serializer(""));
            }
            CoreMapSerializerCtxState::Field {
                field_name,
                field_path,
                serialized,
            } => {
                if serialized.is_none() && *field_name == name {
                    *serialized = Some(serializer(field_path));
                }
            }
        }
    }

    pub fn finish_as_map(self, is_rigid: bool) -> SerializedValue {
        match self.state {
            CoreMapSerializerCtxState::Root { fields } => SerializedValue::Composite(SerializedComposite {
                values: SerializedCompositeValues::Map(SerializedMap {
                    values: fields,
                    enum_metadata: None.into(),
                }),
                is_rigid,
            }),
            CoreMapSerializerCtxState::Field { serialized, .. } => serialized.unwrap_or_default(),
        }
    }

    pub fn finish_as_enum(self, is_rigid: bool, variant: impl Into<FfiString>, variants: FfiVec<FfiString>) -> SerializedValue {
        match self.state {
            CoreMapSerializerCtxState::Root { fields } => SerializedValue::Composite(SerializedComposite {
                values: SerializedCompositeValues::Map(SerializedMap {
                    values: fields,
                    enum_metadata: SerializedEnumMetadata {
                        variant: variant.into(),
                        variants,
                    }
                    .into(),
                }),
                is_rigid,
            }),
            CoreMapSerializerCtxState::Field { serialized, .. } => serialized.unwrap_or_default(),
        }
    }
}

//

// todo: ffi
enum CoreListSerializerCtxState<'a> {
    Root {
        elements: FfiVec<SerializedValue>,
    },
    Element {
        element_idx: Option<u64>,
        element_path: &'a str,
        serialized: Option<SerializedValue>,
    },
}

// todo: ffi
pub struct CoreListSerializerCtx<'a> {
    state: CoreListSerializerCtxState<'a>,
}

impl<'a> CoreListSerializerCtx<'a> {
    pub fn new(path: Option<(&'a str, &'a str)>) -> Self {
        Self {
            state: match path {
                Some((element_name, element_path)) => CoreListSerializerCtxState::Element {
                    element_idx: element_name.parse().ok(),
                    element_path,
                    serialized: None,
                },
                None => CoreListSerializerCtxState::Root {
                    elements: Default::default(),
                },
            },
        }
    }

    // todo: to self-counting
    pub fn add_element(&mut self, idx: usize, serializer: impl FnOnce(&str) -> SerializedValue) {
        let idx = idx as u64;

        match &mut self.state {
            CoreListSerializerCtxState::Root { elements } => {
                elements.push(serializer(""));
            }
            CoreListSerializerCtxState::Element {
                element_idx,
                element_path,
                serialized,
            } => {
                if serialized.is_none() && *element_idx == Some(idx) {
                    *serialized = Some(serializer(element_path));
                }
            }
        }
    }

    pub fn finish_as_list(self, is_rigid: bool) -> SerializedValue {
        match self.state {
            CoreListSerializerCtxState::Root { elements } => SerializedValue::Composite(SerializedComposite {
                values: SerializedCompositeValues::List(elements),
                is_rigid,
            }),
            CoreListSerializerCtxState::Element { serialized, .. } => serialized.unwrap_or_default(),
        }
    }
}

enum CoreMapDeserializerCtxState<'a> {
    Root {
        fields: Option<&'a FfiIndexMap<FfiString, SerializedValue>>,
    },
    Field {
        field_name: &'a str,
        field_path: &'a str,
        value: &'a SerializedValue,
    },
}

// todo: ffi
// todo: merge with state?
pub struct CoreMapDeserializerCtx<'a> {
    state: CoreMapDeserializerCtxState<'a>,
}

impl<'a> CoreMapDeserializerCtx<'a> {
    pub fn new(path: Option<(&'a str, &'a str)>, value: &'a SerializedValue) -> Self {
        let fields = match value {
            SerializedValue::Composite(SerializedComposite {
                values: SerializedCompositeValues::Map(value),
                ..
            }) => Some(&value.values),
            _ => None,
        };

        let state = match path {
            None => CoreMapDeserializerCtxState::Root { fields },
            Some((field_name, field_path)) => CoreMapDeserializerCtxState::Field {
                field_name,
                field_path,
                value,
            },
        };

        Self { state }
    }

    pub fn add_field(&mut self, name: &str, deserializer: impl FnOnce(&str, &SerializedValue)) {
        match &mut self.state {
            CoreMapDeserializerCtxState::Root { fields } => {
                if let Some(serialized) = fields.map(|f| f.get(name)).flatten() {
                    deserializer("", serialized);
                }
            }
            CoreMapDeserializerCtxState::Field {
                field_name,
                field_path,
                value: serialized,
            } => {
                if name == *field_name {
                    deserializer(*field_path, serialized);
                }
            }
        }
    }
}

// todo: ffi
enum CoreListDeserializerCtxState<'a> {
    Root {
        elements: Option<&'a FfiVec<SerializedValue>>,
    },
    Element {
        element_idx: Option<u64>,
        element_path: &'a str,
        value: &'a SerializedValue,
    },
}

pub(crate) fn deserialize_list_inverted(
    path: Option<(&str, &str)>,
    serialized: &SerializedValue,
    mut deserializer: impl FnMut(usize, &str, &SerializedValue),
) {
    match path {
        // root
        None => {
            let SerializedValue::Composite(SerializedComposite {
                values: SerializedCompositeValues::List(list), ..
            }) = serialized else {
                return;
            };

            for (slice_idx, serialized) in list.iter().enumerate() {
                deserializer(slice_idx, "", serialized);
            }
        },
        // element
        Some((element_name, element_path)) => {
            let Ok(element_idx) = element_name.parse::<u64>() else {
                return;
            };

            deserializer(element_idx as usize, element_path, serialized);
        },
    };
}

pub(crate) fn deserialize_map_inverted(
    path: Option<(&str, &str)>,
    serialized: &SerializedValue,
    mut deserializer: impl FnMut(&str, &str, &SerializedValue),
) {
    match path {
        // root
        None => {
            let SerializedValue::Composite(SerializedComposite {
                values: SerializedCompositeValues::Map(SerializedMap { values, .. }), ..
            }) = serialized else {
                return;
            };

            for (field_name, serialized) in values {
                deserializer(field_name, "", serialized);
            }
        },
        // element
        Some((element_name, element_path)) => {
            deserializer(element_name, element_path, serialized);
        },
    };
}

// todo: ffi
pub struct CoreListDeserializerCtx<'a> {
    state: CoreListDeserializerCtxState<'a>,
}

impl<'a> CoreListDeserializerCtx<'a> {
    pub fn new(path: Option<(&'a str, &'a str)>, value: &'a SerializedValue) -> Self {
        let elements = match value {
            SerializedValue::Composite(SerializedComposite {
                values: SerializedCompositeValues::List(value),
                ..
            }) => Some(value),
            _ => None,
        };

        let state = match path {
            None => CoreListDeserializerCtxState::Root { elements },
            Some((element_name, element_path)) => CoreListDeserializerCtxState::Element {
                element_idx: element_name.parse::<u64>().ok(),
                element_path,
                value,
            },
        };

        Self { state }
    }

    pub fn add_element(&mut self, idx: usize, deserializer: impl FnOnce(&str, &SerializedValue)) {
        let idx = idx as u64;

        match &mut self.state {
            CoreListDeserializerCtxState::Root { elements } => {
                if let Some(serialized) = elements.map(|f| f.get(idx)).flatten() {
                    deserializer("", serialized);
                }
            }
            CoreListDeserializerCtxState::Element {
                element_idx,
                element_path,
                value: serialized,
            } => {
                if let Some(element_idx) = element_idx
                    && idx == *element_idx
                {
                    deserializer(*element_path, serialized);
                }
            }
        }
    }
}

enum CoreEnumDeserializerCtxState<'a, T> {
    Root {
        expected_variant: Option<&'a str>,
        variant: Option<Box<dyn 'a + FnOnce() -> T>>,
        is_variant_best: bool,
    },
    Field {
        non_empty_path: &'a str,
    },
}

// todo: ffi
pub struct CoreEnumDeserializerCtx<'a, T> {
    value: &'a SerializedValue,
    state: CoreEnumDeserializerCtxState<'a, T>,
}

impl<'a, T> CoreEnumDeserializerCtx<'a, T> {
    pub fn new(path: &'a str, value: &'a SerializedValue) -> Self {
        let path = normalize_serialization_path(path);

        let state = if path.is_empty() {
            let expected_variant = if let SerializedValue::Composite(SerializedComposite {
                values:
                    SerializedCompositeValues::Map(SerializedMap {
                        enum_metadata: FfiOption::Some(enum_metadata),
                        ..
                    }),
                ..
            }) = value
            {
                Some(enum_metadata.variant.as_str())
            } else {
                None
            };

            CoreEnumDeserializerCtxState::Root {
                expected_variant,
                variant: None,
                is_variant_best: false,
            }
        } else {
            CoreEnumDeserializerCtxState::Field { non_empty_path: path }
        };

        Self { value, state }
    }

    pub fn add_variant(&mut self, variant: impl Into<FfiString>, default: impl 'a + FnOnce() -> T) {
        match &mut self.state {
            CoreEnumDeserializerCtxState::Root {
                expected_variant,
                variant: stored_variant,
                is_variant_best,
            } => {
                if !*is_variant_best {
                    let variant = variant.into();

                    if stored_variant.is_none() {
                        *is_variant_best = Some(variant.as_str()) == *expected_variant;
                        *stored_variant = Some(Box::new(default));
                    } else if Some(variant.as_str()) == *expected_variant {
                        *is_variant_best = true;
                        *stored_variant = Some(Box::new(default));
                    }
                }
            }
            CoreEnumDeserializerCtxState::Field { .. } => {}
        }
    }

    pub fn finish<S>(
        self,
        deserialized: &mut T,
        state: &mut S,
        deserializer: impl for<'b> FnOnce(&mut S, &'b mut T, &'b str, &SerializedValue),
        err_reporder: impl FnOnce(&mut S, SerializationError),
    ) {
        let path = match self.state {
            CoreEnumDeserializerCtxState::Root {
                variant,
                is_variant_best,
                ..
            } => {
                if !is_variant_best {
                    err_reporder(
                        state,
                        SerializationError::InvalidInput {
                            message: format!("Deserializing enum {} without a matching variant", std::any::type_name::<T>()).into(),
                        },
                    );
                }

                let Some(variant) = variant else {
                    panic!("failed to deserialize enum {} with no variants", std::any::type_name::<T>());
                };

                *deserialized = variant();
                ""
            }
            CoreEnumDeserializerCtxState::Field { non_empty_path } => non_empty_path,
        };

        deserializer(state, deserialized, path, &self.value);
    }
}
