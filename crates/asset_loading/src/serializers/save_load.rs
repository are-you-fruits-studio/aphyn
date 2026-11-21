use aphyn_asset_storage::AssetStorageResource;
use aphyn_audio::AudioClip;
use aphyn_ecs::ResourcesHolderRef;
use aphyn_prefab::{Prefab, Scene};
use aphyn_render::{ComputeShader, Material, MaterialTemplate, Shader, StandardMesh, StandardTexture};
use aphyn_serialization::*;
use aphyn_serialization_ecs::SerializersResource;
use aphyn_ui::Font;

use crate::{AssetHandleCtx, AssetHandleCtxKey, EntityIdCtx, EntityIdCtxKey};

pub fn save_load_with_asset_serializers_from_world<R>(
    res: ResourcesHolderRef,
    entity_ids: EntityIdCtx,
    f: impl FnOnce(TransSerializerCtxState) -> R,
) -> Option<R> {
    Some(save_load_with_asset_serializers::<R>(
        &*res.get::<SerializersResource>()?,
        &*res.get::<AssetStorageResource<Prefab>>()?,
        &*res.get::<AssetStorageResource<Scene>>()?,
        &*res.get::<AssetStorageResource<StandardTexture>>()?,
        &*res.get::<AssetStorageResource<Material>>()?,
        &*res.get::<AssetStorageResource<MaterialTemplate>>()?,
        &*res.get::<AssetStorageResource<StandardMesh>>()?,
        &*res.get::<AssetStorageResource<AudioClip>>()?,
        &*res.get::<AssetStorageResource<Shader>>()?,
        &*res.get::<AssetStorageResource<ComputeShader>>()?,
        res.get::<AssetStorageResource<Font>>(),
        entity_ids,
        f,
    ))
}

pub fn save_load_with_asset_serializers<R>(
    serializers: &TransSerializerRegistry,
    prefabs: &AssetStorageResource<Prefab>,
    scenes: &AssetStorageResource<Scene>,
    textures: &AssetStorageResource<StandardTexture>,
    materials: &AssetStorageResource<Material>,
    templates: &AssetStorageResource<MaterialTemplate>,
    meshes: &AssetStorageResource<StandardMesh>,
    audio_clips: &AssetStorageResource<AudioClip>,
    shaders: &AssetStorageResource<Shader>,
    compute_shaders: &AssetStorageResource<ComputeShader>,
    fonts: Option<&AssetStorageResource<Font>>,
    entity_ids: EntityIdCtx,
    f: impl FnOnce(TransSerializerCtxState) -> R,
) -> R {
    let entries = (
        AssetHandleCtx::Registered(prefabs),
        AssetHandleCtx::Registered(scenes),
        AssetHandleCtx::Registered(textures),
        AssetHandleCtx::Registered(materials),
        AssetHandleCtx::Registered(templates),
        AssetHandleCtx::Registered(meshes),
        AssetHandleCtx::Registered(audio_clips),
        AssetHandleCtx::Registered(shaders),
        AssetHandleCtx::Registered(compute_shaders),
        fonts.map(AssetHandleCtx::Registered),
    );

    let mut context = ContextData::new()
        .with::<EntityIdCtxKey>(&entity_ids)
        .with::<AssetHandleCtxKey<Prefab>>(&entries.0)
        .with::<AssetHandleCtxKey<Scene>>(&entries.1)
        .with::<AssetHandleCtxKey<StandardTexture>>(&entries.2)
        .with::<AssetHandleCtxKey<Material>>(&entries.3)
        .with::<AssetHandleCtxKey<MaterialTemplate>>(&entries.4)
        .with::<AssetHandleCtxKey<StandardMesh>>(&entries.5)
        .with::<AssetHandleCtxKey<AudioClip>>(&entries.6)
        .with::<AssetHandleCtxKey<Shader>>(&entries.7)
        .with::<AssetHandleCtxKey<ComputeShader>>(&entries.8);

    if let Some(fonts) = &entries.9 {
        context.insert::<AssetHandleCtxKey<Font>>(fonts);
    }

    f(serializers.to_ctx_state(&context))
}
