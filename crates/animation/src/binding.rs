use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::{EntitiesHolderMut, EntitiesHolderRef, EntityId};
use aphyn_ffi::{FfiAnyMut, FfiAnyRef, FfiArc, FfiOption, FfiString, FfiVec};
use aphyn_math::{RotationOrder3D, Vec3};
use aphyn_reflection::*;
use aphyn_transform::{ChildComponent, NameComponent, ParentComponent};

use crate::{AnimatedLeaf, AnimationClip, is_quat_type, quat_from_f64, quat_to_f64, sample_numeric, sample_track};

pub const EULER_ORDER: RotationOrder3D = RotationOrder3D::Zxy;

#[repr(C)]
#[derive(Clone)]
pub enum BindStep {
    Field(FfiArc<ReflectField>),
    ListIndex(FfiArc<Reflector>, u64),
    MapIndex(FfiArc<Reflector>, u64),
}

#[repr(C)]
#[derive(Clone)]
pub struct BoundTarget {
    pub entity: EntityId,
    pub component: FfiString,
    pub chain: FfiVec<BindStep>,
}

#[repr(C)]
#[derive(Clone)]
pub struct DirectBinding {
    pub track: u64,
    pub target: BoundTarget,
}

#[repr(C)]
#[derive(Clone)]
pub struct EulerBinding {
    // track index per x / y / z axis
    pub tracks: [FfiOption<u64>; 3],
    pub field: ReflectPath,
    pub target: BoundTarget,
}

#[repr(C)]
#[derive(Clone, Default)]
pub struct AnimationBindings {
    pub clip: AssetHandle<AnimationClip>,
    pub revision: u64,
    pub is_valid: bool,
    pub direct: FfiVec<DirectBinding>,
    pub euler: FfiVec<EulerBinding>,
}

impl AnimationBindings {
    pub fn is_current(&self, clip_handle: &AssetHandle<AnimationClip>, clip: &AnimationClip) -> bool {
        self.is_valid && self.clip == *clip_handle && self.revision == clip.revision()
    }

    pub fn invalidate(&mut self) {
        self.is_valid = false;
    }
}

// `path` is `/`-joined child names below `root`; "" is the root itself
pub fn find_entity_by_path(ent: EntitiesHolderRef, root: EntityId, path: &str) -> Option<EntityId> {
    let mut current = root;

    for name in path.split('/').filter(|s| !s.is_empty()) {
        let parent = ent.get_component::<ParentComponent>(current)?;
        current = *parent
            .children()
            .iter()
            .find(|c| ent.get_component::<NameComponent>(**c).is_some_and(|n| &*n.0 == name))?;
    }

    Some(current)
}

// the inverse of `find_entity_by_path`; `None` if `entity` isn't below `root` or an ancestor is unnamed
pub fn entity_path_from(ent: EntitiesHolderRef, root: EntityId, entity: EntityId) -> Option<FfiString> {
    let mut names = Vec::new();
    let mut current = entity;

    while current != root {
        names.push(ent.get_component::<NameComponent>(current)?.0.to_string());
        current = ent.get_component::<ChildComponent>(current)?.parent();
    }

    names.reverse();
    Some(names.join("/").into())
}

// the nearest of `entity` and its ancestors that `has_root` accepts
pub fn find_root_of(ent: EntitiesHolderRef, entity: EntityId, has_root: impl Fn(EntityId) -> bool) -> Option<EntityId> {
    let mut current = entity;

    loop {
        if has_root(current) {
            return Some(current);
        }
        current = ent.get_component::<ChildComponent>(current)?.parent();
    }
}

pub fn resolve_chain<'a>(
    registry: &ReflectionRegistry,
    mut value: FfiAnyRef<'a>,
    path: &ReflectPath,
) -> Option<(FfiVec<BindStep>, FfiAnyRef<'a>)> {
    let mut chain = FfiVec::new();

    for segment in path.segments() {
        let reflector = registry.get(value.type_info().short().name())?;

        let (step, next) = match (&**reflector, segment) {
            (Reflector::Struct(r), ReflectPathSegment::Field(name)) => {
                let field = r.field(name)?.clone();
                let next = field.get(value)?;
                (BindStep::Field(field), next)
            }
            (Reflector::Enum(r), ReflectPathSegment::Field(name)) => {
                let field = r.active_variant(value)?.field(name)?.clone();
                let next = field.get(value)?;
                (BindStep::Field(field), next)
            }
            (Reflector::List(r), ReflectPathSegment::Index(idx)) => {
                (BindStep::ListIndex(reflector.clone(), *idx), r.get(value, *idx as usize)?)
            }
            (Reflector::Map(r), ReflectPathSegment::Index(idx)) => {
                let keys = r.keys(value)?;
                (BindStep::MapIndex(reflector.clone(), *idx), r.get(value, *keys.get(*idx)?)?)
            }
            _ => return None,
        };

        chain.push(step);
        value = next;
    }

    Some((chain, value))
}

pub fn walk_chain_mut<'a>(registry: &ReflectionRegistry, mut value: FfiAnyMut<'a>, chain: &[BindStep]) -> Option<FfiAnyMut<'a>> {
    for step in chain {
        value = match step {
            BindStep::Field(field) => field.get_mut(value)?,
            BindStep::ListIndex(r, idx) => r.as_list()?.get_mut(value, *idx as usize)?,
            BindStep::MapIndex(r, idx) => {
                let r = r.as_map()?;
                let key = reflect_clone(registry, *r.keys(value.as_any_ref())?.get(*idx)?)?;
                r.get_mut(value, key.as_any_ref())?
            }
        };
    }

    Some(value)
}

// tracks that don't resolve (missing entity, component or field, or a type mismatch) are skipped
pub fn bind_clip(
    ent: EntitiesHolderRef,
    registry: &ReflectionRegistry,
    root: EntityId,
    clip_handle: &AssetHandle<AnimationClip>,
    clip: &AnimationClip,
) -> AnimationBindings {
    let mut bindings = AnimationBindings {
        clip: clip_handle.clone(),
        revision: clip.revision(),
        is_valid: true,
        ..Default::default()
    };

    for (idx, track) in clip.tracks.iter().enumerate() {
        let path = &track.path;

        let resolved = find_entity_by_path(ent, root, &path.entity).and_then(|entity| {
            let component = ent.get_component_any(entity, &path.component)?;
            let (chain, value) = resolve_chain(registry, component, &path.field)?;
            let target = BoundTarget {
                entity,
                component: path.component.clone(),
                chain,
            };
            Some((target, value.type_info().short().name()))
        });

        let bound = match (resolved, path.leaf) {
            (Some((target, ty)), AnimatedLeaf::Direct) if ty == track.value_type.as_str() => {
                bindings.direct.push(DirectBinding {
                    track: idx as u64,
                    target,
                });
                true
            }
            (Some((target, ty)), AnimatedLeaf::QuatEuler(axis)) if is_quat_type(ty) && axis < 3 => {
                let group = bindings
                    .euler
                    .iter_mut()
                    .find(|g| g.target.entity == target.entity && g.target.component == target.component && g.field == path.field);

                match group {
                    Some(group) => group.tracks[axis as usize] = FfiOption::Some(idx as u64),
                    None => {
                        let mut tracks = [FfiOption::None, FfiOption::None, FfiOption::None];
                        tracks[axis as usize] = FfiOption::Some(idx as u64);
                        bindings.euler.push(EulerBinding {
                            tracks,
                            field: path.field.clone(),
                            target,
                        });
                    }
                }
                true
            }
            _ => false,
        };

        if !bound {
            eprintln!("animation: track `{path}` could not be bound");
        }
    }

    bindings
}

// returns false when a bound target is gone, so the caller binds again
pub fn apply_bindings(
    mut ent: EntitiesHolderMut,
    registry: &ReflectionRegistry,
    clip: &AnimationClip,
    bindings: &AnimationBindings,
    time: f32,
) -> bool {
    let mut is_valid = true;

    for binding in bindings.direct.iter() {
        let Some(track) = clip.tracks.get(binding.track) else {
            continue;
        };
        let Some(sampled) = sample_track(registry, track, time) else {
            continue;
        };
        let target = &binding.target;

        let Some(component) = ent.get_component_any_mut(target.entity, &target.component) else {
            is_valid = false;
            continue;
        };
        match walk_chain_mut(registry, component, &target.chain) {
            Some(mut value) => _ = value.replace(sampled),
            None => is_valid = false,
        }
    }

    for binding in bindings.euler.iter() {
        let target = &binding.target;

        let Some(component) = ent.get_component_any_mut(target.entity, &target.component) else {
            is_valid = false;
            continue;
        };
        let Some(mut value) = walk_chain_mut(registry, component, &target.chain) else {
            is_valid = false;
            continue;
        };
        let Some(quat) = quat_to_f64(value.as_any_ref()) else {
            continue;
        };

        let mut angles = quat.to_euler_angles(EULER_ORDER).into_array();
        for (axis, track) in binding.tracks.iter().enumerate() {
            let sampled = track
                .as_ref()
                .and_then(|idx| clip.tracks.get(*idx))
                .and_then(|t| sample_numeric(&t.keys, time));
            if let Some(degrees) = sampled {
                angles[axis] = degrees.to_radians();
            }
        }

        let quat = aphyn_math::Quat::<f64>::from_euler_angles(Vec3::from_array(angles), EULER_ORDER);
        if let Some(new) = quat_from_f64(value.type_info().short().name(), quat) {
            _ = value.replace(new);
        }
    }

    is_valid
}
