use crate::{features::asset_serialization::get_asset_type, *};

fn stored<'r, T: 'static + Send + Sync>(res: ResourcesHolderRef<'r>, key: &str) -> Option<&'r T> {
    let storage = res.get::<AssetStorageResource<T>>()?;
    storage.get(storage.get_registered(key)?)
}

// what the inspector shows of an asset: its metadata, or the material / template itself
pub fn clone_asset_for_inspection(res: ResourcesHolderRef, key: &str) -> Option<FfiAny> {
    let registry = res.get::<ReflectionRegistryResource>()?;

    let value = match get_asset_type(res, key)? {
        AssetType::Texture => FfiAnyRef::new(stored::<StandardTexture>(res, key)?.meta()?),
        AssetType::Mesh => FfiAnyRef::new(stored::<StandardMesh>(res, key)?.meta()?),
        AssetType::Shader => FfiAnyRef::new(stored::<Shader>(res, key)?.meta()?),
        AssetType::ComputeShader => FfiAnyRef::new(stored::<ComputeShader>(res, key)?.meta()?),
        AssetType::AudioClip => FfiAnyRef::new(stored::<AudioClip>(res, key)?.meta()?),
        AssetType::Material => FfiAnyRef::new(stored::<Material>(res, key)?),
        AssetType::MaterialTemplate => FfiAnyRef::new(stored::<MaterialTemplate>(res, key)?),
        AssetType::Font | AssetType::Prefab | AssetType::Scene | AssetType::AnimationClip => return None,
    };

    reflect_clone(registry, value)
}

fn take<T: 'static>(value: FfiAny) -> Option<T> {
    value.downcast::<T>().map(|v| v.into_inner())
}

fn replace_registered<T: 'static + Send + Sync>(res: ResourcesHolderMut, key: &str, asset: T) -> bool {
    let Some(storage) = res.into_get_mut::<AssetStorageResource<T>>() else {
        return false;
    };
    let Some(handle) = storage.get_registered(key).cloned() else {
        return false;
    };

    match storage.get_mut(&handle) {
        Some(stored) => {
            *stored = asset;
            true
        }
        None => false,
    }
}

// rebuilds the asset under the same handle from (a clone of) the inspected copy
pub fn reload_asset_from_inspected_copy(
    mut res: ResourcesHolderMut,
    key: &str,
    asset_type: AssetType,
    copy: FfiAnyRef,
    assets_dir_path: &str,
) -> bool {
    let Some(registry) = res.as_ref().get::<ReflectionRegistryResource>().cloned() else {
        return false;
    };
    let Some(copy) = reflect_clone(&registry, copy) else {
        return false;
    };

    let reloaded = reload_asset(res.as_mut(), key, asset_type, copy, assets_dir_path);

    if !reloaded {
        println!("failed to reload asset {key}");
    }

    reloaded
}

fn reload_asset(mut res: ResourcesHolderMut, key: &str, asset_type: AssetType, copy: FfiAny, assets_dir_path: &str) -> bool {
    macro_rules! reload_with {
        ($meta: ty, $asset: ty, |$render_api: ident, $meta_value: ident| $load: expr) => {{
            let Some($meta_value) = take::<$meta>(copy) else {
                return false;
            };
            let asset: Option<$asset> = {
                let Some($render_api) = res.as_ref().get::<RenderApiResource>() else {
                    return false;
                };
                $load
            };
            match asset {
                Some(asset) => replace_registered(res, key, asset),
                None => false,
            }
        }};
    }

    match asset_type {
        AssetType::Texture => reload_with!(StandardTextureAssetMetadata, StandardTexture, |render_api, meta| {
            TextureLoader { render_api }.load_from_deserialized(meta, assets_dir_path)
        }),
        AssetType::Mesh => reload_with!(StandardMeshAssetMetadata, StandardMesh, |render_api, meta| {
            MeshLoader { render_api }.load_from_deserialized(meta, assets_dir_path)
        }),
        AssetType::Shader => reload_with!(ShaderAssetMetadata, Shader, |render_api, meta| {
            ShaderLoader { render_api }.load_from_deserialized(meta, assets_dir_path)
        }),
        AssetType::ComputeShader => reload_with!(ComputeShaderAssetMetadata, ComputeShader, |render_api, meta| {
            ComputeShaderLoader { render_api }.load_from_deserialized(meta, assets_dir_path)
        }),
        AssetType::AudioClip => {
            let Some(meta) = take::<AudioClipAssetMetadata>(copy) else {
                return false;
            };
            let clip = {
                let Some(audio_state) = res.get_mut::<AudioStateResource>() else {
                    return false;
                };
                AudioClipLoader { audio_state }.load_from_deserialized(meta, assets_dir_path)
            };
            match clip {
                Some(clip) => replace_registered(res, key, clip),
                None => false,
            }
        }
        AssetType::Material => {
            let Some(material) = take::<Material>(copy) else {
                return false;
            };
            let material = {
                let loader = MaterialLoader::from_world(&res);
                let res = res.as_ref();
                let (Some(materials), Some(templates), Some(shaders)) = (
                    res.get::<AssetStorageResource<Material>>(),
                    res.get::<AssetStorageResource<MaterialTemplate>>(),
                    res.get::<AssetStorageResource<Shader>>(),
                ) else {
                    return false;
                };
                let old = materials.get_registered(key).and_then(|h| materials.get(h));
                loader.reload(material, old, templates, shaders)
            };
            replace_registered(res, key, material)
        }
        AssetType::MaterialTemplate => match take::<MaterialTemplate>(copy) {
            Some(template) => replace_registered(res, key, template),
            None => false,
        },
        AssetType::Font | AssetType::Prefab | AssetType::Scene | AssetType::AnimationClip => false,
    }
}
