use std::{collections::{HashMap, VecDeque}, path::Path};

use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_ecs::*;
use fruits_ffi::{FfiIndexMap, FfiString, FfiVec};
use fruits_prefab::{Prefab, PrefabDependencies, PrefabEntities, deserialize_component, deserialize_prefab_components, serialize_components};
use fruits_serialization::*;
use fruits_transform::ParentComponent;

use crate::AssetLoader;

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
        let prefab_entities = deserialize_prefab_no_deps(ctx.as_mut().map_state(|_| PureSerializerCtxState), value);
        
        let prefab = Prefab {
            dependencies: load_prefab_dependencies(ctx, &prefab_entities),
            entities: prefab_entities,
        };

        Some(prefab)
    }
}

pub fn deserialize_prefab_no_deps(mut ctx: SerializerCtx<PureSerializerCtxState>, serialized: &SerializedValue) -> PrefabEntities {
    let mut deserialized_entities = PrefabEntities::default();

    ctx.deserialize_map_fields("", serialized)
        .with_field("entities", &mut deserialized_entities);

    deserialized_entities
}
pub fn serialize_prefab_no_deps(mut ctx: SerializerCtx<PureSerializerCtxState>, value: &PrefabEntities) -> SerializedValue {
    let serialized_entities = ctx.serialize(value, "");

    ctx.serialize_map("")
        .with_field("entities", &serialized_entities)
        .finish_as_map(true)
}



//

#[derive(Default)]
struct PrefabComponentSpawnCtx {
    root_entity_id: u64,
    entities: HashMap<u64, EntityId>,
}

fn load_prefab_dependencies(mut ctx: SerializerCtx<TransSerializerCtxState>, prefab: &PrefabEntities) -> PrefabDependencies {
    // todo
    let mut deps = PrefabDependencies::default();

    for prefab_components in prefab.0.values() {
        for (component_id, component_value) in &prefab_components.0 {
            _ = ctx.as_mut().deserialize_default_any(component_id, "", component_value);
        }
    }

    deps
}

pub fn instantiate_prefab(res: ResourcesHolderRef, mut ent: EntitiesHolderMut, prefab: AssetHandle<Prefab>) -> Option<EntityId> {
    let prefabs = res.get::<AssetStorageResource<Prefab>>()?;
    let serializers_global = res.get::<SerializersResource>().unwrap();

    let prefab = prefabs.get(&prefab)?;

    let mut ctx = PrefabComponentSpawnCtx::default();
    let mut is_root_init = false;

    for &entity_id in prefab.entities.0.keys() {
        let entity = ent.create_entity();

        ctx.entities.insert(entity_id, entity);

        if !std::mem::replace(&mut is_root_init, true) {
            ctx.root_entity_id = entity_id;
        }
    }

    let entities_serialized = HashMap::new();

    let deps = &prefab.dependencies;

    let mut serializers_local = TransSerializerRegistry::new();

    serializers_local.register(EntityTransSerializer::new(&ctx.entities, &entities_serialized));
    serializers_local.register(PrefabAssetInstantiateTransSerializer::new(deps, |deps, key| deps.textures.get(key).cloned()));
    serializers_local.register(PrefabAssetInstantiateTransSerializer::new(deps, |deps, key| deps.materials.get(key).cloned()));
    serializers_local.register(PrefabAssetInstantiateTransSerializer::new(deps, |deps, key| deps.meshes.get(key).cloned()));
    serializers_local.register(PrefabAssetInstantiateTransSerializer::new(deps, |deps, key| deps.audio_clips.get(key).cloned()));
    serializers_local.register(PrefabAssetInstantiateTransSerializer::new(deps, |deps, key| deps.fonts.get(key).cloned()));
    serializers_local.register(PrefabAssetInstantiateTransSerializer::new(deps, |deps, key| deps.prefabs.get(key).cloned()));

    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    let serializer_ctx_state = serializers_global.to_ctx_state();
    let mut serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);

    for (&entity_id, prefab_components) in &prefab.entities.0 {
        let entity = *ctx.entities.get(&entity_id).unwrap();

        for (component_id, component_value) in &prefab_components.0 {
            let did_deserialize_component = deserialize_component(
                ent.as_mut(),
                entity,
                component_id,
                serializer_ctx.as_mut(),
                "",
                component_value,
            );

            if !did_deserialize_component {
                println!("failed to deserialize component: {}", component_id);
            }
        }
    }

    ctx.entities.get(&ctx.root_entity_id).copied()
}

// todo: not full
pub fn record_into_prefab_components(
    res: ResourcesHolderRef,
    ent: EntitiesHolderRef,
    entity: EntityId,
    ent_to_id: &HashMap<EntityId, u64>,
) -> FfiIndexMap<FfiString, SerializedValue> {
    let serializers_global = res.get::<SerializersResource>().unwrap();

    let entities_deserialized = HashMap::<u64, EntityId>::new();

    let mut serializers_local = TransSerializerRegistry::new();

    serializers_local.register(EntityTransSerializer {
        entities_deserialized: &entities_deserialized,
        entities_serialized: ent_to_id,
    });

    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    
    let serializer_ctx_state = serializers_global.to_ctx_state();
    let serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);

    serialize_components(entity, serializer_ctx, ent)
}

pub fn record_into_prefab(
    res: ResourcesHolderRef,
    ent: EntitiesHolderRef,
    entity: EntityId,
) -> Option<Prefab> {
    if !ent.contains_entity(entity) {
        return None;
    }

    let serializers_global = res.get::<SerializersResource>().unwrap();

    let entities_deserialized = HashMap::<u64, EntityId>::new();

    let mut ent_serialize_queue = VecDeque::new();
    ent_serialize_queue.push_back(entity);
    
    let mut ent_to_id = HashMap::new();
    let mut entities_ordered = Vec::new();

    while let Some(entity) = ent_serialize_queue.pop_front() {
        let id = entity.version_index().index + 1;
        if ent_to_id.insert(entity, id).is_some() {
            continue;
        }

        entities_ordered.push((entity, id));

        if let Some(parent_c) = ent.get_component::<ParentComponent>(entity) {
            ent_serialize_queue.extend(parent_c.children.iter().copied());
        }
    }

    let mut serializers_local = TransSerializerRegistry::new();
    serializers_local.register(EntityTransSerializer {
        entities_deserialized: &entities_deserialized,
        entities_serialized: &ent_to_id,
    });

    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    
    let serializer_ctx_state = serializers_global.to_ctx_state();
    let mut serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);

    let mut prefab = Prefab::empty();

    for (entity, id) in entities_ordered {
        // todo: asset references
        let components = serialize_components(entity, serializer_ctx.as_mut(), ent);

        prefab.entities.0.insert(id, components.into());
    }

    Some(prefab)
}

// todo
pub fn override_entity_components_from_prefab(
    res: ResourcesHolderRef,
    mut ent: EntitiesHolderMut,
    entity: EntityId,
    components: &FfiIndexMap<FfiString, SerializedValue>,
    id_to_ent: &HashMap<u64, EntityId>,
) {
    let mut components_to_remove = Vec::new();
    ent.get_all_components(entity, |c| components_to_remove.push(c.type_info().short().name()));
    for component_name in components_to_remove {
        ent.remove_component_any(entity, component_name);
    }

    let serializers_global = res.get::<SerializersResource>().unwrap();

    let entities_serialized = HashMap::<EntityId, u64>::new();

    let mut serializers_local = TransSerializerRegistry::new();

    serializers_local.register(EntityTransSerializer {
        entities_deserialized: id_to_ent,
        entities_serialized: &entities_serialized,
    });

    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    
    let serializer_ctx_state = serializers_global.to_ctx_state();
    let serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);

    deserialize_prefab_components(components, entity, serializer_ctx, ent)
}

pub fn deserialize_entity_component_from_prefab(
    res: ResourcesHolderRef,
    ent: EntitiesHolderMut,
    entity: EntityId,
    component_id: &str,
    path: &str,
    serialized: &SerializedValue,
    id_to_ent: &HashMap<u64, EntityId>,
) -> bool {
    let serializers_global = res.get::<SerializersResource>().unwrap();

    let entities_serialized = HashMap::<EntityId, u64>::new();

    let mut serializers_local = TransSerializerRegistry::new();

    serializers_local.register(EntityTransSerializer {
        entities_deserialized: id_to_ent,
        entities_serialized: &entities_serialized,
    });

    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    
    let serializer_ctx_state = serializers_global.to_ctx_state();
    let serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);

    deserialize_component(ent, entity, component_id, serializer_ctx, path, serialized)
}

pub struct EntityTransSerializer<'brw> {
    entities_deserialized: &'brw HashMap<u64, EntityId>,
    entities_serialized: &'brw HashMap<EntityId, u64>,
}

impl<'brw> EntityTransSerializer<'brw> {
    pub fn new(
        entities_deserialized: &'brw HashMap<u64, EntityId>,
        entities_serialized: &'brw HashMap<EntityId, u64>,
    ) -> Self {
        Self {
            entities_deserialized,
            entities_serialized,
        }
    }
}

impl<'brw, S: Copy> Serializer<S> for EntityTransSerializer<'brw> {
    type Deserialized = EntityId;

    fn serialize(&self, value: &Self::Deserialized, _ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        if !normalize_serialization_path(path).is_empty() {
            return SerializedValue::Null;
        }

        SerializedValue::Primitive(SerializedPrimitive::Int(self.entities_serialized.get(value).copied().unwrap_or(0) as i128))
    }

    fn deserialize(&self, value: &mut Self::Deserialized, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }

        let result = match serialized {
            SerializedValue::Primitive(SerializedPrimitive::Int(number)) => {
                match *number {
                    0 => Some(EntityId::EMPTY),
                    number => self.entities_deserialized.get(&(number as u64)).copied(),
                }
            },
            _ => None,
        };

        if result.is_none() {
            ctx.report_err(SerializationError::InvalidInput { message: "failed to parse entity".into() });
        }

        *value = result.unwrap_or_else(|| EntityId::EMPTY)
    }
    
    fn serializable_default(&self) -> Self::Deserialized {
        EntityId::EMPTY
    }
}

pub struct PrefabAssetInstantiateTransSerializer<'a, T: 'static> {
    deps: &'a PrefabDependencies,
    extractor: fn(&PrefabDependencies, &str) -> Option<AssetHandle<T>>,
}
impl<'a, T: 'static> PrefabAssetInstantiateTransSerializer<'a, T> {
    pub fn new(deps: &'a PrefabDependencies, extractor: fn(&PrefabDependencies, &str) -> Option<AssetHandle<T>>) -> Self {
        Self {
            deps,
            extractor,
        }
    }
}
impl<'a, T, S: Copy> Serializer<S> for PrefabAssetInstantiateTransSerializer<'a, T> {
    type Deserialized = AssetHandle<T>;

    fn serialize(&self, _value: &Self::Deserialized, mut _ctx: SerializerCtx<S>, _path: &str) -> SerializedValue {
        unimplemented!("PrefabAssetInstantiateTransSerializer is for instantiation only");
    }

    fn deserialize(&self, value: &mut Self::Deserialized, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }
        
        *value = Default::default();

        let SerializedValue::Primitive(SerializedPrimitive::String(serialized)) = serialized else {
            ctx.report_err(SerializationError::InvalidInput { message: "AssetHandle can only be deserialized from string".into() });
            return;
        };

        let Some(asset_handle) = (self.extractor)(self.deps, serialized.as_str()) else {
            ctx.report_err(SerializationError::InvalidInput { message: format!("AssetHandle {serialized} cannot be loaded").into() });
            return;
        };

        *value = asset_handle.clone();
    }
    
    fn serializable_default(&self) -> Self::Deserialized {
        Default::default()
    }
}

// todo: prefabs instantiation needs to:
// + spawn entities
// + spawn components on the entities
// + access to other entities inside the prefab
// + access to materials
// + access to meshes
// + access to textures
// - access to fonts
// - access to other prefabs
// - access to other assets

const _EXAMPLE: &str = r#"
[
  {
    "entity_id": 0,
    "components": [
      {
        "component_id": GlobalTransform,
        "data": {
         
        }
      }
    ]
  }
]
"#;
