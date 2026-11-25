use aphyn_ffi::{FfiAny, FfiAnyRef, FfiString};
use aphyn_reflection::*;

use crate::{AnimatedLeaf, EULER_ORDER, is_quat_type, quat_to_f64};

pub struct RecordedLeaf {
    pub field: ReflectPath,
    pub leaf: AnimatedLeaf,
    pub value_type: FfiString,
    pub value: FfiAny,
}

// every animatable leaf at or below `path` in `root` (a component), with a clone of its value:
// a quaternion (or a part of one) yields its three Euler angles in degrees, an enum the fields of its
// active variant; leaves the registry can't clone are skipped
pub fn collect_leaves(registry: &ReflectionRegistry, root: FfiAnyRef, path: &ReflectPath) -> Vec<RecordedLeaf> {
    let mut leaves = Vec::new();

    if let Some(parent) = path.parent()
        && let Some(value) = resolve(registry, root, parent.segments())
        && is_quat_type(value.type_info().short().name())
    {
        push_euler_leaves(&mut leaves, value, &parent);
        return leaves;
    }

    if let Some(value) = resolve(registry, root, path.segments()) {
        collect_into(registry, &mut leaves, value, path.clone());
    }

    leaves
}

fn collect_into(registry: &ReflectionRegistry, leaves: &mut Vec<RecordedLeaf>, value: FfiAnyRef, path: ReflectPath) {
    let type_name = value.type_info().short().name();

    if is_quat_type(type_name) {
        push_euler_leaves(leaves, value, &path);
        return;
    }

    let Some(reflector) = registry.get(type_name) else {
        return;
    };

    match &**reflector {
        Reflector::Value(_) => {
            if let Some(cloned) = reflect_clone(registry, value) {
                leaves.push(RecordedLeaf {
                    field: path,
                    leaf: AnimatedLeaf::Direct,
                    value_type: type_name.into(),
                    value: cloned,
                });
            }
        }
        Reflector::Struct(r) => {
            for field in r.fields() {
                if let Some(inner) = field.get(value) {
                    collect_into(
                        registry,
                        leaves,
                        inner,
                        path.join(ReflectPathSegment::Field(field.name().into())),
                    );
                }
            }
        }
        Reflector::Enum(r) => {
            if let Some(variant) = r.active_variant(value) {
                for field in variant.fields() {
                    if let Some(inner) = field.get(value) {
                        collect_into(
                            registry,
                            leaves,
                            inner,
                            path.join(ReflectPathSegment::Field(field.name().into())),
                        );
                    }
                }
            }
        }
        Reflector::List(r) => {
            for idx in 0..r.len(value).unwrap_or(0) {
                if let Some(inner) = r.get(value, idx) {
                    collect_into(registry, leaves, inner, path.join(ReflectPathSegment::Index(idx as u64)));
                }
            }
        }
        Reflector::Map(r) => {
            for (idx, key) in r.keys(value).unwrap_or_default().iter().enumerate() {
                if let Some(inner) = r.get(value, *key) {
                    collect_into(registry, leaves, inner, path.join(ReflectPathSegment::Index(idx as u64)));
                }
            }
        }
    }
}

fn push_euler_leaves(leaves: &mut Vec<RecordedLeaf>, quat: FfiAnyRef, path: &ReflectPath) {
    let Some(quat) = quat_to_f64(quat) else {
        return;
    };

    for (axis, angle) in quat.to_euler_angles(EULER_ORDER).into_array().into_iter().enumerate() {
        leaves.push(RecordedLeaf {
            field: path.clone(),
            leaf: AnimatedLeaf::QuatEuler(axis as u8),
            value_type: std::any::type_name::<f64>().into(),
            value: FfiAny::new(angle.to_degrees()),
        });
    }
}
