use aphyn_any::{AnyBox, AnyRef, AnyTypeId};
use aphyn_reflection::*;

use crate::{AnimatedLeaf, AnimationTypes, EULER_ORDER};

pub struct RecordedLeaf {
    pub field: ReflectPath,
    pub leaf: AnimatedLeaf,
    pub value_type: AnyTypeId,
    pub value: AnyBox,
}

// every animatable leaf at or below `path` in `root` (a component), with a clone of its value:
// a quaternion (or a part of one) yields its three Euler angles in degrees, an enum the fields of its
// active variant; leaves the registry can't clone are skipped
pub fn collect_leaves(registry: &ReflectionRegistry, types: &AnimationTypes, root: AnyRef, path: &ReflectPath) -> Vec<RecordedLeaf> {
    let mut leaves = Vec::new();

    if let Some(parent) = path.parent()
        && let Some(value) = resolve(registry, root, parent.segments())
        && types.is_quat(value.type_id())
    {
        push_euler_leaves(types, &mut leaves, value, &parent);
        return leaves;
    }

    if let Some(value) = resolve(registry, root, path.segments()) {
        collect_into(registry, types, &mut leaves, value, path.clone());
    }

    leaves
}

fn collect_into(
    registry: &ReflectionRegistry,
    types: &AnimationTypes,
    leaves: &mut Vec<RecordedLeaf>,
    value: AnyRef,
    path: ReflectPath,
) {
    if types.is_quat(value.type_id()) {
        push_euler_leaves(types, leaves, value, &path);
        return;
    }

    let Some(reflector) = registry.get_for(value) else {
        return;
    };

    match &**reflector {
        Reflector::Value(_) => {
            if let Some(cloned) = reflect_clone(registry, value) {
                leaves.push(RecordedLeaf {
                    field: path,
                    leaf: AnimatedLeaf::Direct,
                    value_type: cloned.type_id(),
                    value: cloned,
                });
            }
        }
        Reflector::Struct(r) => {
            for field in r.fields() {
                if let Some(inner) = field.get(value) {
                    collect_into(
                        registry,
                        types,
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
                            types,
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
                    collect_into(
                        registry,
                        types,
                        leaves,
                        inner,
                        path.join(ReflectPathSegment::Index(idx as u64)),
                    );
                }
            }
        }
        Reflector::Map(r) => {
            for (idx, key) in r.keys(value).unwrap_or_default().iter().enumerate() {
                if let Some(inner) = r.get(value, *key) {
                    collect_into(
                        registry,
                        types,
                        leaves,
                        inner,
                        path.join(ReflectPathSegment::Index(idx as u64)),
                    );
                }
            }
        }
    }
}

fn push_euler_leaves(types: &AnimationTypes, leaves: &mut Vec<RecordedLeaf>, quat: AnyRef, path: &ReflectPath) {
    let Some(quat) = types.quat_to_f64(quat) else {
        return;
    };

    for (axis, angle) in quat.to_euler_angles(EULER_ORDER).into_array().into_iter().enumerate() {
        leaves.push(RecordedLeaf {
            field: path.clone(),
            leaf: AnimatedLeaf::QuatEuler(axis as u8),
            value_type: types.f64_type_id(),
            value: types.f64_value(angle.to_degrees()),
        });
    }
}
