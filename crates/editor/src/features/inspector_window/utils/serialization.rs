use crate::{
    features::asset_serialization::{InspectedAsset, get_asset_type},
    *,
};

pub fn save_asset_from_world_res(res: ResourcesHolderRef, asset_key: &str) -> Option<SerializedValue> {
    let asset_type = get_asset_type(res, asset_key)?;

    // todo
    let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
    // let mut err_handler = |_| ();

    let inspected_asset = InspectedAsset {
        asset_key: asset_key.into(),
        asset_type,
    };

    let mut serialized = save_load_with_asset_serializers_from_world(res, EntityIdCtx::default(), |serializer_ctx_state| {
        inspected_asset.to_serialized(serializer_ctx_state.into_ctx(&mut err_handler))
    })?;

    enrich_serialized_with_asset_type(&mut serialized, asset_type);

    Some(serialized)
}

pub fn load_asset_to_world_res(
    res: ResourcesHolderMut,
    asset_key: &str,
    value: &SerializedValue,
    asset_type: AssetType,
    assets_dir_path: &str,
) -> bool {
    load_asset_single_from_world(
        res,
        assets_dir_path,
        asset_key,
        EntityIdCtx::default(),
        |serializer_ctx_state| {
            let mut err_handler = |err| println!("{err}");
            let mut serializer_ctx = serializer_ctx_state.into_ctx(&mut err_handler);

            match asset_type {
                AssetType::Texture => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<StandardTexture>>("", value),
                AssetType::Material => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<Material>>("", value),
                AssetType::Mesh => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<StandardMesh>>("", value),
                AssetType::Font => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<Font>>("", value),
                AssetType::AudioClip => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<AudioClip>>("", value),
                AssetType::Prefab => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<Prefab>>("", value),
                AssetType::Scene => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<Scene>>("", value),
                AssetType::Shader => _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<Shader>>("", value),
                AssetType::ComputeShader => {
                    _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<ComputeShader>>("", value)
                }
                AssetType::MaterialTemplate => {
                    _ = serializer_ctx.deserialize_default::<DirectDeserializedAsset<MaterialTemplate>>("", value)
                }
            };
        },
    )
    .is_some()
}

pub fn enrich_serialized_with_asset_type(serialized: &mut SerializedValue, asset_type: AssetType) {
    if let SerializedValue::Composite(serialized) = serialized {
        if let SerializedCompositeValues::Map(serialized) = &mut serialized.values {
            let mut map = FfiIndexMap::new();

            map.insert(
                FfiString::from("asset_type"),
                SerializedValue::Primitive(SerializedPrimitive::String(asset_type.serialized_str().into())),
            );
            for (k, v) in &serialized.values {
                map.insert(k.clone(), v.clone());
            }

            serialized.values = map;
        }
    }
}
