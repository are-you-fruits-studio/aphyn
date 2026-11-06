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
    use aphyn_asset_storage::AssetStorageResource;
    use aphyn_audio::AudioClip;
    use aphyn_ffi::FfiString;
    use aphyn_math::Vec4;
    use aphyn_prefab::{Prefab, Scene};
    use aphyn_render::{
        Material, MaterialData, MaterialDataEntry, MaterialDataValue, StandardMaterial, StandardMesh, StandardShader,
        StandardTexture,
    };
    use aphyn_serialization::*;

    use crate::{DirectSerializableAsset, MaterialLoader, save_load_with_asset_serializers};

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

    #[test]
    fn material_saves_and_loads_back() {
        let original = {
            let mut data = MaterialData::new();
            data.insert("color", MaterialDataValue::Vec4(Vec4::new(1.0, 0.0, 0.0, 1.0)));
            Material {
                template: "unlit_color".into(),
                data,
            }
        };

        let mut materials = AssetStorageResource::<Material>::new();
        materials.insert_and_register(original.clone(), "materials/red.asset".into());

        let global = global_serializers();

        let saved = save_load_with_asset_serializers(
            &AssetStorageResource::<Prefab>::new(),
            &AssetStorageResource::<Scene>::new(),
            &AssetStorageResource::<StandardTexture>::new(),
            &AssetStorageResource::<StandardMaterial>::new(),
            &materials,
            &AssetStorageResource::<StandardMesh>::new(),
            &AssetStorageResource::<AudioClip>::new(),
            &AssetStorageResource::<StandardShader>::new(),
            |local| {
                let mut errors = Vec::new();
                let mut err_handler = |err| errors.push(format!("{err}"));
                let state = global.to_ctx_state();
                let mut ctx = state.wrap_with_local(&local).into_ctx(&mut err_handler);
                let saved = ctx.serialize(&DirectSerializableAsset::<Material>::Key("materials/red.asset".into()), "");
                drop(ctx);
                assert!(errors.is_empty(), "{errors:?}");
                saved
            },
        );

        let json = saved.to_json();
        assert_eq!(json["template"], "unlit_color");
        assert!(
            json["data"]["color"].is_array() || json["data"]["color"].is_object(),
            "{json}"
        );
        assert!(json["data"].as_object().unwrap().contains_key("color"));

        let mut errors = Vec::new();
        let mut err_handler = |err| errors.push(format!("{err}"));
        let state = global.to_ctx_state();
        let ctx = state.into_ctx(&mut err_handler);
        let loaded = MaterialLoader.load_from_serialized(ctx, &saved).unwrap();
        assert!(errors.is_empty(), "{errors:?}");

        assert_eq!(loaded.template.as_str(), "unlit_color");
        assert_eq!(loaded.data.get_vec4("color"), Some(&Vec4::new(1.0, 0.0, 0.0, 1.0)));
    }
}
