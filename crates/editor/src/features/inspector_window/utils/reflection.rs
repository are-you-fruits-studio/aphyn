use std::{
    any::type_name,
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use crate::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InspectorLeafKind {
    ReadOnly,
    Bool,
    Int,
    Float,
    String,
    // a float quaternion's Euler angle in degrees; its path is the quaternion's component of the same axis
    Euler(u8),
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
        is_inline: bool,
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

    pub fn is_single_line(&self) -> bool {
        matches!(self, Self::Leaf { .. } | Self::Container { is_inline: true, .. })
    }
}

static INLINE_VECTOR_TYPE_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    macro_rules! vectors_of {
        ($($t:ty),*) => {
            [$([type_name::<Vec2<$t>>(), type_name::<Vec3<$t>>(), type_name::<Vec4<$t>>()]),*]
        };
    }

    vectors_of!(
        f32, f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, bool, char
    )
    .into_iter()
    .flatten()
    .collect()
});

const EULER_ORDER: RotationOrder3D = RotationOrder3D::Zxy;
const QUAT_FIELDS: [&str; 4] = ["x", "y", "z", "w"];

fn is_float_quat(name: &str) -> bool {
    name == type_name::<Quat<f32>>() || name == type_name::<Quat<f64>>()
}

pub enum InspectorEdit {
    Leaf { kind: InspectorLeafKind, text: FfiString },
    Variant(FfiString),
    AddElement,
    RemoveLastElement,
    Replace(AnyBox),
}

// leaves are converted to and from text through the serializer registry
pub fn build_inspector_value(registry: &ReflectionRegistry, state: TransSerializerCtxState, value: AnyRef) -> InspectorValue {
    let type_name = value.type_info().short().name();

    let Some(reflector) = registry.get_for(value) else {
        return InspectorValue::read_only(format!("not reflectable: {type_name}"));
    };

    let build = |value: Option<AnyRef>| match value {
        Some(value) => build_inspector_value(registry, state, value),
        None => InspectorValue::read_only("<inaccessible>"),
    };

    match &**reflector {
        Reflector::Value(_) => {
            let (text, kind) = leaf_text(state, value);
            InspectorValue::Leaf { text, kind }
        }
        Reflector::Struct(r) if is_float_quat(type_name) => match read_float_quat(r, state, value) {
            Some(quat) => InspectorValue::Container {
                kind: InspectorContainerKind::Fields,
                is_rigid: true,
                is_inline: true,
                enum_variants: None,
                entries: quat
                    .to_euler_angles(EULER_ORDER)
                    .into_array()
                    .into_iter()
                    .zip(QUAT_FIELDS)
                    .enumerate()
                    .map(|(axis, (angle, name))| {
                        let text = format!("{}", (angle.to_degrees() * 1e4).round() / 1e4 + 0.0);
                        let leaf = InspectorValue::Leaf {
                            text: text.into(),
                            kind: InspectorLeafKind::Euler(axis as u8),
                        };
                        (FfiString::from(name), leaf)
                    })
                    .collect(),
            },
            None => InspectorValue::read_only("<inaccessible>"),
        },
        Reflector::Struct(r) => InspectorValue::Container {
            kind: InspectorContainerKind::Fields,
            is_rigid: true,
            is_inline: INLINE_VECTOR_TYPE_NAMES.contains(type_name),
            enum_variants: None,
            entries: r.fields().map(|f| (f.name().into(), build(f.get(value)))).collect(),
        },
        Reflector::Enum(r) => {
            let active = r.active_variant(value);

            InspectorValue::Container {
                kind: InspectorContainerKind::Fields,
                is_rigid: true,
                is_inline: false,
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
            is_inline: false,
            enum_variants: None,
            entries: (0..r.len(value).unwrap_or(0))
                .map(|i| (FfiString::from(i.to_string()), build(r.get(value, i))))
                .collect(),
        },
        Reflector::Map(r) => InspectorValue::Container {
            kind: InspectorContainerKind::Map,
            is_rigid: r.is_rigid(),
            is_inline: false,
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

pub fn leaf_text(state: TransSerializerCtxState, value: AnyRef) -> (FfiString, InspectorLeafKind) {
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
        InspectorLeafKind::Float | InspectorLeafKind::Euler(_) => {
            Some(SerializedPrimitive::Float(text.trim().parse().unwrap_or(0.0)))
        }
        InspectorLeafKind::String => Some(SerializedPrimitive::String(text.into())),
    }
}

fn write_leaf(state: TransSerializerCtxState, value: AnyMut, primitive: SerializedPrimitive) {
    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    state
        .into_ctx(&mut err_handler)
        .deserialize_any(value, "", &SerializedValue::Primitive(primitive));
}

pub fn apply_inspector_edit(
    registry: &ReflectionRegistry,
    state: TransSerializerCtxState,
    root: AnyMut,
    path: &ReflectPath,
    edit: InspectorEdit,
) -> bool {
    if let InspectorEdit::Leaf {
        kind: InspectorLeafKind::Euler(axis),
        text,
    } = &edit
    {
        return apply_euler_edit(registry, state, root, path, *axis as usize, text);
    }

    let Some(mut value) = resolve_mut(registry, root, path.segments()) else {
        return false;
    };
    let Some(reflector) = registry.get_for_type(value.type_id(), value.type_info()).cloned() else {
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
                let (Some(len), Some(element)) = (r.len(value.as_any_ref()), reflect_default(registry, r.element_type_id())) else {
                    return false;
                };
                r.insert(value, len, element).is_ok()
            }
            Reflector::Map(r) => {
                let Some(element) = reflect_default(registry, r.value_type_id()) else {
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

fn read_float_quat(r: &StructReflector, state: TransSerializerCtxState, value: AnyRef) -> Option<Quat<f64>> {
    let [x, y, z, w] = QUAT_FIELDS.map(|name| {
        let field = r.field(name)?.get(value)?;
        leaf_text(state, field).0.as_str().parse::<f64>().ok()
    });
    Some(Quat::new(x?, y?, z?, w?))
}

fn apply_euler_edit(
    registry: &ReflectionRegistry,
    state: TransSerializerCtxState,
    root: AnyMut,
    path: &ReflectPath,
    axis: usize,
    text: &str,
) -> bool {
    let Some(mut quat_value) = path.parent().and_then(|p| resolve_mut(registry, root, p.segments())) else {
        return false;
    };
    let Some(reflector) = registry.get_for_type(quat_value.type_id(), quat_value.type_info()).cloned() else {
        return false;
    };
    let (Some(r), Ok(angle)) = (reflector.as_struct(), text.trim().parse::<f64>()) else {
        return false;
    };
    let Some(quat) = read_float_quat(r, state, quat_value.as_any_ref()) else {
        return false;
    };

    let mut angles = quat.to_euler_angles(EULER_ORDER).into_array();
    let Some(edited) = angles.get_mut(axis) else {
        return false;
    };
    *edited = angle.to_radians();
    let quat = Quat::<f64>::from_euler_angles(Vec3::from_array(angles), EULER_ORDER);

    for (name, component) in QUAT_FIELDS.into_iter().zip(quat.into_array()) {
        if let Some(field) = r.field(name).and_then(|f| f.get_mut(quat_value.reborrow())) {
            write_leaf(state, field, SerializedPrimitive::Float(component));
        }
    }
    true
}

// the key type's default, or else the first of "0", "1", ... the key type deserializes to that is still free
fn free_map_key(registry: &ReflectionRegistry, state: TransSerializerCtxState, r: &MapReflector, map: AnyRef) -> Option<AnyBox> {
    let key_type = r.key_type_id();
    let is_free = |key: &AnyBox| r.get(map, key.as_any_ref()).is_none();

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
    component: AnyTypeId,
    f: impl FnOnce(AnyMut) -> R,
) -> Option<R> {
    ent.get_component_any_mut(entity, component).map(f)
}

pub fn with_component<R>(ent: EntitiesHolderRef, entity: EntityId, component: AnyTypeId, f: impl FnOnce(AnyRef) -> R) -> Option<R> {
    ent.get_component_any(entity, component).map(f)
}

// engine data (components, asset metadata) is Send + Sync; AnyBox itself can't promise it
pub struct EditorValue(pub AnyBox);

unsafe impl Send for EditorValue {}
unsafe impl Sync for EditorValue {}
