use aphyn_ffi::{FfiString, FfiVec};
use aphyn_reflection::ReflectPath;
use aphyn_serialization::*;

use crate::{AnimatedLeaf, AnimationClip, AnimationKey, AnimationPath, AnimationTrack, KeyInterpolation};

#[repr(C)]
#[derive(Clone, Default, Serializable)]
pub struct AnimationClipAssetMetadata {
    pub tracks: FfiVec<AnimationTrackMetadata>,
}

#[repr(C)]
#[derive(Clone, Default, Serializable)]
pub struct AnimationTrackMetadata {
    pub entity: FfiString,
    pub component: FfiString,
    pub field: FfiString,
    pub leaf: AnimatedLeaf,
    pub value_type: FfiString,
    pub keys: FfiVec<AnimationKeyMetadata>,
}

#[repr(C)]
#[derive(Clone, Default, Serializable)]
pub struct AnimationKeyMetadata {
    pub time: f32,
    pub value: SerializedValue,
    pub interpolation: KeyInterpolation,
    pub in_tangent: f32,
    pub out_tangent: f32,
    pub auto_tangent: bool,
}

impl AnimationClip {
    // `component` and `value_type` are serialized names; tracks naming an unknown type or an unparsable field path
    // and keys that don't deserialize to the track's type are dropped
    pub fn from_meta(ctx: &mut SerializerCtx<TransSerializerCtxState>, meta: &AnimationClipAssetMetadata) -> Self {
        let mut clip = AnimationClip::new();

        for t in meta.tracks.iter() {
            let Some(field) = ReflectPath::parse(&t.field) else {
                eprintln!("animation clip: invalid field path `{}`", t.field);
                continue;
            };
            let registry = ctx.registry();
            let (Some(component), Some(value_type)) = (
                registry.type_id_of_name(&t.component),
                registry.type_id_of_name(&t.value_type),
            ) else {
                eprintln!(
                    "animation clip: unknown component `{}` or value type `{}`",
                    t.component, t.value_type
                );
                continue;
            };
            let path = AnimationPath {
                entity: t.entity.clone(),
                component,
                field,
                leaf: t.leaf,
            };
            let mut track = AnimationTrack::new(path, value_type);

            for k in t.keys.iter() {
                let Some(value) = ctx.deserialize_default_any_by_id(value_type, "", &k.value) else {
                    continue;
                };
                track.keys.push(AnimationKey {
                    time: k.time,
                    value,
                    interpolation: k.interpolation,
                    in_tangent: k.in_tangent,
                    out_tangent: k.out_tangent,
                    auto_tangent: k.auto_tangent,
                });
            }

            track.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
            clip.tracks.push(track);
        }

        clip
    }

    // tracks whose component or value type has no serializer (so no serialized name) are dropped
    pub fn to_meta(&self, ctx: &mut SerializerCtx<TransSerializerCtxState>) -> AnimationClipAssetMetadata {
        let registry = ctx.registry();
        let tracks = self
            .tracks
            .iter()
            .filter_map(|t| {
                let (Some(component), Some(value_type)) = (registry.name_of(t.path.component), registry.name_of(t.value_type))
                else {
                    eprintln!("animation clip: a track's component or value type has no serialized name");
                    return None;
                };
                Some((t, component, value_type))
            })
            .map(|(t, component, value_type)| AnimationTrackMetadata {
                entity: t.path.entity.clone(),
                component: component.into(),
                field: t.path.field.to_string().into(),
                leaf: t.path.leaf,
                value_type: value_type.into(),
                keys: t
                    .keys
                    .iter()
                    .map(|k| AnimationKeyMetadata {
                        time: k.time,
                        value: ctx.serialize_any(k.value.as_any_ref(), ""),
                        interpolation: k.interpolation,
                        in_tangent: k.in_tangent,
                        out_tangent: k.out_tangent,
                        auto_tangent: k.auto_tangent,
                    })
                    .collect(),
            })
            .collect();

        AnimationClipAssetMetadata { tracks }
    }
}
