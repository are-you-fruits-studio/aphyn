use aphyn_any::{AnyBox, AnyTypeId};
use aphyn_ffi::{FfiString, FfiVec};
use aphyn_reflection::*;
use aphyn_serialization::*;

use crate::AnimationTypes;

pub const KEY_TIME_EPSILON: f32 = 1e-4;

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationPath {
    pub entity: FfiString,
    pub component: AnyTypeId,
    pub field: ReflectPath,
    pub leaf: AnimatedLeaf,
}

impl AnimationPath {
    // `component_name` is how the caller names the component (its serialized name, say); the path holds only its id
    pub fn label(&self, component_name: &str) -> String {
        let component = component_name.rsplit("::").next().unwrap_or(component_name);
        let mut label = format!("{}:{component}{}", self.entity, self.field);
        if let AnimatedLeaf::QuatEuler(axis) = self.leaf {
            label += &format!("/euler_{}", ["x", "y", "z"].get(axis as usize).unwrap_or(&"?"));
        }
        label
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serializable, Reflectable)]
pub enum AnimatedLeaf {
    #[default]
    Direct,
    // the field is a quaternion animated as one Euler angle in degrees (ZXY, like the inspector)
    QuatEuler(u8),
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serializable, Reflectable)]
pub enum KeyInterpolation {
    Constant,
    Linear,
    #[default]
    Hermite,
}

#[repr(C)]
pub struct AnimationKey {
    pub time: f32,
    pub value: AnyBox,
    pub interpolation: KeyInterpolation,
    pub in_tangent: f32,
    pub out_tangent: f32,
    pub auto_tangent: bool,
}

impl AnimationKey {
    pub fn new(time: f32, value: AnyBox) -> Self {
        Self {
            time,
            value,
            interpolation: KeyInterpolation::Hermite,
            in_tangent: 0.0,
            out_tangent: 0.0,
            auto_tangent: true,
        }
    }
}

#[repr(C)]
pub struct AnimationTrack {
    pub path: AnimationPath,
    pub value_type: AnyTypeId,
    pub keys: FfiVec<AnimationKey>,
}

impl AnimationTrack {
    pub fn new(path: AnimationPath, value_type: AnyTypeId) -> Self {
        Self {
            path,
            value_type,
            keys: FfiVec::new(),
        }
    }

    pub fn is_numeric(&self, types: &AnimationTypes) -> bool {
        types.is_numeric(self.value_type)
    }

    pub fn key_index_at(&self, time: f32) -> Option<usize> {
        self.keys.iter().position(|k| (k.time - time).abs() < KEY_TIME_EPSILON)
    }

    pub fn recompute_auto_tangents(&mut self, types: &AnimationTypes) {
        let values = self
            .keys
            .iter()
            .map(|k| types.numeric_to_f64(k.value.as_any_ref()))
            .collect::<Vec<_>>();
        let times = self.keys.iter().map(|k| k.time).collect::<Vec<_>>();
        let last = times.len().saturating_sub(1);

        for (i, key) in self.keys.iter_mut().enumerate() {
            if !key.auto_tangent {
                continue;
            }

            let slope = match (i, values.get(i.wrapping_sub(1)), values.get(i + 1)) {
                (0, ..) => 0.0,
                (i, ..) if i == last => 0.0,
                (_, Some(Some(prev)), Some(Some(next))) => {
                    let dt = (times[i + 1] - times[i - 1]) as f64;
                    if dt > 0.0 { ((next - prev) / dt) as f32 } else { 0.0 }
                }
                _ => 0.0,
            };

            key.in_tangent = slope;
            key.out_tangent = slope;
        }
    }

    fn sort_keys(&mut self) {
        self.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
}

#[repr(C)]
#[derive(Default)]
pub struct AnimationClip {
    pub tracks: FfiVec<AnimationTrack>,
    revision: u64,
}

// keys hold engine data, which is Send + Sync; AnyBox itself can't promise it
unsafe impl Send for AnimationClip {}
unsafe impl Sync for AnimationClip {}

impl AnimationClip {
    pub fn new() -> Self {
        Self::default()
    }

    // bumped by every edit through this API, so bindings know to resolve the clip again
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn mark_edited(&mut self) {
        self.revision += 1;
    }

    pub fn duration(&self) -> f32 {
        self.tracks
            .iter()
            .filter_map(|t| t.keys.last())
            .map(|k| k.time)
            .fold(0.0, f32::max)
    }

    pub fn track_index(&self, path: &AnimationPath) -> Option<usize> {
        self.tracks.iter().position(|t| t.path == *path)
    }

    // creates the track if missing and replaces a key within `KEY_TIME_EPSILON` of `time`;
    // a track whose value type differs is emptied first
    pub fn set_key(
        &mut self,
        types: &AnimationTypes,
        path: AnimationPath,
        value_type: AnyTypeId,
        time: f32,
        value: AnyBox,
    ) -> (usize, usize) {
        let track_idx = match self.track_index(&path) {
            Some(idx) => idx,
            None => {
                self.tracks.push(AnimationTrack::new(path, value_type));
                self.tracks.len() as usize - 1
            }
        };

        let track = &mut self.tracks[track_idx as u64];

        if track.value_type != value_type {
            track.value_type = value_type;
            track.keys.clear();
        }

        match track.key_index_at(time) {
            Some(idx) => track.keys[idx as u64].value = value,
            None => {
                track.keys.push(AnimationKey::new(time, value));
                track.sort_keys();
            }
        }

        track.recompute_auto_tangents(types);
        self.mark_edited();

        let key_idx = self.tracks[track_idx as u64].key_index_at(time).unwrap();
        (track_idx, key_idx)
    }

    // removes the track too once its last key is gone
    pub fn remove_key(&mut self, types: &AnimationTypes, track: usize, key: usize) -> Option<AnimationKey> {
        let t = self.tracks.get_mut(track as u64)?;
        let removed = t.keys.remove(key as u64)?;

        if t.keys.is_empty() {
            self.tracks.remove(track as u64);
        } else {
            t.recompute_auto_tangents(types);
        }

        self.mark_edited();
        Some(removed)
    }

    // returns the key's new index; a key already at `time` is replaced
    pub fn move_key(&mut self, types: &AnimationTypes, track: usize, key: usize, time: f32) -> Option<usize> {
        let t = self.tracks.get_mut(track as u64)?;
        let time = time.max(0.0);

        if let Some(existing) = t.key_index_at(time)
            && existing != key
        {
            t.keys.remove(existing as u64);
            return self.move_key(types, track, if existing < key { key - 1 } else { key }, time);
        }

        t.keys.get_mut(key as u64)?.time = time;
        t.sort_keys();
        t.recompute_auto_tangents(types);
        let idx = t.key_index_at(time);

        self.mark_edited();
        idx
    }

    pub fn remove_track(&mut self, track: usize) -> Option<AnimationTrack> {
        let removed = self.tracks.remove(track as u64)?;
        self.mark_edited();
        Some(removed)
    }

    pub fn recompute_auto_tangents(&mut self, types: &AnimationTypes, track: usize) {
        if let Some(t) = self.tracks.get_mut(track as u64) {
            t.recompute_auto_tangents(types);
            self.mark_edited();
        }
    }
}
