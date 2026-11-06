use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::ResourcesHolderMut;
use aphyn_render::Material;
use aphyn_serialization::{SerializedValue, SerializerCtx, SerializerCtxState};

pub struct MaterialHandleLoader<'a> {
    pub materials: &'a mut AssetStorageResource<Material>,
}

impl<'a> MaterialHandleLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(unsafe {
            Self {
                materials: &mut *res.get_ptr::<AssetStorageResource<Material>>()?,
            }
        })
    }
}

//

pub struct MaterialLoader;

impl MaterialLoader {
    pub fn load_from_serialized<S: Copy + SerializerCtxState<Material>>(
        &mut self,
        mut ctx: SerializerCtx<S>,
        value: &SerializedValue,
    ) -> Option<Material> {
        let material = ctx.deserialize_default::<Material>("", value);

        self.load_from_deserialized(material)
    }

    pub fn load_from_deserialized(&mut self, material: Material) -> Option<Material> {
        Some(material)
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Mutex};

    use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
    use aphyn_audio::AudioClip;
    use aphyn_ffi::FfiString;
    use aphyn_math::Vec4;
    use aphyn_prefab::{Prefab, Scene};
    use aphyn_render::{
        Material, MaterialData, MaterialDataEntry, MaterialDataValue, StandardMaterial, StandardMesh, StandardShader,
        StandardTexture,
    };
    use aphyn_serialization::*;

    use crate::{
        DirectDeserializedAsset, DirectSerializableAsset, MaterialLoader, SingleLoadTransSerializer,
        material_direct_load_serializer, save_load_with_asset_serializers,
    };

    const KEY: &str = "materials/red.asset";

    fn global_serializers() -> TransSerializerRegistry<'static> {
        let mut registry = TransSerializerRegistry::new();
        registry.register(StandardSerializer::<Material>::default());
        registry.register(StandardSerializer::<MaterialData>::default());
        registry.register(StandardSerializer::<MaterialDataEntry>::default());
        registry.register(StandardSerializer::<MaterialDataValue>::default());
        registry.register(StandardSerializer::<FfiString>::default());
        registry.register(StandardSerializer::<f32>::default());
        registry.register(StandardSerializer::<Vec4<f32>>::default());
        registry
    }

    fn red_material() -> Material {
        let mut data = MaterialData::new();
        data.insert("color", MaterialDataValue::Vec4(Vec4::new(1.0, 0.0, 0.0, 1.0)));
        data.insert("color_tex", MaterialDataValue::Texture(AssetHandle::EMPTY));
        Material {
            template: "unlit_color".into(),
            data,
        }
    }

    fn save_material(
        global: &TransSerializerRegistry,
        materials: &AssetStorageResource<Material>,
        textures: &AssetStorageResource<StandardTexture>,
    ) -> SerializedValue {
        save_load_with_asset_serializers(
            &AssetStorageResource::<Prefab>::new(),
            &AssetStorageResource::<Scene>::new(),
            textures,
            &AssetStorageResource::<StandardMaterial>::new(),
            materials,
            &AssetStorageResource::<StandardMesh>::new(),
            &AssetStorageResource::<AudioClip>::new(),
            &AssetStorageResource::<StandardShader>::new(),
            |local| {
                let mut errors = Vec::new();
                let mut err_handler = |err| errors.push(format!("{err}"));
                let state = global.to_ctx_state();
                let mut ctx = state.wrap_with_local(&local).into_ctx(&mut err_handler);
                let saved = ctx.serialize(&DirectSerializableAsset::<Material>::Key(KEY.into()), "");
                drop(ctx);
                assert!(errors.is_empty(), "{errors:?}");
                saved
            },
        )
    }

    #[test]
    fn material_saves_and_loads_back() {
        let mut materials = AssetStorageResource::<Material>::new();
        materials.insert_and_register(red_material(), KEY.into());
        let textures = AssetStorageResource::<StandardTexture>::new();

        let global = global_serializers();
        let saved = save_material(&global, &materials, &textures);

        let json = saved.to_json();
        assert_eq!(json["template"], "unlit_color");
        assert_eq!(
            json["data"]["color"],
            serde_json::json!({ "$enum_variant": "Vec4", "0": { "x": 1.0, "y": 0.0, "z": 0.0, "w": 1.0 } })
        );
        assert_eq!(
            json["data"]["color_tex"],
            serde_json::json!({ "$enum_variant": "Texture", "0": "" })
        );

        let loaded = save_load_with_asset_serializers(
            &AssetStorageResource::<Prefab>::new(),
            &AssetStorageResource::<Scene>::new(),
            &textures,
            &AssetStorageResource::<StandardMaterial>::new(),
            &materials,
            &AssetStorageResource::<StandardMesh>::new(),
            &AssetStorageResource::<AudioClip>::new(),
            &AssetStorageResource::<StandardShader>::new(),
            |local| {
                let mut errors = Vec::new();
                let mut err_handler = |err| errors.push(format!("{err}"));
                let state = global.to_ctx_state();
                let ctx = state.wrap_with_local(&local).into_ctx(&mut err_handler);
                let loaded = MaterialLoader.load_from_serialized(ctx, &saved).unwrap();
                assert!(errors.is_empty(), "{errors:?}");
                loaded
            },
        );

        assert_eq!(loaded.template.as_str(), "unlit_color");
        assert_eq!(loaded.data.get_vec4("color"), Some(&Vec4::new(1.0, 0.0, 0.0, 1.0)));
        assert_eq!(loaded.data.get_texture("color_tex"), Some(&AssetHandle::EMPTY));
    }

    #[test]
    fn reloading_under_the_same_key_carries_snapshots() {
        let mut materials = AssetStorageResource::<Material>::new();
        let handle = materials.insert_and_register(red_material(), KEY.into());
        let mut textures = AssetStorageResource::<StandardTexture>::new();

        let global = global_serializers();
        let saved = save_material(&global, &materials, &textures);

        {
            let materials = Mutex::new(&mut materials);
            let textures = Mutex::new(&mut textures);
            let mut local = TransSerializerRegistry::new();
            local.register(SingleLoadTransSerializer { assets: &textures });
            local.register(material_direct_load_serializer(&materials, Path::new(""), KEY));

            let mut errors = Vec::new();
            let mut err_handler = |err| errors.push(format!("{err}"));
            let state = global.to_ctx_state();
            let mut ctx = state.wrap_with_local(&local).into_ctx(&mut err_handler);
            _ = ctx.deserialize_default::<DirectDeserializedAsset<Material>>("", &saved);
            drop(ctx);
            assert!(errors.is_empty(), "{errors:?}");
        }

        assert_eq!(materials.get_registered(KEY), Some(&handle));
        let reloaded = &materials.get(&handle).unwrap().data;
        assert_eq!(reloaded.get_vec4("color"), Some(&Vec4::new(1.0, 0.0, 0.0, 1.0)));
        assert_eq!(reloaded.entry("color").unwrap().snapshot(), 2);
        assert_eq!(reloaded.entry("color_tex").unwrap().snapshot(), 2);
    }
}
