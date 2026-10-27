use std::{collections::{HashSet, VecDeque}, path::Path};

use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_ecs::*;
use fruits_prefab::Prefab;
use fruits_serialization::*;
use fruits_transform::ParentComponent;

use crate::{AssetLoader, deserialize_entities_no_deps, instantiate_serialized_entities, record_serialized_entities};

pub struct PrefabHandleLoader<'a> {
    pub prefabs: &'a mut AssetStorageResource<Prefab>,
}

impl<'a> PrefabHandleLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(Self {
            prefabs: res.into_get_mut::<AssetStorageResource<Prefab>>()?,
        })
    }
}
impl<'a, 's> AssetLoader<TransSerializerCtxState<'s>> for PrefabHandleLoader<'a> {
    type Asset = Prefab;
    type SelfWithAnotherLifetime<'r> = PrefabHandleLoader<'r>;

    fn create_loader<'r>(res: ResourcesHolderMut<'r>) -> Option<Self::SelfWithAnotherLifetime<'r>> {
        Self::SelfWithAnotherLifetime::from_world(res)
    }

    fn get_related_asset_storage(&mut self) -> &mut AssetStorageResource<Self::Asset> {
        self.prefabs
    }

    fn load_from_serialized(&mut self, ctx: SerializerCtx<TransSerializerCtxState>, value: &SerializedValue, _assets_dir_path: impl AsRef<Path>) -> Option<Self::Asset> {
        PrefabLoader.load_from_serialized(ctx, value)
    }
}

//

pub struct PrefabLoader;
impl PrefabLoader {
    pub fn load_from_serialized(&mut self, mut ctx: SerializerCtx<TransSerializerCtxState>, value: &SerializedValue) -> Option<Prefab> {
        let prefab_entities = deserialize_entities_no_deps(ctx.as_mut().map_state(|_| PureSerializerCtxState), value);

        let prefab = Prefab {
            entities: prefab_entities,
        };

        Some(prefab)
    }
}

//

pub fn instantiate_prefab(res: ResourcesHolderRef, ent: EntitiesHolderMut, prefab: AssetHandle<Prefab>) -> Option<EntityId> {
    let prefab = res.get::<AssetStorageResource<Prefab>>()?.get(&prefab)?;

    let root_entity_id = *prefab.entities.0.keys().next()?;

    instantiate_serialized_entities(res, ent, &prefab.entities).get(&root_entity_id).copied()
}

pub fn record_into_prefab(
    res: ResourcesHolderRef,
    ent: EntitiesHolderRef,
    entity: EntityId,
) -> Option<Prefab> {
    if !ent.contains_entity(entity) {
        return None;
    }

    let mut ent_serialize_queue = VecDeque::new();
    ent_serialize_queue.push_back(entity);

    let mut ent_to_id = HashSet::new();
    let mut entities_ordered = Vec::new();

    while let Some(entity) = ent_serialize_queue.pop_front() {
        if !ent_to_id.insert(entity) {
            continue;
        }

        entities_ordered.push(entity);

        if let Some(parent_c) = ent.get_component::<ParentComponent>(entity) {
            ent_serialize_queue.extend(parent_c.children.iter().copied());
        }
    }

    Some(Prefab {
        entities: record_serialized_entities(res, ent, &entities_ordered, false)?,
    })
}

// todo: prefabs instantiation needs access to fonts

const _EXAMPLE: &str = r#"
[
  "0": {
    "GlobalTransform": {

    }
  }
]
"#;
