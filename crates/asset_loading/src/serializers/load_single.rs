use std::{path::Path, sync::Mutex};

use aphyn_animation::{AnimationClip, AnimationClipAssetMetadata};
use aphyn_asset_storage::AssetStorageResource;
use aphyn_audio::{AudioClip, AudioStateResource};
use aphyn_ecs::{ResourcesHolderMut, TypesRegistryCache};
use aphyn_prefab::{Prefab, Scene};
use aphyn_render::{
    ComputeShader, ComputeShaderAssetMetadata, Material, MaterialTemplate, RenderApiResource, Shader, ShaderAssetMetadata,
    StandardMesh, StandardTexture,
};
use aphyn_serialization::*;
use aphyn_serialization_ecs::SerializersResource;
use aphyn_ui::{Font, FontAssetMetadata};

use crate::{
    AssetHandleCtx, AssetHandleCtxKey, AudioClipLoader, ComputeShaderLoader, FontLoader, DirectAssetLoadCtx, DirectAssetLoadCtxKey, EntityIdCtx,
    EntityIdCtxKey, MaterialLoader, MeshLoader, PrefabLoader, SceneLoader, ShaderLoader, TextureLoader, asset_loader,
    deserialize_material,
};

pub fn load_asset_single_from_world<R>(
    res: ResourcesHolderMut,
    assets_dir_path: impl AsRef<Path>,
    asset_key: &str,
    entity_ids: EntityIdCtx,
    f: impl FnOnce(TransSerializerCtxState) -> R,
) -> Option<R> {
    let material_loader = MaterialLoader::from_world(&res);

    Some(unsafe {
        load_asset_single::<R>(
            &*res.get_ptr::<RenderApiResource>()?,
            material_loader,
            &mut *res.get_ptr::<AudioStateResource>()?,
            &*res.get_ptr::<SerializersResource>()?,
            res.types(),
            &mut *res.get_ptr::<AssetStorageResource<Prefab>>()?,
            &mut *res.get_ptr::<AssetStorageResource<Scene>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardTexture>>()?,
            &mut *res.get_ptr::<AssetStorageResource<Material>>()?,
            &mut *res.get_ptr::<AssetStorageResource<StandardMesh>>()?,
            &mut *res.get_ptr::<AssetStorageResource<AudioClip>>()?,
            &mut *res.get_ptr::<AssetStorageResource<Shader>>()?,
            &mut *res.get_ptr::<AssetStorageResource<ComputeShader>>()?,
            &mut *res.get_ptr::<AssetStorageResource<MaterialTemplate>>()?,
            &mut *res.get_ptr::<AssetStorageResource<AnimationClip>>()?,
            // optional: worlds without the ui module have no fonts
            res.get_ptr::<AssetStorageResource<Font>>().map(|fonts| &mut *fonts),
            assets_dir_path,
            asset_key,
            entity_ids,
            f,
        )
    })
}

pub fn load_asset_single<R>(
    render_api: &RenderApiResource,
    material_loader: MaterialLoader,
    audio_state: &mut AudioStateResource,
    serializers: &TransSerializerRegistry,
    local_types: &TypesRegistryCache,
    prefabs: &mut AssetStorageResource<Prefab>,
    scenes: &mut AssetStorageResource<Scene>,
    textures: &mut AssetStorageResource<StandardTexture>,
    materials: &mut AssetStorageResource<Material>,
    meshes: &mut AssetStorageResource<StandardMesh>,
    audio_clips: &mut AssetStorageResource<AudioClip>,
    shaders: &mut AssetStorageResource<Shader>,
    compute_shaders: &mut AssetStorageResource<ComputeShader>,
    templates: &mut AssetStorageResource<MaterialTemplate>,
    animation_clips: &mut AssetStorageResource<AnimationClip>,
    fonts: Option<&mut AssetStorageResource<Font>>,
    assets_dir_path: impl AsRef<Path>,
    asset_key: &str,
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
    let animation_clips = Mutex::new(&mut *animation_clips);
    let fonts = fonts.map(Mutex::new);

    let texture_loader = asset_loader(|mut ctx, value, assets_dir_path| {
        // todo: use real path?
        TextureLoader { render_api }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
    });
    let material_loader = asset_loader(|ctx, value, _assets_dir_path| {
        // todo: use real path?
        reload_material(ctx, value, &materials, &templates, &shaders, &material_loader, asset_key)
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
    let animation_clip_loader = asset_loader(|mut ctx, value, _assets_dir_path| {
        let meta = ctx.deserialize_default::<AnimationClipAssetMetadata>("", value);
        Some(AnimationClip::from_meta(&mut ctx, &meta))
    });
    let font_loader = asset_loader(|mut ctx, value, assets_dir_path| {
        FontLoader { render_api }.load_from_deserialized(
            ctx.deserialize_default::<FontAssetMetadata>("", value),
            &mut textures.lock().unwrap(),
            assets_dir_path,
        )
    });
    let prefab_loader = asset_loader(|ctx, value, _assets_dir_path| PrefabLoader.load_from_serialized(ctx, value));
    let scene_loader = asset_loader(|ctx, value, _assets_dir_path| SceneLoader.load_from_serialized(ctx, value));

    let handle_entries = (
        AssetHandleCtx::RegisteredLocked(&textures),
        AssetHandleCtx::RegisteredLocked(&materials),
        AssetHandleCtx::RegisteredLocked(&meshes),
        AssetHandleCtx::RegisteredLocked(&audio_clips),
        AssetHandleCtx::RegisteredLocked(&shaders),
        AssetHandleCtx::RegisteredLocked(&compute_shaders),
        AssetHandleCtx::RegisteredLocked(&templates),
        AssetHandleCtx::RegisteredLocked(&prefabs),
        AssetHandleCtx::RegisteredLocked(&scenes),
        AssetHandleCtx::RegisteredLocked(&animation_clips),
    );
    let direct_entries = (
        DirectAssetLoadCtx {
            assets: &textures,
            assets_dir_path,
            asset_key,
            loader: &texture_loader,
        },
        DirectAssetLoadCtx {
            assets: &materials,
            assets_dir_path,
            asset_key,
            loader: &material_loader,
        },
        DirectAssetLoadCtx {
            assets: &meshes,
            assets_dir_path,
            asset_key,
            loader: &mesh_loader,
        },
        DirectAssetLoadCtx {
            assets: &audio_clips,
            assets_dir_path,
            asset_key,
            loader: &audio_clip_loader,
        },
        DirectAssetLoadCtx {
            assets: &shaders,
            assets_dir_path,
            asset_key,
            loader: &shader_loader,
        },
        DirectAssetLoadCtx {
            assets: &compute_shaders,
            assets_dir_path,
            asset_key,
            loader: &compute_shader_loader,
        },
        DirectAssetLoadCtx {
            assets: &templates,
            assets_dir_path,
            asset_key,
            loader: &template_loader,
        },
        DirectAssetLoadCtx {
            assets: &prefabs,
            assets_dir_path,
            asset_key,
            loader: &prefab_loader,
        },
        DirectAssetLoadCtx {
            assets: &scenes,
            assets_dir_path,
            asset_key,
            loader: &scene_loader,
        },
        DirectAssetLoadCtx {
            assets: &animation_clips,
            assets_dir_path,
            asset_key,
            loader: &animation_clip_loader,
        },
    );

    let context = ContextData::new()
        .with::<EntityIdCtxKey>(&entity_ids)
        .with::<AssetHandleCtxKey<StandardTexture>>(&handle_entries.0)
        .with::<AssetHandleCtxKey<Material>>(&handle_entries.1)
        .with::<AssetHandleCtxKey<StandardMesh>>(&handle_entries.2)
        .with::<AssetHandleCtxKey<AudioClip>>(&handle_entries.3)
        .with::<AssetHandleCtxKey<Shader>>(&handle_entries.4)
        .with::<AssetHandleCtxKey<ComputeShader>>(&handle_entries.5)
        .with::<AssetHandleCtxKey<MaterialTemplate>>(&handle_entries.6)
        .with::<AssetHandleCtxKey<Prefab>>(&handle_entries.7)
        .with::<AssetHandleCtxKey<Scene>>(&handle_entries.8)
        .with::<AssetHandleCtxKey<AnimationClip>>(&handle_entries.9)
        .with::<DirectAssetLoadCtxKey<StandardTexture>>(&direct_entries.0)
        .with::<DirectAssetLoadCtxKey<Material>>(&direct_entries.1)
        .with::<DirectAssetLoadCtxKey<StandardMesh>>(&direct_entries.2)
        .with::<DirectAssetLoadCtxKey<AudioClip>>(&direct_entries.3)
        .with::<DirectAssetLoadCtxKey<Shader>>(&direct_entries.4)
        .with::<DirectAssetLoadCtxKey<ComputeShader>>(&direct_entries.5)
        .with::<DirectAssetLoadCtxKey<MaterialTemplate>>(&direct_entries.6)
        .with::<DirectAssetLoadCtxKey<Prefab>>(&direct_entries.7)
        .with::<DirectAssetLoadCtxKey<Scene>>(&direct_entries.8)
        .with::<DirectAssetLoadCtxKey<AnimationClip>>(&direct_entries.9);

    let font_handle_entry = fonts.as_ref().map(AssetHandleCtx::RegisteredLocked);
    let font_direct_entry = fonts.as_ref().map(|fonts| DirectAssetLoadCtx {
        assets: fonts,
        assets_dir_path,
        asset_key,
        loader: &font_loader,
    });
    let mut context = context;
    if let (Some(handle_entry), Some(direct_entry)) = (&font_handle_entry, &font_direct_entry) {
        context.insert::<AssetHandleCtxKey<Font>>(handle_entry);
        context.insert::<DirectAssetLoadCtxKey<Font>>(direct_entry);
    }

    f(serializers.to_ctx_state(&context, local_types))
}

pub(crate) fn reload_material(
    ctx: SerializerCtx<TransSerializerCtxState>,
    value: &SerializedValue,
    materials: &Mutex<&mut AssetStorageResource<Material>>,
    templates: &Mutex<&mut AssetStorageResource<MaterialTemplate>>,
    shaders: &Mutex<&mut AssetStorageResource<Shader>>,
    material_loader: &MaterialLoader,
    asset_key: &str,
) -> Option<Material> {
    let material = deserialize_material(ctx, value, templates);
    let materials = materials.lock().unwrap();
    let old = materials.get_registered(asset_key).and_then(|handle| materials.get(handle));
    Some(material_loader.reload(material, old, &templates.lock().unwrap(), &shaders.lock().unwrap()))
}
