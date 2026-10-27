use std::path::Path;

use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_ecs::*;
use fruits_prefab::Scene;
use fruits_serialization::*;

use crate::{AssetLoader, deserialize_entities_no_deps, instantiate_serialized_entities, record_serialized_entities};

pub struct SceneHandleLoader<'a> {
    pub scenes: &'a mut AssetStorageResource<Scene>,
}

impl<'a> SceneHandleLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(Self {
            scenes: res.into_get_mut::<AssetStorageResource<Scene>>()?,
        })
    }
}
impl<'a, 's> AssetLoader<TransSerializerCtxState<'s>> for SceneHandleLoader<'a> {
    type Asset = Scene;
    type SelfWithAnotherLifetime<'r> = SceneHandleLoader<'r>;

    fn create_loader<'r>(res: ResourcesHolderMut<'r>) -> Option<Self::SelfWithAnotherLifetime<'r>> {
        Self::SelfWithAnotherLifetime::from_world(res)
    }

    fn get_related_asset_storage(&mut self) -> &mut AssetStorageResource<Self::Asset> {
        self.scenes
    }

    fn load_from_serialized(&mut self, ctx: SerializerCtx<TransSerializerCtxState>, value: &SerializedValue, _assets_dir_path: impl AsRef<Path>) -> Option<Self::Asset> {
        SceneLoader.load_from_serialized(ctx, value)
    }
}

//

pub struct SceneLoader;
impl SceneLoader {
    pub fn load_from_serialized(&mut self, mut ctx: SerializerCtx<TransSerializerCtxState>, value: &SerializedValue) -> Option<Scene> {
        let scene_entities = deserialize_entities_no_deps(ctx.as_mut().map_state(|_| PureSerializerCtxState), value);

        Some(Scene {
            entities: scene_entities,
        })
    }
}

//

// todo: accept an optional parameter to store the spawned entities in
pub fn instantiate_scene(res: ResourcesHolderRef, ent: EntitiesHolderMut, scene: AssetHandle<Scene>) {
    let Some(scene) = res.get::<AssetStorageResource<Scene>>().and_then(|s| s.get(&scene)) else {
        return;
    };

    instantiate_serialized_entities(res, ent, &scene.entities);
}

pub fn record_into_scene(res: ResourcesHolderRef, ent: EntitiesHolderRef) -> Option<Scene> {
    let mut entities_ordered = ent
        .query::<EntityId>()
        .iter()
        .collect::<Vec<_>>();

    entities_ordered.sort_by_key(|e| e.version_index().index + 1);

    Some(Scene {
        entities: record_serialized_entities(res, ent, &entities_ordered, true)?,
    })
}
