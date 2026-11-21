use std::{marker::PhantomData, path::Path, sync::Mutex};

use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_audio::{AudioClip, AudioClipAssetMetadata};
use aphyn_ecs::EntityId;
use aphyn_ffi::FfiString;
use aphyn_prefab::{Prefab, Scene};
use aphyn_render::{
    ComputeShader, CoordinateSpaceType, Material, MaterialTemplate, Shader, StandardMesh, StandardMeshAssetMetadata, StandardTexture,
};
use aphyn_serialization::*;
use aphyn_ui::Font;

use crate::{EntityIdDecoder, EntityIdEncoder, serialize_entities_no_deps};

pub type AssetLoaderFn<'a, T> = dyn 'a + Fn(SerializerCtx<TransSerializerCtxState>, &SerializedValue, &Path) -> Option<T>;

pub fn asset_loader<T, F: Fn(SerializerCtx<TransSerializerCtxState>, &SerializedValue, &Path) -> Option<T>>(f: F) -> F {
    f
}

// todo: allow custom (e.g. dyn) entries for more agility
pub enum AssetHandleCtx<'a, T: 'static> {
    Registered(&'a AssetStorageResource<T>),
    RegisteredLocked(&'a Mutex<&'a mut AssetStorageResource<T>>),
    LoadTransitively {
        assets: &'a Mutex<&'a mut AssetStorageResource<T>>,
        assets_dir_path: &'a Path,
        loader: &'a AssetLoaderFn<'a, T>,
    },
}

impl<'a, T: 'static> AssetHandleCtx<'a, T> {
    pub fn with_storage<R>(&self, f: impl FnOnce(&AssetStorageResource<T>) -> R) -> R {
        match self {
            Self::Registered(assets) => f(assets),
            Self::RegisteredLocked(assets) | Self::LoadTransitively { assets, .. } => f(&**assets.lock().unwrap()),
        }
    }
}

pub struct AssetHandleCtxKey<T>(PhantomData<fn(T) -> T>);

impl<T: 'static> ContextDataKey for AssetHandleCtxKey<T> {
    type Value<'a> = AssetHandleCtx<'a, T>;
}

pub struct DirectAssetLoadCtx<'a, T: 'static> {
    pub assets: &'a Mutex<&'a mut AssetStorageResource<T>>,
    pub assets_dir_path: &'a Path,
    pub asset_key: &'a str,
    pub loader: &'a AssetLoaderFn<'a, T>,
}

pub struct DirectAssetLoadCtxKey<T>(PhantomData<fn(T) -> T>);

impl<T: 'static> ContextDataKey for DirectAssetLoadCtxKey<T> {
    type Value<'a> = DirectAssetLoadCtx<'a, T>;
}

pub struct EntityIdCtx<'a> {
    pub decoder: EntityIdDecoder<'a>,
    pub encoder: EntityIdEncoder<'a>,
}

impl<'a> EntityIdCtx<'a> {
    pub fn new(decoder: EntityIdDecoder<'a>, encoder: EntityIdEncoder<'a>) -> Self {
        Self { decoder, encoder }
    }
}

impl<'a> Default for EntityIdCtx<'a> {
    fn default() -> Self {
        Self::new(EntityIdDecoder::Empty, EntityIdEncoder::Empty)
    }
}

pub struct EntityIdCtxKey;

impl ContextDataKey for EntityIdCtxKey {
    type Value<'a> = EntityIdCtx<'a>;
}

//

pub fn register_asset_loading_serializers(registry: &mut TransSerializerRegistry) {
    registry.register(EntityIdSerializer);

    register_asset_serializers::<StandardTexture>(registry);
    register_asset_serializers::<Material>(registry);
    register_asset_serializers::<MaterialTemplate>(registry);
    register_asset_serializers::<StandardMesh>(registry);
    register_asset_serializers::<AudioClip>(registry);
    register_asset_serializers::<Shader>(registry);
    register_asset_serializers::<ComputeShader>(registry);
    register_asset_serializers::<Font>(registry);
    register_asset_serializers::<Prefab>(registry);
    register_asset_serializers::<Scene>(registry);

    registry.register(DirectAssetSaveSerializer::<StandardTexture, _>::new(|_, a| {
        a.meta().cloned().unwrap_or_default()
    }));
    registry.register(DirectAssetSaveSerializer::<Material, _>::new(|_, m| m.clone()));
    registry.register(DirectAssetSaveSerializer::<MaterialTemplate, _>::new(|_, t| t.clone()));
    registry.register(DirectAssetSaveSerializer::<Shader, _>::new(|_, s| {
        s.meta().cloned().unwrap_or_default()
    }));
    registry.register(DirectAssetSaveSerializer::<ComputeShader, _>::new(|_, s| {
        s.meta().cloned().unwrap_or_default()
    }));
    registry.register(DirectAssetSaveSerializer::<StandardMesh, _>::new(|_, a| {
        a.meta().cloned().unwrap_or_else(|| StandardMeshAssetMetadata {
            raw_mesh: Default::default(),
            coordinate_space: CoordinateSpaceType::LeftHandZForward,
            has_clockwise_winding: Default::default(),
            has_inverted_u: Default::default(),
            has_inverted_v: Default::default(),
        })
    }));
    registry.register(DirectAssetSaveSerializer::<AudioClip, _>::new(|_, a| {
        a.meta().cloned().unwrap_or_else(|| AudioClipAssetMetadata {
            raw_audio: Default::default(),
        })
    }));
    // todo: fonts
    registry.register(DirectAssetSaveSerializer::<Prefab, _>::new(|ctx, p| {
        serialize_entities_no_deps(ctx.map_state(|_| PureSerializerCtxState), &p.entities)
    }));
    registry.register(DirectAssetSaveSerializer::<Scene, _>::new(|ctx, s| {
        serialize_entities_no_deps(ctx.map_state(|_| PureSerializerCtxState), &s.entities)
    }));
}

fn register_asset_serializers<T: 'static>(registry: &mut TransSerializerRegistry) {
    registry.register(AssetHandleSerializer::<T>(PhantomData));
    registry.register(DirectAssetLoadSerializer::<T>(PhantomData));
}

fn missing_context_data<K: ContextDataKey>() -> SerializationError {
    SerializationError::MissingContextData {
        key: std::any::type_name::<K>().into(),
    }
}

fn read_key(ctx: &mut SerializerCtx<TransSerializerCtxState>, serialized: &SerializedValue) -> Option<FfiString> {
    let SerializedValue::Primitive(SerializedPrimitive::String(key)) = serialized else {
        ctx.report_err(SerializationError::InvalidInput {
            message: "AssetHandle can only be deserialized from string".into(),
        });
        return None;
    };

    Some(key.clone())
}

//

pub struct AssetHandleSerializer<T>(PhantomData<fn(T) -> T>);

impl<'x, T: 'static> Serializer<TransSerializerCtxState<'x>> for AssetHandleSerializer<T> {
    type Deserialized = AssetHandle<T>;

    fn serializable_default(&self) -> Self::Deserialized {
        Default::default()
    }

    fn serialize(
        &self,
        value: &Self::Deserialized,
        mut ctx: SerializerCtx<TransSerializerCtxState<'x>>,
        path: &str,
    ) -> SerializedValue {
        if !normalize_serialization_path(path).is_empty() {
            return SerializedValue::Null;
        }

        let Some(entry) = ctx.context_data::<AssetHandleCtxKey<T>>() else {
            ctx.report_err(missing_context_data::<AssetHandleCtxKey<T>>());
            return SerializedValue::Null;
        };

        let key = entry.with_storage(|assets| assets.get_registration(value).map(FfiString::from));

        if value != &AssetHandle::EMPTY && key.is_none() {
            ctx.report_err(SerializationError::InvalidInput {
                message: "AssetHandle is not registered".into(),
            });
        }

        SerializedValue::Primitive(SerializedPrimitive::String(key.unwrap_or_default()))
    }

    fn deserialize(
        &self,
        value: &mut Self::Deserialized,
        mut ctx: SerializerCtx<TransSerializerCtxState<'x>>,
        path: &str,
        serialized: &SerializedValue,
    ) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }

        *value = Default::default();

        let Some(entry) = ctx.context_data::<AssetHandleCtxKey<T>>() else {
            ctx.report_err(missing_context_data::<AssetHandleCtxKey<T>>());
            return;
        };

        let Some(key) = read_key(&mut ctx, serialized) else {
            return;
        };

        if key.as_str().trim().is_empty() {
            return;
        }

        let handle = match entry {
            AssetHandleCtx::Registered(assets) => assets.get_registered(&key).cloned(),
            AssetHandleCtx::RegisteredLocked(assets) => assets.lock().unwrap().get_registered(&key).cloned(),
            AssetHandleCtx::LoadTransitively {
                assets,
                assets_dir_path,
                loader,
            } => {
                if let Some(handle) = get_from_asset_storage_or_unregister_if_missing(&mut assets.lock().unwrap(), &key) {
                    *value = handle;
                    return;
                }

                let Some(serialized) = try_read_serialized_from_file(assets_dir_path, &key) else {
                    ctx.report_err(SerializationError::InvalidInput {
                        message: format!("AssetHandle failed to be deserialized from key {key}").into(),
                    });
                    return;
                };

                let Some(asset) = loader(ctx.as_mut(), &serialized, assets_dir_path) else {
                    return;
                };

                Some(assets.lock().unwrap().insert_and_register(asset, key.clone()))
            }
        };

        let Some(handle) = handle else {
            ctx.report_err(SerializationError::InvalidInput {
                message: format!("AssetHandle with a key \"{key}\" not found").into(),
            });
            return;
        };

        *value = handle;
    }
}

fn try_read_serialized_from_file(assets_dir_path: impl AsRef<Path>, key: &str) -> Option<SerializedValue> {
    let mut path = assets_dir_path.as_ref().to_path_buf();
    path.push(key);

    let raw_asset = std::fs::read_to_string(path).ok()?;

    Some(SerializedValue::from_json(
        &serde_json::from_str::<serde_json::Value>(&raw_asset).ok()?,
    ))
}

fn get_from_asset_storage_or_unregister_if_missing<T: 'static>(
    storage: &mut AssetStorageResource<T>,
    key: &str,
) -> Option<AssetHandle<T>> {
    let stored_asset = storage.get_registered(key)?;

    if storage.get(stored_asset).is_some() {
        return Some(stored_asset.clone());
    }

    storage.unregister(key);
    None
}

//

pub struct EntityIdSerializer;

impl<'x> Serializer<TransSerializerCtxState<'x>> for EntityIdSerializer {
    type Deserialized = EntityId;

    fn serializable_default(&self) -> Self::Deserialized {
        EntityId::EMPTY
    }

    fn serialize(&self, value: &Self::Deserialized, ctx: SerializerCtx<TransSerializerCtxState<'x>>, path: &str) -> SerializedValue {
        if !normalize_serialization_path(path).is_empty() {
            return SerializedValue::Null;
        }

        let id = ctx
            .context_data::<EntityIdCtxKey>()
            .map(|entity_ids| entity_ids.encoder.entity_to_id(*value))
            .unwrap_or(0);

        SerializedValue::Primitive(SerializedPrimitive::Int(id as i128))
    }

    fn deserialize(
        &self,
        value: &mut Self::Deserialized,
        mut ctx: SerializerCtx<TransSerializerCtxState<'x>>,
        path: &str,
        serialized: &SerializedValue,
    ) {
        if !normalize_serialization_path(path).is_empty() {
            return;
        }

        let result = match serialized {
            SerializedValue::Primitive(SerializedPrimitive::Int(number)) => match *number {
                0 => Some(EntityId::EMPTY),
                number => Some(
                    ctx.context_data::<EntityIdCtxKey>()
                        .map(|entity_ids| entity_ids.decoder.entity_from_id(number as u64))
                        .unwrap_or(EntityId::EMPTY),
                ),
            },
            _ => None,
        };

        if result.is_none() {
            ctx.report_err(SerializationError::InvalidInput {
                message: "failed to parse entity".into(),
            });
        }

        *value = result.unwrap_or(EntityId::EMPTY)
    }
}

//

#[repr(C)]
pub enum DirectSerializableAsset<T> {
    Handle(AssetHandle<T>),
    Key(FfiString),
}

pub struct DirectAssetSaveSerializer<T, E> {
    extractor: fn(SerializerCtx<TransSerializerCtxState>, &T) -> E,
}

impl<T, E> DirectAssetSaveSerializer<T, E> {
    pub fn new(extractor: fn(SerializerCtx<TransSerializerCtxState>, &T) -> E) -> Self {
        Self { extractor }
    }
}

impl<'x, T: 'static, E: 'static> Serializer<TransSerializerCtxState<'x>> for DirectAssetSaveSerializer<T, E> {
    type Deserialized = DirectSerializableAsset<T>;

    fn serializable_default(&self) -> Self::Deserialized {
        DirectSerializableAsset::Key(Default::default())
    }

    fn serialize(
        &self,
        value: &Self::Deserialized,
        mut ctx: SerializerCtx<TransSerializerCtxState<'x>>,
        path: &str,
    ) -> SerializedValue {
        let Some(entry) = ctx.context_data::<AssetHandleCtxKey<T>>() else {
            ctx.report_err(missing_context_data::<AssetHandleCtxKey<T>>());
            return SerializedValue::Null;
        };

        let extracted = entry.with_storage(|assets| {
            let asset_handle = match value {
                DirectSerializableAsset::Handle(asset_handle) => asset_handle,
                DirectSerializableAsset::Key(key) => match assets.get_registered(key.as_str()) {
                    Some(asset_handle) => asset_handle,
                    None => {
                        return Err(format!(
                            "failed to serialize asset {}, Asset key is missing {}",
                            std::any::type_name::<T>(),
                            key.as_str()
                        ));
                    }
                },
            };

            let Some(asset) = assets.get(asset_handle) else {
                return Err(format!(
                    "failed to serialize asset {}, AssetHandle is invalid",
                    std::any::type_name::<T>()
                ));
            };

            Ok((self.extractor)(ctx.as_mut(), asset))
        });

        match extracted {
            Ok(extracted) => ctx.serialize(&extracted, path),
            Err(message) => {
                ctx.report_err(SerializationError::InvalidInput { message: message.into() });
                SerializedValue::Null
            }
        }
    }

    fn deserialize(
        &self,
        _value: &mut Self::Deserialized,
        mut ctx: SerializerCtx<TransSerializerCtxState<'x>>,
        _path: &str,
        _serialized: &SerializedValue,
    ) {
        ctx.report_err(SerializationError::InvalidInput {
            message: "DirectSerializableAsset is for saving only".into(),
        });
    }
}

//

pub struct DirectDeserializedAsset<T> {
    pub handle: AssetHandle<T>,
    pub key: FfiString,
}

impl<T> Default for DirectDeserializedAsset<T> {
    fn default() -> Self {
        Self {
            handle: Default::default(),
            key: Default::default(),
        }
    }
}

pub struct DirectAssetLoadSerializer<T>(PhantomData<fn(T) -> T>);

impl<'x, T: 'static> Serializer<TransSerializerCtxState<'x>> for DirectAssetLoadSerializer<T> {
    type Deserialized = DirectDeserializedAsset<T>;

    fn serializable_default(&self) -> Self::Deserialized {
        Default::default()
    }

    fn serialize(
        &self,
        value: &Self::Deserialized,
        mut ctx: SerializerCtx<TransSerializerCtxState<'x>>,
        _path: &str,
    ) -> SerializedValue {
        let Some(entry) = ctx.context_data::<DirectAssetLoadCtxKey<T>>() else {
            ctx.report_err(missing_context_data::<DirectAssetLoadCtxKey<T>>());
            return SerializedValue::Null;
        };

        let key = entry
            .assets
            .lock()
            .unwrap()
            .get_registration(&value.handle)
            .map(FfiString::from);

        if key.is_none() {
            ctx.report_err(SerializationError::InvalidInput {
                message: "AssetHandle is not registered".into(),
            });
        }

        SerializedValue::Primitive(SerializedPrimitive::String(key.unwrap_or_default()))
    }

    fn deserialize(
        &self,
        value: &mut Self::Deserialized,
        mut ctx: SerializerCtx<TransSerializerCtxState<'x>>,
        _path: &str,
        serialized: &SerializedValue,
    ) {
        *value = Default::default();

        let Some(entry) = ctx.context_data::<DirectAssetLoadCtxKey<T>>() else {
            ctx.report_err(missing_context_data::<DirectAssetLoadCtxKey<T>>());
            return;
        };

        let Some(asset) = (entry.loader)(ctx.as_mut(), serialized, entry.assets_dir_path) else {
            return;
        };

        let asset_key: FfiString = entry.asset_key.into();
        let asset_handle = 'reg: {
            let mut assets = entry.assets.lock().unwrap();
            if let Some(registered_asset_handle) = assets.get_registered(&asset_key).cloned() {
                if let Some(stored_asset) = assets.get_mut(&registered_asset_handle) {
                    *stored_asset = asset;
                    break 'reg registered_asset_handle;
                }
            }

            assets.insert_and_register(asset, asset_key.clone())
        };

        *value = DirectDeserializedAsset {
            handle: asset_handle,
            key: asset_key,
        };
    }
}
