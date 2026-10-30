use std::collections::HashMap;

use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_audio::AudioClip;
use aphyn_ecs::*;
use aphyn_ffi::{FfiIndexMap, FfiString};
use aphyn_prefab::{Prefab, Scene, SerializedEntities, deserialize_component, deserialize_entity_components, serialize_components};
use aphyn_render::{StandardMaterial, StandardMesh, StandardShader, StandardTexture};
use aphyn_serialization::*;
use aphyn_ui::Font;

use crate::save_load_with_asset_serializers_from_world;

pub fn deserialize_entities_no_deps(mut ctx: SerializerCtx<PureSerializerCtxState>, serialized: &SerializedValue) -> SerializedEntities {
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
    let serializers_global = res.get::<SerializersResource>().unwrap();

    let textures = res.get::<AssetStorageResource<StandardTexture>>();
    let materials = res.get::<AssetStorageResource<StandardMaterial>>();
    let meshes = res.get::<AssetStorageResource<StandardMesh>>();
    let audio_clips = res.get::<AssetStorageResource<AudioClip>>();
    let shaders = res.get::<AssetStorageResource<StandardShader>>();
    let fonts = res.get::<AssetStorageResource<Font>>();
    let prefabs = res.get::<AssetStorageResource<Prefab>>();
    let scenes = res.get::<AssetStorageResource<Scene>>();

    let mut id_to_ent = HashMap::new();

    for &entity_id in serialized_entities.0.keys() {
        id_to_ent.insert(entity_id, ent.create_entity());
    }

    {
        let mut serializers_local = TransSerializerRegistry::new();

        serializers_local.register(EntityTransSerializer::new(EntityIdDecoder::Restricted(&id_to_ent), EntityIdEncoder::Empty));
        serializers_local.register(AssetInstantiateTransSerializer::new(textures));
        serializers_local.register(AssetInstantiateTransSerializer::new(materials));
        serializers_local.register(AssetInstantiateTransSerializer::new(meshes));
        serializers_local.register(AssetInstantiateTransSerializer::new(audio_clips));
        serializers_local.register(AssetInstantiateTransSerializer::new(shaders));
        serializers_local.register(AssetInstantiateTransSerializer::new(fonts));
        serializers_local.register(AssetInstantiateTransSerializer::new(prefabs));
        serializers_local.register(AssetInstantiateTransSerializer::new(scenes));

        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        let serializer_ctx_state = serializers_global.to_ctx_state();
        let mut serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);

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

pub fn record_serialized_entities(
    res: ResourcesHolderRef,
    ent: EntitiesHolderRef,
    entities_ordered: &[EntityId],
    can_serialize_any_entity: bool,
) -> Option<SerializedEntities> {
    let serializers_global = res.get::<SerializersResource>().unwrap();

    let ent_to_id;

    let ent_to_id = if can_serialize_any_entity {
        EntityIdEncoder::World(ent)
    } else {
        ent_to_id = entities_ordered.iter().copied().map(|e| (e, ent.encode_entity_as_id(e))).collect::<HashMap<_, _>>();
        EntityIdEncoder::Restricted(&ent_to_id)
    };

    let mut serializers_sublocal = TransSerializerRegistry::new();
    serializers_sublocal.register(EntityTransSerializer {
        entities_deserialized: EntityIdDecoder::Empty,
        entities_serialized: ent_to_id,
    });

    save_load_with_asset_serializers_from_world(res, |serializers_local| {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());

        let serializer_ctx_state = serializers_global.to_ctx_state();
        let serializer_ctx_state = serializer_ctx_state.wrap_with_local(&serializers_local);
        let mut serializer_ctx = serializer_ctx_state
            .wrap_with_local(&serializers_sublocal)
            .into_ctx(&mut err_handler);

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
    let serializers_global = res.get::<SerializersResource>().unwrap();

    let mut serializers_sublocal = TransSerializerRegistry::new();

    serializers_sublocal.register(EntityTransSerializer {
        entities_deserialized: EntityIdDecoder::Empty,
        entities_serialized: ent_to_id,
    });

    save_load_with_asset_serializers_from_world(res, |serializers_local| {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());

        let serializer_ctx_state = serializers_global.to_ctx_state();
        let serializer_ctx_state = serializer_ctx_state.wrap_with_local(&serializers_local);
        let serializer_ctx = serializer_ctx_state
            .wrap_with_local(&serializers_sublocal)
            .into_ctx(&mut err_handler);

        serialize_components(entity, serializer_ctx, ent)
        // todo: maybe return option?
    }).unwrap_or_default()
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

    let serializers_global = res.get::<SerializersResource>().unwrap();

    let mut serializers_local = TransSerializerRegistry::new();

    serializers_local.register(EntityTransSerializer {
        entities_deserialized: id_to_ent,
        entities_serialized: EntityIdEncoder::Empty,
    });

    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());

    let serializer_ctx_state = serializers_global.to_ctx_state();
    let serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);

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
    let serializers_global = res.get::<SerializersResource>().unwrap();
    
    // todo: optimize and don't id-cache the whole world
    let id_to_ent = ent.query::<EntityId>().iter().map(|e| (ent.encode_entity_as_id(e), e)).collect::<HashMap<_, _>>();

    let mut serializers_sublocal = TransSerializerRegistry::new();
    
    serializers_sublocal.register(EntityTransSerializer {
        entities_deserialized: EntityIdDecoder::Restricted(&id_to_ent),
        entities_serialized: EntityIdEncoder::Empty,
    });

    save_load_with_asset_serializers_from_world(res, |serializers_local| {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());

        let serializer_ctx_state = serializers_global.to_ctx_state();
        let serializer_ctx_state = serializer_ctx_state.wrap_with_local(&serializers_local);
        let serializer_ctx = serializer_ctx_state
            .wrap_with_local(&serializers_sublocal)
            .into_ctx(&mut err_handler);

        deserialize_component(ent, entity, component_id, serializer_ctx, path, serialized)
    }).unwrap_or(false)
}

pub enum EntityIdEncoder<'a> {
    Empty,
    World(EntitiesHolderRef<'a>),
    Restricted(&'a HashMap<EntityId, u64>)
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
    Restricted(&'a HashMap<u64, EntityId>)
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

pub struct EntityTransSerializer<'brw> {
    entities_deserialized: EntityIdDecoder<'brw>,
    entities_serialized: EntityIdEncoder<'brw>,
}

impl<'brw> EntityTransSerializer<'brw> {
    pub fn new(
        entities_deserialized: EntityIdDecoder<'brw>,
        entities_serialized: EntityIdEncoder<'brw>,
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

        let id = self.entities_serialized.entity_to_id(*value);

        SerializedValue::Primitive(SerializedPrimitive::Int(id as i128))
    }

    fn deserialize(&self, value: &mut Self::Deserialized, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }

        let result = match serialized {
            SerializedValue::Primitive(SerializedPrimitive::Int(number)) => {
                match *number {
                    0 => Some(EntityId::EMPTY),
                    number => Some(self.entities_deserialized.entity_from_id(number as u64)),
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

pub struct AssetInstantiateTransSerializer<'a, T: 'static> {
    asset_storage: Option<&'a AssetStorageResource<T>>
}
impl<'a, T: 'static> AssetInstantiateTransSerializer<'a, T> {
    pub fn new(asset_storage: Option<&'a AssetStorageResource<T>>) -> Self {
        Self {
            asset_storage,
        }
    }
}
impl<'a, T: 'static, S: Copy> Serializer<S> for AssetInstantiateTransSerializer<'a, T> {
    type Deserialized = AssetHandle<T>;

    fn serialize(&self, _value: &Self::Deserialized, mut _ctx: SerializerCtx<S>, _path: &str) -> SerializedValue {
        unimplemented!("AssetInstantiateTransSerializer is for instantiation only");
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

        let asset_handle = self.asset_storage.map(|s| s.get_registered(serialized.as_str())).flatten().cloned();

        let Some(asset_handle) = asset_handle else {
            ctx.report_err(SerializationError::InvalidInput { message: format!("AssetHandle {serialized} cannot be loaded").into() });
            return;
        };

        *value = asset_handle.clone();
    }

    fn serializable_default(&self) -> Self::Deserialized {
        Default::default()
    }
}
