use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::ResourcesHolderMut;
use aphyn_render::{Material, MaterialTemplate, RenderApiResource, Shader};

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

#[derive(Clone, Copy, Default)]
pub struct MaterialLoader<'a> {
    pub render_api: Option<&'a RenderApiResource>,
}

impl<'a> MaterialLoader<'a> {
    pub fn from_world(res: &ResourcesHolderMut<'a>) -> Self {
        Self {
            render_api: res.get_ptr::<RenderApiResource>().map(|render_api| unsafe { &*render_api }),
        }
    }

    pub fn bind(
        &self,
        mut material: Material,
        templates: &AssetStorageResource<MaterialTemplate>,
        shaders: &AssetStorageResource<Shader>,
    ) -> Material {
        match self.render_api {
            Some(render_api) => render_api.rebuild_material_gpu(templates, shaders, &mut material),
            None => {
                if let Some(template) = templates.get(&material.template) {
                    material.data.reconcile(template);
                }
            }
        }

        material
    }

    pub fn reload(
        &self,
        mut material: Material,
        old: Option<&Material>,
        templates: &AssetStorageResource<MaterialTemplate>,
        shaders: &AssetStorageResource<Shader>,
    ) -> Material {
        if let Some(old) = old {
            material.data.carry_snapshots_from(&old.data);
        }

        self.bind(material, templates, shaders)
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
        Material, MaterialData, MaterialDataEntry, MaterialDataValue, MaterialInput, MaterialTemplate, MaterialTextureFallback,
        Shader, StandardMesh, StandardTexture,
    };
    use aphyn_serialization::*;

    use crate::{
        DirectDeserializedAsset, DirectSerializableAsset, MaterialLoader, SingleLoadTransSerializer,
        material_direct_load_serializer, save_load_with_asset_serializers,
    };

    const KEY: &str = "materials/red.asset";
    const TEMPLATE_KEY: &str = "templates/unlit.asset";

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

    fn templates() -> AssetStorageResource<MaterialTemplate> {
        let mut template = MaterialTemplate::default();
        template
            .inputs
            .insert("color".into(), MaterialInput::new(MaterialDataValue::Vec4(Vec4::splat(1.0))));
        template
            .inputs
            .insert("color_tex".into(), MaterialInput::texture(MaterialTextureFallback::White));
        template
            .inputs
            .insert("speed".into(), MaterialInput::new(MaterialDataValue::F32(0.3)));

        let mut templates = AssetStorageResource::new();
        templates.insert_and_register(template, TEMPLATE_KEY.into());
        templates
    }

    fn red_material(templates: &AssetStorageResource<MaterialTemplate>) -> Material {
        let mut data = MaterialData::new();
        data.insert("color", MaterialDataValue::Vec4(Vec4::new(1.0, 0.0, 0.0, 1.0)));
        data.insert("color_tex", MaterialDataValue::Texture(AssetHandle::EMPTY));
        Material::new(templates.get_registered(TEMPLATE_KEY).unwrap().clone(), data)
    }

    fn with_asset_serializers<R>(
        materials: &AssetStorageResource<Material>,
        templates: &AssetStorageResource<MaterialTemplate>,
        textures: &AssetStorageResource<StandardTexture>,
        f: impl FnOnce(TransSerializerRegistry) -> R,
    ) -> R {
        save_load_with_asset_serializers(
            &AssetStorageResource::<Prefab>::new(),
            &AssetStorageResource::<Scene>::new(),
            textures,
            materials,
            templates,
            &AssetStorageResource::<StandardMesh>::new(),
            &AssetStorageResource::<AudioClip>::new(),
            &AssetStorageResource::<Shader>::new(),
            f,
        )
    }

    fn save_material(
        global: &TransSerializerRegistry,
        materials: &AssetStorageResource<Material>,
        templates: &AssetStorageResource<MaterialTemplate>,
        textures: &AssetStorageResource<StandardTexture>,
    ) -> SerializedValue {
        with_asset_serializers(materials, templates, textures, |local| {
            let mut errors = Vec::new();
            let mut err_handler = |err| errors.push(format!("{err}"));
            let state = global.to_ctx_state();
            let mut ctx = state.wrap_with_local(&local).into_ctx(&mut err_handler);
            let saved = ctx.serialize(&DirectSerializableAsset::<Material>::Key(KEY.into()), "");
            drop(ctx);
            assert!(errors.is_empty(), "{errors:?}");
            saved
        })
    }

    #[test]
    fn material_saves_and_loads_back_reconciled() {
        let templates = templates();
        let mut materials = AssetStorageResource::<Material>::new();
        materials.insert_and_register(red_material(&templates), KEY.into());
        let textures = AssetStorageResource::<StandardTexture>::new();

        let global = global_serializers();
        let saved = save_material(&global, &materials, &templates, &textures);

        let json = saved.to_json();
        assert_eq!(json["template"], TEMPLATE_KEY);
        assert_eq!(
            json["data"]["color"],
            serde_json::json!({ "$enum_variant": "Vec4", "0": { "x": 1.0, "y": 0.0, "z": 0.0, "w": 1.0 } })
        );
        assert_eq!(
            json["data"]["color_tex"],
            serde_json::json!({ "$enum_variant": "Texture", "0": "" })
        );

        let loaded = with_asset_serializers(&materials, &templates, &textures, |local| {
            let mut errors = Vec::new();
            let mut err_handler = |err| errors.push(format!("{err}"));
            let state = global.to_ctx_state();
            let mut ctx = state.wrap_with_local(&local).into_ctx(&mut err_handler);
            let material = ctx.deserialize_default::<Material>("", &saved);
            drop(ctx);
            assert!(errors.is_empty(), "{errors:?}");
            MaterialLoader::default().bind(material, &templates, &AssetStorageResource::new())
        });

        assert_eq!(&loaded.template, templates.get_registered(TEMPLATE_KEY).unwrap());
        assert_eq!(loaded.data.get_vec4("color"), Some(&Vec4::new(1.0, 0.0, 0.0, 1.0)));
        assert_eq!(loaded.data.get_texture("color_tex"), Some(&AssetHandle::EMPTY));
        assert_eq!(loaded.data.get_f32("speed"), Some(&0.3));
        assert!(!loaded.gpu().is_built());
    }

    #[test]
    fn reloading_under_the_same_key_carries_snapshots() {
        let mut templates = templates();
        let mut shaders = AssetStorageResource::<Shader>::new();
        let mut materials = AssetStorageResource::<Material>::new();
        let mut textures = AssetStorageResource::<StandardTexture>::new();

        let mut material = red_material(&templates);
        material.data.reconcile(templates.get(&material.template).unwrap());
        let handle = materials.insert_and_register(material, KEY.into());

        let global = global_serializers();
        let saved = save_material(&global, &materials, &templates, &textures);

        {
            let materials = Mutex::new(&mut materials);
            let textures = Mutex::new(&mut textures);
            let templates = Mutex::new(&mut templates);
            let shaders = Mutex::new(&mut shaders);
            let mut local = TransSerializerRegistry::new();
            local.register(SingleLoadTransSerializer { assets: &textures });
            local.register(SingleLoadTransSerializer { assets: &templates });
            local.register(material_direct_load_serializer(
                &materials,
                &templates,
                &shaders,
                MaterialLoader::default(),
                Path::new(""),
                KEY,
            ));

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
        assert_eq!(reloaded.entry("speed").unwrap().snapshot(), 2);
    }
}
