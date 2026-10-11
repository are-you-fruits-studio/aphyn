use std::{collections::HashMap, sync::Mutex};

use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_audio::{AudioClip, AudioClipAssetMetadata};
use fruits_ecs::ResourcesHolderRef;
use fruits_ffi::FfiString;
use fruits_prefab::{Prefab, PrefabDependencies};
use fruits_render_core::{CoordinateSpaceType, StandardMaterial, StandardMesh, StandardMeshAssetMetadata, StandardTexture};
use fruits_serialization::*;

use crate::{EntityTransSerializer, serialize_prefab_no_deps};

//

pub fn save_with_asset_serializers_from_world<R>(
    res: ResourcesHolderRef,
    prefab_dependencies: Option<&mut PrefabDependencies>,
    f: impl FnOnce(TransSerializerRegistry) -> R,
) -> Option<R> {
    Some(
        save_with_asset_serializers::<R>(
            &*res.get::<AssetStorageResource<Prefab>>()?,
            &*res.get::<AssetStorageResource<StandardTexture>>()?,
            &*res.get::<AssetStorageResource<StandardMaterial>>()?,
            &*res.get::<AssetStorageResource<StandardMesh>>()?,
            &*res.get::<AssetStorageResource<AudioClip>>()?,
            prefab_dependencies,
            f,
        )
    )
}

pub fn save_with_asset_serializers<R>(
    prefabs: &AssetStorageResource<Prefab>,
    textures: &AssetStorageResource<StandardTexture>,
    materials: &AssetStorageResource<StandardMaterial>,
    meshes: &AssetStorageResource<StandardMesh>,
    audio_clips: &AssetStorageResource<AudioClip>,
    prefab_dependencies: Option<&mut PrefabDependencies>,
    f: impl FnOnce(TransSerializerRegistry) -> R,
) -> R {
    let deps = prefab_dependencies.map(Mutex::new);

    let entities_deserialized = HashMap::new();
    let entities_serialized = HashMap::new();

    let mut serializer_local = TransSerializerRegistry::new();

    // todo: collect deps on serialization as well?
    let deps = deps.as_ref();

    serializer_local.register(EntityTransSerializer::new(&entities_deserialized, &entities_serialized));

    serializer_local.register(AssetHandleLinkTransSerializer { assets: textures });
    serializer_local.register(AssetHandleLinkTransSerializer { assets: materials });
    serializer_local.register(AssetHandleLinkTransSerializer { assets: meshes });
    serializer_local.register(AssetHandleLinkTransSerializer { assets: audio_clips });
    // todo: font
    serializer_local.register(AssetHandleLinkTransSerializer { assets: prefabs });

    serializer_local.register(DirectAssetSaveTransSerializer {
        assets: textures,
        extractor: |_, a| a.meta().cloned().unwrap_or_default(),
    });
    serializer_local.register(DirectAssetSaveTransSerializer {
        assets: materials,
        extractor: |_, a| a.meta().clone(),
    });
    serializer_local.register(DirectAssetSaveTransSerializer {
        assets: meshes,
        extractor: |_, a| a.meta().cloned().unwrap_or_else(|| StandardMeshAssetMetadata {
            raw_mesh: Default::default(),
            coordinate_space: CoordinateSpaceType::LeftHandZForward,
            has_clockwise_winding: Default::default(),
            has_inverted_u: Default::default(),
            has_inverted_v: Default::default(),
        }),
    });
    serializer_local.register(DirectAssetSaveTransSerializer {
        assets: audio_clips,
        extractor: |_, a| a.meta().cloned().unwrap_or_else(|| AudioClipAssetMetadata {
            raw_audio: Default::default(),
        }),
    });
    // todo: fonts
    serializer_local.register(DirectAssetSaveTransSerializer {
        assets: prefabs,
        extractor: |ctx, p| serialize_prefab_no_deps(ctx.map_state(|_| PureSerializerCtxState), &p.entities).clone(),
    });

    f(serializer_local)
}

//

// todo: use path where appropriate

#[repr(C)]
pub enum DirectSerializableAsset<T> {
    Handle(AssetHandle<T>),
    Key(FfiString),
}

pub struct AssetHandleLinkTransSerializer<'m, T: 'static> {
    assets: &'m AssetStorageResource<T>,
}
impl<'m, T: 'static, S: Copy> Serializer<S> for AssetHandleLinkTransSerializer<'m, T> {
    type Deserialized = AssetHandle<T>;

    fn serialize(&self, value: &Self::Deserialized, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        let key = self.assets.get_registration(value);

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

        let Some(stored_asset) = self.assets.get_registered(key) else {
            ctx.report_err(SerializationError::InvalidInput { message: format!("AssetHandle with a key \"{key}\" not found").into() });
            return;
        };

        *value = stored_asset.clone()
    }
    
    fn serializable_default(&self) -> Self::Deserialized {
        Default::default()
    }
}

pub struct DirectAssetSaveTransSerializer<'m, T: 'static, S: 'static, F: Fn(SerializerCtx<TransSerializerCtxState>, &T) -> S> {
    assets: &'m AssetStorageResource<T>,
    extractor: F,
}
impl<'a, 'm, T: 'static, S: 'static, F: Fn(SerializerCtx<TransSerializerCtxState>, &T) -> S> Serializer<TransSerializerCtxState<'a>> for DirectAssetSaveTransSerializer<'m, T, S, F> {
    type Deserialized = DirectSerializableAsset<T>;

    fn serialize(&self, value: &Self::Deserialized, mut ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        let serializable_part = {
            let assets = self.assets;

            let asset_handle = match value {
                DirectSerializableAsset::Handle(asset_handle) => asset_handle,
                DirectSerializableAsset::Key(key) => {
                    let Some(asset) = assets.get_registered(key.as_str()) else {
                        ctx.report_err(SerializationError::InvalidInput { message: format!("failed to serialize asset {}, Asset key is missing {}", std::any::type_name::<T>(), key.as_str()).into() });
                        return SerializedValue::Null;
                    };

                    asset
                },
            };

            let Some(asset) = assets.get(asset_handle) else {
                println!("failed to serialize asset {}, AssetHandle is invalid", std::any::type_name::<T>());
                return SerializedValue::Null;
            };

            (self.extractor)(ctx.as_mut(), asset)
        };

        ctx.serialize(&serializable_part, path)
    }

    fn deserialize(&self, value: &mut Self::Deserialized, mut ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        todo!()
    }
    
    fn serializable_default(&self) -> Self::Deserialized {
        todo!()
    }
}