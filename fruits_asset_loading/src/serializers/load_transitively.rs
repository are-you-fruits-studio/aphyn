use std::{collections::HashMap, path::Path, sync::Mutex};

use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_audio::{AudioClip, AudioStateResource};
use fruits_ecs::ResourcesHolderMut;
use fruits_prefab::{Prefab, Scene};
use fruits_render_core::{RenderApiResource, StandardMaterial, StandardMesh, StandardTexture, StandardTextureAssetMetadata};
use fruits_serialization::*;

use crate::{AudioClipLoader, EntityIdDecoder, EntityIdEncoder, EntityTransSerializer, MaterialLoader, MeshLoader, PrefabLoader, SceneLoader, TextureLoader};

pub fn load_asset_transitively_from_world<R>(
    res: ResourcesHolderMut,
    assets_dir_path: impl AsRef<Path>,
    f: impl FnOnce(TransSerializerRegistry, &SerializersResource) -> R,
) -> Option<R> {
    Some(unsafe {
        load_asset_transitively::<R>(
            &*res.get_ptr::<RenderApiResource>()?,
            &mut *res.get_ptr::<AudioStateResource>()?,
            &*res.get_ptr::<SerializersResource>()?,
            &mut *res.get_ptr::<AssetStorageResource<Prefab>>()?,
            &mut *res.get_ptr::<AssetStorageResource<Scene>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardTexture>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardMaterial>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardMesh>>()?,
            &mut *res.get_ptr::<AssetStorageResource<AudioClip>>()?,
            assets_dir_path,
            f,
        )
    })
}

pub fn load_asset_transitively<R>(
    render_api: &RenderApiResource,
    audio_state: &mut AudioStateResource,
    serializers: &SerializersResource,
    prefabs: &mut AssetStorageResource<Prefab>,
    scenes: &mut AssetStorageResource<Scene>,
    textures: &mut AssetStorageResource<StandardTexture>,
    materials: &mut AssetStorageResource<StandardMaterial>,
    meshes: &mut AssetStorageResource<StandardMesh>,
    audio_clips: &mut AssetStorageResource<AudioClip>,
    assets_dir_path: impl AsRef<Path>,
    f: impl FnOnce(TransSerializerRegistry, &SerializersResource) -> R,
) -> R {
    let (
        audio_state,
        prefabs,
        scenes,
        textures,
        materials,
        meshes,
        audio_clips
    ) = (
        &Mutex::new(audio_state),
        &Mutex::new(prefabs),
        &Mutex::new(scenes),
        &Mutex::new(textures),
        &Mutex::new(materials),
        &Mutex::new(meshes),
        &Mutex::new(audio_clips),
    );

    let assets_dir_path = assets_dir_path.as_ref();

    let mut serializer_local = TransSerializerRegistry::new();

    serializer_local.register(EntityTransSerializer::new(EntityIdDecoder::Empty, EntityIdEncoder::Empty));

    serializer_local.register(TransitiveLoadTransSerializer { 
        assets: textures,
        assets_dir_path,
        loader: |mut ctx, value, assets_dir_path| {
            TextureLoader {
                render_api: render_api,
            // todo: use real path?
            }.load_from_deserialized(ctx.deserialize_default::<StandardTextureAssetMetadata>("", value), assets_dir_path)
        },
    });
    serializer_local.register(TransitiveLoadTransSerializer { 
        assets: materials,
        assets_dir_path,
        loader: |mut ctx, value, _assets_dir_path| {
            // todo: use real path?
            let asset_metadata = ctx.deserialize_default("", value);
            MaterialLoader {
                render_api: render_api,
            }.load_from_deserialized(asset_metadata, &*textures.lock().unwrap())
        },
    });
    serializer_local.register(TransitiveLoadTransSerializer { 
        assets: meshes,
        assets_dir_path,
        loader: |mut ctx, value, assets_dir_path| {
            MeshLoader {
                render_api: render_api,
            // todo: use real path?
            }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
        },
    });
    serializer_local.register(TransitiveLoadTransSerializer { 
        assets: audio_clips,
        assets_dir_path,
        loader: |mut ctx, value, assets_dir_path| {
            AudioClipLoader {
                audio_state: &mut *audio_state.lock().unwrap(),
            // todo: use real path?
            }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
        },
    });
    // todo: fonts
    serializer_local.register(TransitiveLoadTransSerializer { 
        assets: prefabs,
        assets_dir_path,
        loader: |ctx, value, assets_dir_path| {
            PrefabLoader.load_from_serialized(ctx, value)
        },
    });
    serializer_local.register(TransitiveLoadTransSerializer {
        assets: scenes,
        assets_dir_path,
        loader: |ctx, value, assets_dir_path| {
            SceneLoader.load_from_serialized(ctx, value)
        },
    });

    f(serializer_local, serializers)
}

pub struct TransitiveLoadTransSerializer<'m, 'brw: 'm, T: 'static, F: 'm + Send + Sync + Fn(SerializerCtx<TransSerializerCtxState>, &SerializedValue, &Path) -> Option<T>> {
    pub assets: &'m Mutex<&'brw mut AssetStorageResource<T>>,
    pub assets_dir_path: &'m Path,
    pub loader: F,
}
impl<'a, 'm, 'brw: 'm, T: 'static, F: 'm + Send + Sync + Fn(SerializerCtx<TransSerializerCtxState>, &SerializedValue, &Path) -> Option<T>> Serializer<TransSerializerCtxState<'a>> for TransitiveLoadTransSerializer<'m, 'brw, T, F> {
    type Deserialized = AssetHandle<T>;

    fn serialize(&self, value: &Self::Deserialized, mut ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        let asset_storage = self.assets.lock().unwrap();
        let key = asset_storage.get_registration(value);

        if key.is_none() {
            ctx.report_err(SerializationError::InvalidInput { message: "AssetHandle is not registered".into() });
        }

        SerializedValue::Primitive(SerializedPrimitive::String(key.unwrap_or("").into()))
    }

    fn deserialize(&self, value: &mut Self::Deserialized, mut ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        *value = Default::default();

        let SerializedValue::Primitive(SerializedPrimitive::String(key)) = serialized else {
            ctx.report_err(SerializationError::InvalidInput { message: "AssetHandle can only be deserialized from string".into() });
            return;
        };

        if key.as_str().trim().is_empty() {
            return;
        }

        if let Some(handle) = { get_from_asset_storage_or_unregister_if_missing(&mut self.assets.lock().unwrap(), key) } {
            *value = handle;
            return;
        }

        let Some(serialized) = try_read_serialized_from_file(self.assets_dir_path, key) else {
            ctx.report_err(SerializationError::InvalidInput { message: format!("AssetHandle failed to be deserialized from key {key}").into() });
            return;
        };

        let Some(asset) = (self.loader)(ctx.as_mut(), &serialized, self.assets_dir_path) else {
            return;
        };
        
        *value = self.assets.lock().unwrap().insert_and_register(asset, key.clone());
    }
    
    fn serializable_default(&self) -> Self::Deserialized {
        Default::default()
    }
}

fn try_read_serialized_from_file(assets_dir_path: impl AsRef<Path>, key: &str) -> Option<SerializedValue> {
    let mut path = assets_dir_path.as_ref().to_path_buf();
    path.push(key);

    let raw_asset = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(_err) => return None,
    };

    let value = SerializedValue::from_json(&serde_json::from_str::<serde_json::Value>(&raw_asset).ok()?);

    Some(value)
}

fn get_from_asset_storage_or_unregister_if_missing<T: 'static>(
    storage: &mut AssetStorageResource<T>,
    key: &str,
) -> Option<AssetHandle<T>> {
    let Some(stored_asset) = storage.get_registered(key) else {
        return None;
    };

    if storage.get(stored_asset).is_some() {
        return Some(stored_asset.clone());
    }

    storage.unregister(key);
    None
}
