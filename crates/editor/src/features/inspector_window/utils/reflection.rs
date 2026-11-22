use std::collections::HashMap;

use crate::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InspectorLeafKind {
    ReadOnly,
    Bool,
    Int,
    Float,
    String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InspectorContainerKind {
    Fields,
    List,
    Map,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectorEnum {
    pub variant: FfiString,
    pub variants: FfiVec<FfiString>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InspectorValue {
    Leaf {
        text: FfiString,
        kind: InspectorLeafKind,
    },
    Container {
        kind: InspectorContainerKind,
        is_rigid: bool,
        enum_variants: Option<InspectorEnum>,
        entries: Vec<(FfiString, InspectorValue)>,
    },
}

impl InspectorValue {
    pub fn read_only(text: impl Into<FfiString>) -> Self {
        Self::Leaf {
            text: text.into(),
            kind: InspectorLeafKind::ReadOnly,
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf { .. })
    }
}

pub enum InspectorEdit {
    Leaf { kind: InspectorLeafKind, text: FfiString },
    Variant(FfiString),
    AddElement,
    RemoveLastElement,
    Replace(FfiAny),
}

// leaves are converted to and from text through the serializer registry
pub fn build_inspector_value(registry: &ReflectionRegistry, state: TransSerializerCtxState, value: FfiAnyRef) -> InspectorValue {
    let type_name = value.type_info().short().name();

    let Some(reflector) = registry.get(type_name) else {
        return InspectorValue::read_only(format!("not reflectable: {type_name}"));
    };

    let build = |value: Option<FfiAnyRef>| match value {
        Some(value) => build_inspector_value(registry, state, value),
        None => InspectorValue::read_only("<inaccessible>"),
    };

    match &**reflector {
        Reflector::Value(_) => {
            let (text, kind) = leaf_text(state, value);
            InspectorValue::Leaf { text, kind }
        }
        Reflector::Struct(r) => InspectorValue::Container {
            kind: InspectorContainerKind::Fields,
            is_rigid: true,
            enum_variants: None,
            entries: r.fields().map(|f| (f.name().into(), build(f.get(value)))).collect(),
        },
        Reflector::Enum(r) => {
            let active = r.active_variant(value);

            InspectorValue::Container {
                kind: InspectorContainerKind::Fields,
                is_rigid: true,
                enum_variants: Some(InspectorEnum {
                    variant: active.map(|v| FfiString::from(v.name())).unwrap_or_default(),
                    variants: r.variants().map(|v| FfiString::from(v.name())).collect(),
                }),
                entries: active
                    .map(|v| v.fields().map(|f| (f.name().into(), build(f.get(value)))).collect())
                    .unwrap_or_default(),
            }
        }
        Reflector::List(r) => InspectorValue::Container {
            kind: InspectorContainerKind::List,
            is_rigid: r.is_rigid(),
            enum_variants: None,
            entries: (0..r.len(value).unwrap_or(0))
                .map(|i| (FfiString::from(i.to_string()), build(r.get(value, i))))
                .collect(),
        },
        Reflector::Map(r) => InspectorValue::Container {
            kind: InspectorContainerKind::Map,
            is_rigid: r.is_rigid(),
            enum_variants: None,
            entries: r
                .keys(value)
                .unwrap_or_default()
                .into_iter()
                .map(|key| (leaf_text(state, key).0, build(r.get(value, key))))
                .collect(),
        },
    }
}

pub fn leaf_text(state: TransSerializerCtxState, value: FfiAnyRef) -> (FfiString, InspectorLeafKind) {
    let mut err_handler = |_err: SerializationError| ();
    let serialized = state.into_ctx(&mut err_handler).serialize_any(value, "");

    match serialized {
        SerializedValue::Primitive(SerializedPrimitive::Bool(v)) => (v.to_string().into(), InspectorLeafKind::Bool),
        SerializedValue::Primitive(SerializedPrimitive::Int(v)) => (v.to_string().into(), InspectorLeafKind::Int),
        SerializedValue::Primitive(SerializedPrimitive::Float(v)) => (v.to_string().into(), InspectorLeafKind::Float),
        SerializedValue::Primitive(SerializedPrimitive::String(v)) => (v, InspectorLeafKind::String),
        _ => (value.type_info().short().name().into(), InspectorLeafKind::ReadOnly),
    }
}

pub fn parse_leaf_text(kind: InspectorLeafKind, text: &str) -> Option<SerializedPrimitive> {
    match kind {
        InspectorLeafKind::ReadOnly => None,
        InspectorLeafKind::Bool => Some(SerializedPrimitive::Bool(text.trim().eq_ignore_ascii_case("true"))),
        InspectorLeafKind::Int => Some(SerializedPrimitive::Int(text.trim().parse().unwrap_or(0))),
        InspectorLeafKind::Float => Some(SerializedPrimitive::Float(text.trim().parse().unwrap_or(0.0))),
        InspectorLeafKind::String => Some(SerializedPrimitive::String(text.into())),
    }
}

fn write_leaf(state: TransSerializerCtxState, value: FfiAnyMut, primitive: SerializedPrimitive) {
    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    state
        .into_ctx(&mut err_handler)
        .deserialize_any(value, "", &SerializedValue::Primitive(primitive));
}

pub fn apply_inspector_edit(
    registry: &ReflectionRegistry,
    state: TransSerializerCtxState,
    root: FfiAnyMut,
    path: &ReflectPath,
    edit: InspectorEdit,
) -> bool {
    let Some(mut value) = resolve_mut(registry, root, path.segments()) else {
        return false;
    };
    let Some(reflector) = registry.get(value.type_info().short().name()).cloned() else {
        return false;
    };

    match edit {
        InspectorEdit::Leaf { kind, text } => {
            let Some(primitive) = parse_leaf_text(kind, &text) else {
                return false;
            };
            write_leaf(state, value, primitive);
            true
        }
        InspectorEdit::Variant(name) => {
            let Some(variant) = reflector.as_enum().and_then(|r| r.variant(&name)) else {
                return false;
            };
            if variant.is_active(value.as_any_ref()) {
                return false;
            }
            let Some(constructed) = reflect_construct_variant(registry, variant) else {
                println!("can't construct variant {name} of {}", reflector.type_name());
                return false;
            };
            value.replace(constructed).is_ok()
        }
        InspectorEdit::AddElement => match &*reflector {
            Reflector::List(r) => {
                let (Some(len), Some(element)) = (r.len(value.as_any_ref()), reflect_default(registry, r.element_type_name()))
                else {
                    return false;
                };
                r.insert(value, len, element).is_ok()
            }
            Reflector::Map(r) => {
                let Some(element) = reflect_default(registry, r.value_type_name()) else {
                    return false;
                };
                let Some(key) = free_map_key(registry, state, r, value.as_any_ref()) else {
                    return false;
                };
                r.insert(value, key, element).is_ok()
            }
            _ => false,
        },
        InspectorEdit::RemoveLastElement => match &*reflector {
            Reflector::List(r) => match r.len(value.as_any_ref()).and_then(|l| l.checked_sub(1)) {
                Some(last) => r.remove(value, last).is_some(),
                None => false,
            },
            Reflector::Map(r) => {
                let last_key = {
                    let keys = r.keys(value.as_any_ref()).unwrap_or_default();
                    let Some(&last) = keys.as_slice().last() else {
                        return false;
                    };
                    reflect_clone(registry, last)
                };
                match last_key {
                    Some(key) => r.remove(value, key.as_any_ref()).is_some(),
                    None => false,
                }
            }
            _ => false,
        },
        InspectorEdit::Replace(new_value) => value.replace(new_value).is_ok(),
    }
}

// the key type's default, or else the first of "0", "1", ... the key type deserializes to that is still free
fn free_map_key(registry: &ReflectionRegistry, state: TransSerializerCtxState, r: &MapReflector, map: FfiAnyRef) -> Option<FfiAny> {
    let key_type = r.key_type_name();
    let is_free = |key: &FfiAny| r.get(map, key.as_any_ref()).is_none();

    let key = reflect_default(registry, key_type)?;
    if is_free(&key) {
        return Some(key);
    }

    let (_, kind) = leaf_text(state, key.as_any_ref());

    for i in 0..1024_u32 {
        let mut key = reflect_default(registry, key_type)?;
        let primitive = match kind {
            InspectorLeafKind::String => SerializedPrimitive::String(i.to_string().into()),
            InspectorLeafKind::Int => SerializedPrimitive::Int(i as i128),
            InspectorLeafKind::Float => SerializedPrimitive::Float(i as f64),
            _ => return None,
        };
        write_leaf(state, key.as_any_mut(), primitive);

        if is_free(&key) {
            return Some(key);
        }
    }

    None
}

pub fn entity_ids_of(ent: EntitiesHolderRef) -> (HashMap<u64, EntityId>, HashMap<EntityId, u64>) {
    let mut id_to_ent = HashMap::new();
    let mut ent_to_id = HashMap::new();

    for entity in ent.query::<EntityId>().iter() {
        let id = ent.encode_entity_as_id(entity);
        id_to_ent.insert(id, entity);
        ent_to_id.insert(entity, id);
    }

    (id_to_ent, ent_to_id)
}

pub fn with_component_mut<R>(
    mut ent: EntitiesHolderMut,
    entity: EntityId,
    type_name: &str,
    f: impl FnOnce(FfiAnyMut) -> R,
) -> Option<R> {
    let mut component = None;

    ent.get_all_components_mut(entity, |c| {
        if component.is_none() && c.type_info().short().name() == type_name {
            component = Some(c);
        }
    });

    component.map(f)
}

pub fn with_component<R>(ent: EntitiesHolderRef, entity: EntityId, type_name: &str, f: impl FnOnce(FfiAnyRef) -> R) -> Option<R> {
    let mut component = None;

    ent.get_all_components(entity, |c| {
        if component.is_none() && c.type_info().short().name() == type_name {
            component = Some(c);
        }
    });

    component.map(f)
}

// engine data (components, asset metadata) is Send + Sync; FfiAny itself can't promise it
pub struct EditorValue(pub FfiAny);

unsafe impl Send for EditorValue {}
unsafe impl Sync for EditorValue {}
