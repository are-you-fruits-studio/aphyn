use std::{path::Path, sync::Mutex};

use aphyn_asset_storage::AssetStorageResource;
use aphyn_audio::{AudioClip, AudioStateResource};
use aphyn_ecs::ResourcesHolderMut;
use aphyn_prefab::{Prefab, Scene};
use aphyn_render::{
    ComputeShader, ComputeShaderAssetMetadata, Material, MaterialTemplate, RenderApiResource, Shader, ShaderAssetMetadata,
    StandardMesh, StandardTexture, StandardTextureAssetMetadata,
};
use aphyn_serialization::*;
use aphyn_serialization_ecs::SerializersResource;

use crate::{
    AssetHandleCtx, AssetHandleCtxKey, AudioClipLoader, ComputeShaderLoader, EntityIdCtx, EntityIdCtxKey, MaterialLoader,
    MeshLoader, PrefabLoader, SceneLoader, ShaderLoader, TextureLoader, asset_loader, deserialize_material,
};

pub fn load_asset_transitively_from_world<R>(
    res: ResourcesHolderMut,
    assets_dir_path: impl AsRef<Path>,
    entity_ids: EntityIdCtx,
    f: impl FnOnce(TransSerializerCtxState) -> R,
) -> Option<R> {
    let material_loader = MaterialLoader::from_world(&res);

    Some(unsafe {
        load_asset_transitively::<R>(
            &*res.get_ptr::<RenderApiResource>()?,
            material_loader,
            &mut *res.get_ptr::<AudioStateResource>()?,
            &*res.get_ptr::<SerializersResource>()?,
            &mut *res.get_ptr::<AssetStorageResource<Prefab>>()?,
            &mut *res.get_ptr::<AssetStorageResource<Scene>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardTexture>>()?,
            &mut *res.get_ptr::<AssetStorageResource<Material>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardMesh>>()?,
            &mut *res.get_ptr::<AssetStorageResource<AudioClip>>()?,
            &mut *res.get_ptr::<AssetStorageResource<Shader>>()?,
            &mut *res.get_ptr::<AssetStorageResource<ComputeShader>>()?,
            &mut *res.get_ptr::<AssetStorageResource<MaterialTemplate>>()?,
            assets_dir_path,
            entity_ids,
            f,
        )
    })
}

pub fn load_asset_transitively<R>(
    render_api: &RenderApiResource,
    material_loader: MaterialLoader,
    audio_state: &mut AudioStateResource,
    serializers: &TransSerializerRegistry,
    prefabs: &mut AssetStorageResource<Prefab>,
    scenes: &mut AssetStorageResource<Scene>,
    textures: &mut AssetStorageResource<StandardTexture>,
    materials: &mut AssetStorageResource<Material>,
    meshes: &mut AssetStorageResource<StandardMesh>,
    audio_clips: &mut AssetStorageResource<AudioClip>,
    shaders: &mut AssetStorageResource<Shader>,
    compute_shaders: &mut AssetStorageResource<ComputeShader>,
    templates: &mut AssetStorageResource<MaterialTemplate>,
    assets_dir_path: impl AsRef<Path>,
    entity_ids: EntityIdCtx,
    f: impl FnOnce(TransSerializerCtxState) -> R,
) -> R {
    let assets_dir_path = assets_dir_path.as_ref();

    let audio_state = Mutex::new(audio_state);
    let prefabs = Mutex::new(&mut *prefabs);
    let scenes = Mutex::new(&mut *scenes);
    let textures = Mutex::new(&mut *textures);
    let materials = Mutex::new(&mut *materials);
    let meshes = Mutex::new(&mut *meshes);
    let audio_clips = Mutex::new(&mut *audio_clips);
    let shaders = Mutex::new(&mut *shaders);
    let compute_shaders = Mutex::new(&mut *compute_shaders);
    let templates = Mutex::new(&mut *templates);

    let texture_loader = asset_loader(|mut ctx, value, assets_dir_path| {
        // todo: use real path?
        TextureLoader { render_api }.load_from_deserialized(
            ctx.deserialize_default::<StandardTextureAssetMetadata>("", value),
            assets_dir_path,
        )
    });
    let material_loader = asset_loader(|ctx, value, _assets_dir_path| {
        // todo: use real path?
        let material = deserialize_material(ctx, value, &templates);
        Some(material_loader.bind(material, &templates.lock().unwrap(), &shaders.lock().unwrap()))
    });
    let mesh_loader = asset_loader(|mut ctx, value, assets_dir_path| {
        // todo: use real path?
        MeshLoader { render_api }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
    });
    let audio_clip_loader = asset_loader(|mut ctx, value, assets_dir_path| {
        // todo: use real path?
        AudioClipLoader {
            audio_state: &mut *audio_state.lock().unwrap(),
        }
        .load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
    });
    let shader_loader = asset_loader(|mut ctx, value, assets_dir_path| {
        ShaderLoader { render_api }
            .load_from_deserialized(ctx.deserialize_default::<ShaderAssetMetadata>("", value), assets_dir_path)
    });
    let compute_shader_loader = asset_loader(|mut ctx, value, assets_dir_path| {
        ComputeShaderLoader { render_api }.load_from_deserialized(
            ctx.deserialize_default::<ComputeShaderAssetMetadata>("", value),
            assets_dir_path,
        )
    });
    let template_loader =
        asset_loader(|mut ctx, value, _assets_dir_path| Some(ctx.deserialize_default::<MaterialTemplate>("", value)));
    // todo: fonts
    let prefab_loader = asset_loader(|ctx, value, _assets_dir_path| PrefabLoader.load_from_serialized(ctx, value));
    let scene_loader = asset_loader(|ctx, value, _assets_dir_path| SceneLoader.load_from_serialized(ctx, value));

    let entries = (
        AssetHandleCtx::LoadTransitively {
            assets: &textures,
            assets_dir_path,
            loader: &texture_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &materials,
            assets_dir_path,
            loader: &material_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &meshes,
            assets_dir_path,
            loader: &mesh_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &audio_clips,
            assets_dir_path,
            loader: &audio_clip_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &shaders,
            assets_dir_path,
            loader: &shader_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &compute_shaders,
            assets_dir_path,
            loader: &compute_shader_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &templates,
            assets_dir_path,
            loader: &template_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &prefabs,
            assets_dir_path,
            loader: &prefab_loader,
        },
        AssetHandleCtx::LoadTransitively {
            assets: &scenes,
            assets_dir_path,
            loader: &scene_loader,
        },
    );

    let context = ContextData::new()
        .with::<EntityIdCtxKey>(&entity_ids)
        .with::<AssetHandleCtxKey<StandardTexture>>(&entries.0)
        .with::<AssetHandleCtxKey<Material>>(&entries.1)
        .with::<AssetHandleCtxKey<StandardMesh>>(&entries.2)
        .with::<AssetHandleCtxKey<AudioClip>>(&entries.3)
        .with::<AssetHandleCtxKey<Shader>>(&entries.4)
        .with::<AssetHandleCtxKey<ComputeShader>>(&entries.5)
        .with::<AssetHandleCtxKey<MaterialTemplate>>(&entries.6)
        .with::<AssetHandleCtxKey<Prefab>>(&entries.7)
        .with::<AssetHandleCtxKey<Scene>>(&entries.8);

    f(serializers.to_ctx_state(&context))
}
