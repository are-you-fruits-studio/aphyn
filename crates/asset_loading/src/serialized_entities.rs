use std::collections::HashMap;

use aphyn_asset_storage::AssetStorageResource;
use aphyn_audio::AudioClip;
use aphyn_ecs::*;
use aphyn_ffi::{FfiIndexMap, FfiString};
use aphyn_prefab::{Prefab, Scene, SerializedEntities, deserialize_component, deserialize_entity_components, serialize_components};
use aphyn_render::{ComputeShader, Material, MaterialTemplate, Shader, StandardMesh, StandardTexture};
use aphyn_serialization::*;
use aphyn_serialization_ecs::SerializersResource;
use aphyn_ui::Font;

use crate::{AssetHandleCtx, AssetHandleCtxKey, EntityIdCtx, EntityIdCtxKey, save_load_with_asset_serializers_from_world};

pub fn deserialize_entities_no_deps(
    mut ctx: SerializerCtx<PureSerializerCtxState>,
    serialized: &SerializedValue,
) -> SerializedEntities {
    let mut deserialized_entities = SerializedEntities::default();

    ctx.deserialize_map_fields("", serialized)
        .with_field("entities", &mut deserialized_entities);

    deserialized_entities
}
pub fn serialize_entities_no_deps(mut ctx: SerializerCtx<PureSerializerCtxState>, value: &SerializedEntities) -> SerializedValue {
    let serialized_entities = ctx.serialize(value, "");

    ctx.serialize_map("")
        .with_field("entities", &serialized_entities)
        .finish_as_map(true)
}

//

pub fn instantiate_serialized_entities(
    res: ResourcesHolderRef,
    mut ent: EntitiesHolderMut,
    serialized_entities: &SerializedEntities,
) -> HashMap<u64, EntityId> {
    let serializers = res.get::<SerializersResource>().unwrap();

    let mut id_to_ent = HashMap::new();

    for &entity_id in serialized_entities.0.keys() {
        id_to_ent.insert(entity_id, ent.create_entity());
    }

    {
        let entity_ids = EntityIdCtx::new(EntityIdDecoder::Restricted(&id_to_ent), EntityIdEncoder::Empty);
        let entries = (
            res.get::<AssetStorageResource<StandardTexture>>()
                .map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<Material>>().map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<MaterialTemplate>>()
                .map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<StandardMesh>>()
                .map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<AudioClip>>().map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<Shader>>().map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<ComputeShader>>()
                .map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<Font>>().map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<Prefab>>().map(AssetHandleCtx::Registered),
            res.get::<AssetStorageResource<Scene>>().map(AssetHandleCtx::Registered),
        );

        let mut context = ContextData::new().with::<EntityIdCtxKey>(&entity_ids);
        insert_optional::<AssetHandleCtxKey<StandardTexture>>(&mut context, &entries.0);
        insert_optional::<AssetHandleCtxKey<Material>>(&mut context, &entries.1);
        insert_optional::<AssetHandleCtxKey<MaterialTemplate>>(&mut context, &entries.2);
        insert_optional::<AssetHandleCtxKey<StandardMesh>>(&mut context, &entries.3);
        insert_optional::<AssetHandleCtxKey<AudioClip>>(&mut context, &entries.4);
        insert_optional::<AssetHandleCtxKey<Shader>>(&mut context, &entries.5);
        insert_optional::<AssetHandleCtxKey<ComputeShader>>(&mut context, &entries.6);
        insert_optional::<AssetHandleCtxKey<Font>>(&mut context, &entries.7);
        insert_optional::<AssetHandleCtxKey<Prefab>>(&mut context, &entries.8);
        insert_optional::<AssetHandleCtxKey<Scene>>(&mut context, &entries.9);

        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        let mut serializer_ctx = serializers.to_ctx_state(&context).into_ctx(&mut err_handler);

        for (&entity_id, entity_components) in &serialized_entities.0 {
            let entity = *id_to_ent.get(&entity_id).unwrap();

            for (component_id, component_value) in &entity_components.0 {
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
    }

    id_to_ent
}

fn insert_optional<'a, K: ContextDataKey>(context: &mut ContextData<'a>, value: &'a Option<K::Value<'a>>) {
    if let Some(value) = value {
        context.insert::<K>(value);
    }
}

pub fn record_serialized_entities(
    res: ResourcesHolderRef,
    ent: EntitiesHolderRef,
    entities_ordered: &[EntityId],
    can_serialize_any_entity: bool,
) -> Option<SerializedEntities> {
    let ent_to_id;

    let ent_to_id = if can_serialize_any_entity {
        EntityIdEncoder::World(ent)
    } else {
        ent_to_id = entities_ordered
            .iter()
            .copied()
            .map(|e| (e, ent.encode_entity_as_id(e)))
            .collect::<HashMap<_, _>>();
        EntityIdEncoder::Restricted(&ent_to_id)
    };

    let entity_ids = EntityIdCtx::new(EntityIdDecoder::Empty, ent_to_id);

    save_load_with_asset_serializers_from_world(res, entity_ids, |serializer_ctx_state| {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        let mut serializer_ctx = serializer_ctx_state.into_ctx(&mut err_handler);

        let mut serialized_entities = SerializedEntities::default();

        for &entity in entities_ordered {
            let id = ent.encode_entity_as_id(entity);
            // todo: asset references
            let components = serialize_components(entity, serializer_ctx.as_mut(), ent);

            serialized_entities.0.insert(id, components.into());
        }

        serialized_entities
    })
}

// todo: not full
pub fn record_entity_components(
    res: ResourcesHolderRef,
    ent: EntitiesHolderRef,
    entity: EntityId,
    ent_to_id: EntityIdEncoder,
) -> FfiIndexMap<FfiString, SerializedValue> {
    let entity_ids = EntityIdCtx::new(EntityIdDecoder::Empty, ent_to_id);

    save_load_with_asset_serializers_from_world(res, entity_ids, |serializer_ctx_state| {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        let serializer_ctx = serializer_ctx_state.into_ctx(&mut err_handler);

        serialize_components(entity, serializer_ctx, ent)
        // todo: maybe return option?
    })
    .unwrap_or_default()
}

// todo
pub fn override_entity_components(
    res: ResourcesHolderRef,
    mut ent: EntitiesHolderMut,
    entity: EntityId,
    components: &FfiIndexMap<FfiString, SerializedValue>,
    id_to_ent: EntityIdDecoder,
) {
    let mut components_to_remove = Vec::new();
    ent.get_all_components(entity, |c| components_to_remove.push(c.type_info().short().name()));
    for component_name in components_to_remove {
        ent.remove_component_any(entity, component_name);
    }

    let serializers = res.get::<SerializersResource>().unwrap();

    let entity_ids = EntityIdCtx::new(id_to_ent, EntityIdEncoder::Empty);
    let context = ContextData::new().with::<EntityIdCtxKey>(&entity_ids);

    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    let serializer_ctx = serializers.to_ctx_state(&context).into_ctx(&mut err_handler);

    deserialize_entity_components(components, entity, serializer_ctx, ent)
}

pub fn deserialize_entity_component_at_path(
    res: ResourcesHolderRef,
    ent: EntitiesHolderMut,
    entity: EntityId,
    component_id: &str,
    path: &str,
    serialized: &SerializedValue,
) -> bool {
    // todo: optimize and don't id-cache the whole world
    let id_to_ent = ent
        .query::<EntityId>()
        .iter()
        .map(|e| (ent.encode_entity_as_id(e), e))
        .collect::<HashMap<_, _>>();

    let entity_ids = EntityIdCtx::new(EntityIdDecoder::Restricted(&id_to_ent), EntityIdEncoder::Empty);

    save_load_with_asset_serializers_from_world(res, entity_ids, |serializer_ctx_state| {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        let serializer_ctx = serializer_ctx_state.into_ctx(&mut err_handler);

        deserialize_component(ent, entity, component_id, serializer_ctx, path, serialized)
    })
    .unwrap_or(false)
}

pub enum EntityIdEncoder<'a> {
    Empty,
    World(EntitiesHolderRef<'a>),
    Restricted(&'a HashMap<EntityId, u64>),
}

impl<'a> EntityIdEncoder<'a> {
    pub fn entity_to_id(&self, entity: EntityId) -> u64 {
        match self {
            Self::Empty => 0,
            Self::World(ent) => ent.encode_entity_as_id(entity),
            Self::Restricted(map) => map.get(&entity).copied().unwrap_or(0),
        }
    }
}

pub enum EntityIdDecoder<'a> {
    Empty,
    World(EntitiesHolderRef<'a>),
    Restricted(&'a HashMap<u64, EntityId>),
}

impl<'a> EntityIdDecoder<'a> {
    pub fn entity_from_id(&self, id: u64) -> EntityId {
        match self {
            Self::Empty => EntityId::EMPTY,
            Self::World(ent) => ent.decode_entity_from_id(id),
            Self::Restricted(map) => map.get(&id).copied().unwrap_or(EntityId::EMPTY),
        }
    }
}
