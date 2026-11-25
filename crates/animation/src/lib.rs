//! # aphyn_animation
//!
//! Keyframe animation of any reflected value: an [`AnimationClip`] asset holds tracks of keys,
//! and an [`AnimationComponent`] on a root entity plays a clip on that entity and the named
//! entities below it.
//!
//! # How to use
//!
//! The subsystem is registered through the engine's default modules (or
//! [`add_animation_module_to`]). Clips are `.asset` files with `"asset_type": "animation_clip"`,
//! loaded like any other asset and usually authored in the editor's Animation window.
//!
//! #### Clips, tracks and keys
//!
//! A track animates one leaf value. Its [`AnimationPath`] has three parts:
//!
//! - `entity`: `/`-joined [`NameComponent`](aphyn_transform::NameComponent) names below the root
//!   (`""` is the root itself, `"Arm/Hand"` the child `Hand` of the root's child `Arm`);
//! - `component`: the component's `AnyTypeId` (its serialized name on disk);
//! - `field`: an [`aphyn_reflection::ReflectPath`] inside the component (fields, list and map indices).
//!
//! A track also stores its value's `AnyTypeId`, and every key holds the value as an `AnyBox` of
//! that type. Conversions through `f64` go through [`AnimationTypes`]: the ids of the numeric and
//! quaternion types in one world's types registry (the module keeps one as a resource). Numbers (`f32`, `f64` and the integer types) interpolate per [`KeyInterpolation`]
//! (`Constant`, `Linear`, or cubic `Hermite` through the keys' in/out tangents, in value per
//! second); integers are rounded to the nearest value. Any other type steps from key to key.
//! A quaternion is animated as up to three Euler tracks ([`AnimatedLeaf::QuatEuler`], degrees,
//! ZXY order like the inspector) that are combined back into the quaternion.
//!
//! ```
//! use aphyn_animation::*;
//! use aphyn_any::*;
//! use aphyn_reflection::{ReflectPath, ReflectionRegistryBuilder};
//!
//! struct Door;
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let anim = AnimationTypes::new(&types);
//! let f32_id = types.type_id_of::<f32>().any_type_id();
//!
//! let path = AnimationPath {
//!     entity: "".into(),
//!     component: types.type_id_of::<Door>().any_type_id(),
//!     field: ReflectPath::parse("/angle").unwrap(),
//!     leaf: AnimatedLeaf::Direct,
//! };
//!
//! let mut clip = AnimationClip::new();
//! clip.set_key(&anim, path.clone(), f32_id, 0.0, AnyBox::new(0.0_f32, &types));
//! let (track, key) = clip.set_key(&anim, path, f32_id, 2.0, AnyBox::new(90.0_f32, &types));
//! clip.tracks[track as u64].keys[0].interpolation = KeyInterpolation::Linear;
//! assert_eq!(clip.duration(), 2.0);
//! assert_eq!(key, 1);
//!
//! let registry = ReflectionRegistryBuilder::new().build();
//! let sampled = sample_track(&registry, &anim, &clip.tracks[0], 0.5).unwrap();
//! assert_eq!(sampled.downcast_ref(types.type_id_of::<f32>()), Some(&22.5));
//!
//! // before the first and after the last key the end values hold
//! let sampled = sample_track(&registry, &anim, &clip.tracks[0], 5.0).unwrap();
//! assert_eq!(sampled.downcast_ref(types.type_id_of::<f32>()), Some(&90.0));
//! ```
//!
//! [`AnimationClip::set_key`] creates the track on demand and replaces a key that is already at
//! that time (within [`KEY_TIME_EPSILON`]). New keys use `Hermite` with automatic tangents
//! (Catmull-Rom slopes, flat at the ends), recomputed whenever the track's keys change. Integers
//! interpolate too:
//!
//! ```
//! use aphyn_animation::*;
//! use aphyn_any::*;
//! use aphyn_reflection::ReflectPath;
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let anim = AnimationTypes::new(&types);
//! let i32_id = types.type_id_of::<i32>().any_type_id();
//! let path = AnimationPath { entity: "".into(), component: AnyTypeId::UNRESOLVED, field: ReflectPath::new(), leaf: AnimatedLeaf::Direct };
//! let mut clip = AnimationClip::new();
//! clip.set_key(&anim, path.clone(), i32_id, 0.0, AnyBox::new(0_i32, &types));
//! clip.set_key(&anim, path, i32_id, 1.0, AnyBox::new(3_i32, &types));
//! clip.tracks[0].keys[0].interpolation = KeyInterpolation::Linear;
//!
//! assert_eq!(sample_numeric(&anim, &clip.tracks[0].keys, 0.5), Some(1.5));
//! let rounded = anim.numeric_from_f64(i32_id, 1.5).unwrap();
//! assert_eq!(rounded.downcast_ref(types.type_id_of::<i32>()), Some(&2));
//! ```
//!
//! #### Playing a clip
//!
//! Put an [`AnimationComponent`] on the root entity. Each `Update`, [`animation_system`]
//! advances `time` by `delta_time * speed` while `playing`, wraps it per [`AnimationWrapMode`]
//! (`Once` stops at the end, `Loop`, `PingPong`), and writes the sampled values into the targets:
//!
//! ```ignore
//! let clip: AssetHandle<AnimationClip> = clips.get_registered("anims/door.asset").cloned().unwrap();
//! entities.add_component(door, AnimationComponent::new(clip)).ok();
//! ```
//!
//! [`wrap_time`] maps an unbounded time to the time sampled in a clip:
//!
//! ```
//! use aphyn_animation::*;
//!
//! assert_eq!(wrap_time(AnimationWrapMode::Loop, 2.5, 2.0), 0.5);
//! assert_eq!(wrap_time(AnimationWrapMode::PingPong, 2.5, 2.0), 1.5);
//! assert_eq!(wrap_time(AnimationWrapMode::Once, 2.5, 2.0), 2.0);
//! ```
//!
//! Driving a root by hand (the editor's preview does) goes through [`animate_entity`], with a
//! closure that sets the component's time instead of advancing it. The lower-level
//! [`bind_clip`] and [`apply_bindings`] resolve and write a clip without a component:
//!
//! ```
//! use aphyn_animation::*;
//! use aphyn_asset_storage::AssetHandle;
//! use aphyn_any::AnyBox;
//! use aphyn_ecs::{Component, WorldData};
//! use aphyn_reflection::*;
//! use aphyn_transform::{NameComponent, set_parent};
//!
//! #[derive(Component, Reflectable, Default)]
//! struct Door {
//!     angle: f32,
//! }
//!
//! let mut data = WorldData::default();
//! let types = data.types().clone();
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Door>(&types).register::<f32>(&types);
//! let registry = builder.build();
//! let anim = AnimationTypes::new(&types);
//!
//! let mut ent = data.entities_mut();
//! let root = ent.create_entity();
//! let door = ent.create_entity();
//! ent.add_component(door, NameComponent("Door".into())).ok().unwrap();
//! ent.add_component(door, Door::default()).ok().unwrap();
//! set_parent(ent.as_mut(), door, root);
//!
//! let path = AnimationPath {
//!     entity: "Door".into(),
//!     component: types.type_id_of::<Door>().any_type_id(),
//!     field: ReflectPath::parse("/angle").unwrap(),
//!     leaf: AnimatedLeaf::Direct,
//! };
//! let mut clip = AnimationClip::new();
//! clip.set_key(&anim, path.clone(), types.type_id_of::<f32>().any_type_id(), 0.0, AnyBox::new(0.0_f32, &types));
//! clip.set_key(&anim, path, types.type_id_of::<f32>().any_type_id(), 1.0, AnyBox::new(90.0_f32, &types));
//!
//! let bindings = bind_clip(ent.as_ref(), &registry, &anim, root, &AssetHandle::EMPTY, &clip);
//! assert!(apply_bindings(ent.as_mut(), &registry, &anim, &clip, &bindings, 1.0));
//! assert_eq!(ent.get_component::<Door>(door).unwrap().angle, 90.0);
//! assert_eq!(entity_path_from(ent.as_ref(), root, door).unwrap().as_str(), "Door");
//! ```
//!
//! #### Recording
//!
//! [`collect_leaves`] lists the animatable leaves at or below a path in a component with clones
//! of their current values, ready for [`AnimationClip::set_key`]; [`entity_path_from`] gives the
//! entity part of the path. [`unwrap_degrees_near`] keeps a recorded Euler angle close to the
//! previous key so rotations don't flip by 360°.
//!
//! # How to maintain
//!
//! #### Storage vs runtime values
//!
//! On disk a clip is [`AnimationClipAssetMetadata`]: the field path as text
//! ([`ReflectPath::parse`](aphyn_reflection::ReflectPath::parse) / `Display`) and each key's
//! value as a `SerializedValue`. [`AnimationClip::from_meta`] resolves the track's `component` and `value_type` serialized
//! names to `AnyTypeId`s through the serializer registry (a track naming an unknown type is dropped) and
//! deserializes every key into an `AnyBox` of the value type through the transitive serializer registry, so the value
//! type needs a registered serializer; [`AnimationClip::to_meta`] is the inverse and is what the
//! asset save serializer writes. Sampling and applying never touch serialization.
//!
//! #### Binding
//!
//! [`bind_clip`] resolves each track once: the entity by names, the component by its id
//! (`get_component_any`), and the field into a chain of [`BindStep`]s (a `ReflectField` per
//! struct or enum field, the list or map reflector plus an index otherwise). The value at the end
//! must have the track's type (or be a `Quat<f32>`/`Quat<f64>` for Euler tracks, which are grouped
//! per quaternion so it is rebuilt once). Tracks that don't resolve are skipped with one log line.
//! [`AnimationBindings`] remember the clip handle and its [`AnimationClip::revision`], which every
//! editing method bumps; the bindings are rebuilt when either changes or when
//! [`apply_bindings`] finds a target gone. An enum field bound through its active variant stops
//! applying if the variant changes until the next rebind; map indices are positions in the map's
//! key order, as in `ReflectPath`.
//!
//! #### Component data
//!
//! The bindings live inside [`AnimationComponent`] but are runtime state, so its `Serializable`
//! and `Reflectable` impls are written by hand and expose only `clip`, `time`, `speed`, `playing`
//! and `wrap`. [`animate_entity`] takes the bindings out of the component while it writes other
//! components, so the root's own component can be animated too.
//!
//! #### System group
//!
//! [`animation_system`] runs in [`SYSTEM_GROUP_ANIMATION`] on `Update`; the default modules order
//! it before transform so animated transforms propagate in the same frame.

mod binding;
mod clip;
mod component;
mod meta;
mod record;
mod sampling;

pub use self::{binding::*, clip::*, component::*, meta::*, record::*, sampling::*};

use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::{Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_render_features::RenderFeatureRegistry;

pub const SYSTEM_GROUP_ANIMATION: &str = "aphyn_animation";

pub fn add_animation_module_to(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(AssetStorageResource::<AnimationClip>::new());
    let types = AnimationTypes::new(data.types());
    data.resources_mut().insert(types);

    behavior
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP_ANIMATION)
        .insert_child_system(animation_system);
}
