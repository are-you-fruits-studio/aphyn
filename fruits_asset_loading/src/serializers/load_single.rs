use std::{collections::HashMap, path::Path, sync::Mutex};

use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_audio::{AudioClip, AudioStateResource};
use fruits_ecs::ResourcesHolderMut;
use fruits_ffi::FfiString;
use fruits_prefab::{Prefab, PrefabDependencies};
use fruits_render_core::{RenderApiResource, StandardMaterial, StandardMesh, StandardTexture};
use fruits_serialization::*;

use crate::{AudioClipLoader, EntityTransSerializer, MaterialLoader, MeshLoader, PrefabLoader, TextureLoader};

pub fn load_asset_single_from_world<R>(
    res: ResourcesHolderMut,
    assets_dir_path: impl AsRef<Path>,
    asset_key: &str,
    prefab_dependencies: Option<&mut PrefabDependencies>,
    f: impl FnOnce(TransSerializerRegistry, &SerializersResource) -> R,
) -> Option<R> {
    Some(unsafe {
        load_asset_single::<R>(
            &*res.get_ptr::<RenderApiResource>()?,
            &mut *res.get_ptr::<AudioStateResource>()?,
            &*res.get_ptr::<SerializersResource>()?,
            &mut *res.get_ptr::<AssetStorageResource<Prefab>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardTexture>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardMaterial>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardMesh>>()?,
            &mut *res.get_ptr::<AssetStorageResource<AudioClip>>()?,
            assets_dir_path,
            asset_key,
            prefab_dependencies,
            f,
        )
    })
}

pub fn load_asset_single<R>(
    render_api: &RenderApiResource,
    audio_state: &mut AudioStateResource,
    serializers: &SerializersResource,
    prefabs: &mut AssetStorageResource<Prefab>,
    textures: &mut AssetStorageResource<StandardTexture>,
    materials: &mut AssetStorageResource<StandardMaterial>,
    meshes: &mut AssetStorageResource<StandardMesh>,
    audio_clips: &mut AssetStorageResource<AudioClip>,
    assets_dir_path: impl AsRef<Path>,
    asset_key: &str,
    prefab_dependencies: Option<&mut PrefabDependencies>,
    f: impl FnOnce(TransSerializerRegistry, &SerializersResource) -> R,
) -> R {
    let (
        audio_state,
        prefabs,
        textures,
        materials,
        meshes,
        audio_clips,
        deps,
    ) = (
        &Mutex::new(audio_state),
        &Mutex::new(prefabs),
        &Mutex::new(textures),
        &Mutex::new(materials),
        &Mutex::new(meshes),
        &Mutex::new(audio_clips),
        prefab_dependencies.map(Mutex::new),
    );

    let assets_dir_path = assets_dir_path.as_ref();

    let entities_deserialized = HashMap::new();
    let entities_serialized = HashMap::new();

    let mut serializer_local = TransSerializerRegistry::new();

    // todo: collect deps on serialization as well?
    let deps = deps.as_ref();

    serializer_local.register(EntityTransSerializer::new(&entities_deserialized, &entities_serialized));

    serializer_local.register(SingleLoadTransSerializer { assets: textures, deps });
    serializer_local.register(SingleLoadTransSerializer { assets: materials, deps });
    serializer_local.register(SingleLoadTransSerializer { assets: meshes, deps });
    serializer_local.register(SingleLoadTransSerializer { assets: audio_clips, deps });
    // todo: font
    serializer_local.register(SingleLoadTransSerializer { assets: prefabs, deps });

    serializer_local.register(SingleDirectLoadTransSerializer { 
        assets: textures,
        assets_dir_path,
        asset_key,
        deps,
        loader: |mut ctx, value, assets_dir_path| {
            TextureLoader {
                render_api: render_api,
            // todo: use real path?
            }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
        },
    });
    serializer_local.register(SingleDirectLoadTransSerializer { 
        assets: materials,
        assets_dir_path,
        asset_key,
        deps,
        loader: |mut ctx, value, _assets_dir_path| {
            // todo: use real path?
            let asset_metadata = ctx.deserialize_default("", value);
            MaterialLoader {
                render_api: render_api,
            }.load_from_deserialized(asset_metadata, &*textures.lock().unwrap())
        },
    });
    serializer_local.register(SingleDirectLoadTransSerializer { 
        assets: meshes,
        assets_dir_path,
        asset_key,
        deps,
        loader: |mut ctx, value, assets_dir_path| {
            MeshLoader {
                render_api: render_api,
            // todo: use real path?
            }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
        },
    });
    serializer_local.register(SingleDirectLoadTransSerializer { 
        assets: audio_clips,
        assets_dir_path,
        asset_key,
        deps,
        loader: |mut ctx, value, assets_dir_path| {
            AudioClipLoader {
                audio_state: &mut *audio_state.lock().unwrap(),
            // todo: use real path?
            }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
        },
    });
    // todo: fonts
    serializer_local.register(SingleDirectLoadTransSerializer { 
        assets: prefabs,
        assets_dir_path,
        asset_key,
        deps,
        loader: |ctx, value, assets_dir_path| {
            PrefabLoader.load_from_serialized(ctx, value)
        },
    });

    f(serializer_local, serializers)
}

pub struct SingleLoadTransSerializer<'m, 'brw: 'm, T: 'static> {
    pub assets: &'m Mutex<&'brw mut AssetStorageResource<T>>,
    pub deps: Option<&'m Mutex<&'brw mut PrefabDependencies>>,
}
impl<'m, 'brw: 'm, T: 'static, S: Copy> Serializer<S> for SingleLoadTransSerializer<'m, 'brw, T> {
    type Deserialized = AssetHandle<T>;

    fn serialize(&self, value: &Self::Deserialized, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        let asset_storage = self.assets.lock().unwrap();
        let key = asset_storage.get_registration(&value);

        if key.is_none() {
            ctx.report_err(SerializationError::InvalidInput { message: "AssetHandle is not registered".into() });
        }

        SerializedValue::Primitive(SerializedPrimitive::String(key.unwrap_or("").into()))
    }

    fn deserialize(&self, value: &mut Self::Deserialized, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        *value = Default::default();

        let SerializedValue::Primitive(SerializedPrimitive::String(key)) = serialized else {
            ctx.report_err(SerializationError::InvalidInput { message: "AssetHandle can only be deserialized from string".into() });
            return;
        };

        *value = self.assets.lock().unwrap().get_registered(key.as_str()).cloned().unwrap_or_default()
    }
    
    fn serializable_default(&self) -> Self::Deserialized {
        Default::default()
    }
}


pub struct SingleDirectLoadTransSerializer<'m, 'brw: 'm, T: 'static, F: 'm + Send + Sync + Fn(SerializerCtx<TransSerializerCtxState>, &SerializedValue, &Path) -> Option<T>> {
    pub assets: &'m Mutex<&'brw mut AssetStorageResource<T>>,
    pub assets_dir_path: &'m Path,
    pub asset_key: &'m str,
    pub deps: Option<&'m Mutex<&'brw mut PrefabDependencies>>,
    pub loader: F,
}
impl<'a, 'm, 'brw: 'm, T: 'static, F: 'm + Send + Sync + Fn(SerializerCtx<TransSerializerCtxState>, &SerializedValue, &Path) -> Option<T>> Serializer<TransSerializerCtxState<'a>> for SingleDirectLoadTransSerializer<'m, 'brw, T, F> {
    type Deserialized = DirectDeserializedAsset<T>;

    fn serialize(&self, value: &Self::Deserialized, mut ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        let asset_storage = self.assets.lock().unwrap();
        let key = asset_storage.get_registration(&value.handle);

        if key.is_none() {
            ctx.report_err(SerializationError::InvalidInput { message: "AssetHandle is not registered".into() });
        }

        SerializedValue::Primitive(SerializedPrimitive::String(key.unwrap_or("").into()))
    }

    fn deserialize(&self, value: &mut Self::Deserialized, mut ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        let Some(asset) = (self.loader)(ctx.as_mut(), &serialized, self.assets_dir_path) else {
            *value = self.serializable_default();
            return;
        };

        let asset_key: FfiString = self.asset_key.into();
        let asset_handle = self.assets.lock().unwrap().insert_and_register(asset, asset_key.clone());

        *value = DirectDeserializedAsset {
            handle: asset_handle,
            key: asset_key,
        };
    }
    
    fn serializable_default(&self) -> Self::Deserialized {
        DirectDeserializedAsset {
            handle: AssetHandle::EMPTY,
            key: Default::default(),
        }
    }
}

pub struct DirectDeserializedAsset<T> {
    handle: AssetHandle<T>,
    key: FfiString,
}

impl<T> Default for DirectDeserializedAsset<T> {
    fn default() -> Self {
        Self {
            handle: Default::default(),
            key: Default::default(),
        }
    }
}