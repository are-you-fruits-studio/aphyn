use crate::*;

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InspectedAsset {
    pub asset_type: AssetType,
    pub asset_key: FfiString,
}

impl InspectedAsset {
    pub(crate) fn to_serialized(&self, mut ctx: SerializerCtx<TransSerializerCtxState>) -> SerializedValue {
        let mut serialized = match self.asset_type {
            AssetType::Texture => ctx.serialize(&DirectSerializableAsset::<StandardTexture>::Key(self.asset_key.clone()), ""),
            AssetType::Material => ctx.serialize(&DirectSerializableAsset::<StandardMaterial>::Key(self.asset_key.clone()), ""),
            AssetType::Mesh => ctx.serialize(&DirectSerializableAsset::<StandardMesh>::Key(self.asset_key.clone()), ""),
            AssetType::AudioClip => ctx.serialize(&DirectSerializableAsset::<AudioClip>::Key(self.asset_key.clone()), ""),
            AssetType::Font => ctx.serialize(&DirectSerializableAsset::<Font>::Key(self.asset_key.clone()), ""),
            AssetType::Prefab => ctx.serialize(&DirectSerializableAsset::<Prefab>::Key(self.asset_key.clone()), ""),
        };

        if let SerializedValue::Composite(serialized_composite) = &mut serialized {
            if let SerializedCompositeValues::Map(serialized_map) = &mut serialized_composite.values {
                let mut map = FfiIndexMap::new();

                map.insert(FfiString::from("asset_type"), SerializedValue::Primitive(SerializedPrimitive::String(self.asset_type.serialized_str().into())));
                for (k, v) in &serialized_map.values {
                    map.insert(k.clone(), v.clone());
                }

                serialized_map.values = map;
            } else {
                eprintln!("Strange InspectedAsset serialization, failed to enrich with asset_type");
            }
        } else {
            eprintln!("Strange InspectedAsset serialization, failed to enrich with asset_type");
        }

        serialized
    }

    // todo
    // pub(crate) fn from_serialized(ctx: SerializerCtx, serialized: &SerializedValue) -> Option<Self> {
    //     let SerializedValue::Composite(serialized_composite) = serialized else {
    //         return None;
    //     };

    //     let SerializedCompositeValues::Map(serialized_values) = &serialized_composite.values else {
    //         return None;
    //     };

    //     let mut asset_type = None;
    //     let mut reserialized_map = SerializedMap::default();
    //     reserialized_map.enum_metadata = serialized_values.enum_metadata.clone();

    //     for (key, serialized_value) in &serialized_values.values {
    //         if key.as_str() == "asset_type" {
    //             if let SerializedValue::Primitive(SerializedPrimitive::String(serialized_value)) = &serialized_value {
    //                 asset_type = Some(serialized_value.clone());
    //             }

    //             continue;
    //         }

    //         reserialized_map.values.insert(key.clone(), serialized_value.clone());
    //     }

    //     let Some(asset_type) = asset_type else {
    //         return None;
    //     };

    //     let serialized_asset = SerializedValue::Composite(SerializedComposite {
    //         is_rigid: serialized_composite.is_rigid,
    //         values: SerializedCompositeValues::Map(reserialized_map)
    //     });

    //     match asset_type.as_str() {
    //         "material" => SerializedMaterial::deserialize(ctx, &serialized_asset).map(Self::Material),
    //         "texture" => SerializedTexture::deserialize(ctx, &serialized_asset).map(Self::Texture),
    //         "audio_clip" => SerializedAudioClip::deserialize(ctx, &serialized_asset).map(Self::AudioClip),
    //         "mesh" => SerializedMesh::deserialize(ctx, &serialized_asset).map(Self::Mesh),
    //         _ => None,
    //     }
    // }
}

pub fn get_asset_type(res: ResourcesHolderRef, key: &str) -> Option<AssetType> {
    if let Some(storage) = res.get::<AssetStorageResource<StandardTexture>>() && storage.get_registered(key).is_some() {
        return Some(AssetType::Texture);
    }
    if let Some(storage) = res.get::<AssetStorageResource<StandardMaterial>>() && storage.get_registered(key).is_some() {
        return Some(AssetType::Material);
    }
    if let Some(storage) = res.get::<AssetStorageResource<StandardMesh>>() && storage.get_registered(key).is_some() {
        return Some(AssetType::Mesh);
    }
    if let Some(storage) = res.get::<AssetStorageResource<Font>>() && storage.get_registered(key).is_some() {
        return Some(AssetType::Font);
    }
    if let Some(storage) = res.get::<AssetStorageResource<AudioClip>>() && storage.get_registered(key).is_some() {
        return Some(AssetType::AudioClip);
    }
    if let Some(storage) = res.get::<AssetStorageResource<Prefab>>() && storage.get_registered(key).is_some() {
        return Some(AssetType::Prefab);
    }

    None
}
