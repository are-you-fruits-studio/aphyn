use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::{Component, EntitiesHolderMut, EntityId, Res};
use aphyn_reflection::*;
use aphyn_reflection_ecs::ReflectionRegistryResource;
use aphyn_serialization::*;
use aphyn_time::TimeResource;

use crate::{AnimationBindings, AnimationClip, AnimationTypes, apply_bindings, bind_clip};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serializable, Reflectable)]
pub enum AnimationWrapMode {
    Once,
    #[default]
    Loop,
    PingPong,
}

#[repr(C)]
#[derive(Component)]
pub struct AnimationComponent {
    pub clip: AssetHandle<AnimationClip>,
    pub time: f32,
    pub speed: f32,
    pub playing: bool,
    pub wrap: AnimationWrapMode,
    bindings: AnimationBindings,
}

impl Default for AnimationComponent {
    fn default() -> Self {
        Self {
            clip: AssetHandle::EMPTY,
            time: 0.0,
            speed: 1.0,
            playing: true,
            wrap: AnimationWrapMode::Loop,
            bindings: AnimationBindings::default(),
        }
    }
}

impl AnimationComponent {
    pub fn new(clip: AssetHandle<AnimationClip>) -> Self {
        Self {
            clip,
            ..Default::default()
        }
    }

    pub fn bindings(&self) -> &AnimationBindings {
        &self.bindings
    }

    pub fn sample_time(&self, duration: f32) -> f32 {
        wrap_time(self.wrap, self.time, duration)
    }

    // `Once` stops at either end; `Loop` and `PingPong` keep `time` within one cycle
    pub fn advance(&mut self, delta_time: f32, duration: f32) {
        if !self.playing {
            return;
        }

        self.time += delta_time * self.speed;

        if duration <= 0.0 {
            self.time = 0.0;
            return;
        }

        match self.wrap {
            AnimationWrapMode::Once => {
                if self.time >= duration || self.time <= 0.0 {
                    self.playing = false;
                }
                self.time = self.time.clamp(0.0, duration);
            }
            AnimationWrapMode::Loop => self.time = self.time.rem_euclid(duration),
            AnimationWrapMode::PingPong => self.time = self.time.rem_euclid(2.0 * duration),
        }
    }
}

pub fn wrap_time(wrap: AnimationWrapMode, time: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return 0.0;
    }

    match wrap {
        AnimationWrapMode::Once => time.clamp(0.0, duration),
        AnimationWrapMode::Loop => time.rem_euclid(duration),
        AnimationWrapMode::PingPong => {
            let t = time.rem_euclid(2.0 * duration);
            if t > duration { 2.0 * duration - t } else { t }
        }
    }
}

// bindings are runtime state: they are neither serialized nor reflected
impl<S> Serializable<S> for AnimationComponent
where
    S: Copy
        + SerializerCtxState<AssetHandle<AnimationClip>>
        + SerializerCtxState<f32>
        + SerializerCtxState<bool>
        + SerializerCtxState<AnimationWrapMode>,
{
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        ctx.serialize_map(path)
            .with_field("clip", &self.clip)
            .with_field("time", &self.time)
            .with_field("speed", &self.speed)
            .with_field("playing", &self.playing)
            .with_field("wrap", &self.wrap)
            .finish_as_map(true)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, value: &SerializedValue) {
        ctx.deserialize_map(self, path, value)
            .with_field("clip", &mut self.clip)
            .with_field("time", &mut self.time)
            .with_field("speed", &mut self.speed)
            .with_field("playing", &mut self.playing)
            .with_field("wrap", &mut self.wrap);
    }
}

impl Reflectable for AnimationComponent {
    fn reflector() -> Reflector {
        StructReflectorBuilder::<Self>::new()
            .field("clip", |v| Some(&v.clip), |v| Some(&mut v.clip))
            .field("time", |v| Some(&v.time), |v| Some(&mut v.time))
            .field("speed", |v| Some(&v.speed), |v| Some(&mut v.speed))
            .field("playing", |v| Some(&v.playing), |v| Some(&mut v.playing))
            .field("wrap", |v| Some(&v.wrap), |v| Some(&mut v.wrap))
            .default_fn(Some(Self::default))
            .build()
    }
}

pub fn animation_system(
    mut ent: EntitiesHolderMut,
    time: Option<Res<TimeResource>>,
    clips: Option<Res<AssetStorageResource<AnimationClip>>>,
    registry: Option<Res<ReflectionRegistryResource>>,
    types: Option<Res<AnimationTypes>>,
) {
    let (Some(clips), Some(registry), Some(types)) = (clips, registry, types) else {
        return;
    };
    let delta_time = time.map_or(0.0, |t| t.delta_time);

    let animated = ent
        .query::<(EntityId, &AnimationComponent)>()
        .iter()
        .map(|(e, _)| e)
        .collect::<Vec<_>>();

    for entity in animated {
        animate_entity(ent.as_mut(), &registry, &types, &clips, entity, |anim, duration| {
            anim.advance(delta_time, duration)
        });
    }
}

// advances (through `step`), binds if needed and applies one root's animation; also used by the editor's preview
pub fn animate_entity(
    mut ent: EntitiesHolderMut,
    registry: &ReflectionRegistry,
    types: &AnimationTypes,
    clips: &AssetStorageResource<AnimationClip>,
    entity: EntityId,
    step: impl FnOnce(&mut AnimationComponent, f32),
) {
    let Some(anim) = ent.get_component_mut::<AnimationComponent>(entity) else {
        return;
    };
    let Some(clip) = clips.get(&anim.clip) else {
        return;
    };

    let duration = clip.duration();
    step(anim, duration);

    let sample_time = anim.sample_time(duration);
    let clip_handle = anim.clip.clone();
    let mut bindings = std::mem::take(&mut anim.bindings);

    if !bindings.is_current(&clip_handle, clip) {
        bindings = bind_clip(ent.as_ref(), registry, types, entity, &clip_handle, clip);
    }

    if !apply_bindings(ent.as_mut(), registry, types, clip, &bindings, sample_time) {
        bindings.invalidate();
    }

    if let Some(anim) = ent.get_component_mut::<AnimationComponent>(entity) {
        anim.bindings = bindings;
    }
}
