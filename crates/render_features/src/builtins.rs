use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::{ResourcesHolderRef, WorldDataMut};
use aphyn_ffi::FfiOption;
use aphyn_math::Vec4;
use aphyn_render::{
    Material, MaterialData, MaterialDataValue, MaterialInput, MaterialPassTarget, MaterialTemplate, MaterialTemplatePass,
    MaterialTextureBinding, MaterialTextureFallback, MaterialValueBinding, RenderApiResource, STANDARD_SHADER_WGSL, Shader,
    WgslComposer,
};
use aphyn_render_core::{
    GpuBlendComponent, GpuBlendFactor, GpuBlendOperation, GpuBlendState, GpuColorWrites, GpuCompareFunction, GpuCullMode,
    GpuFrontFace, GpuPrimitiveTopology, GpuShaderDesc,
};

use crate::{RENDER_ORDER_OPAQUE, RENDER_ORDER_TRANSPARENT};

pub const BUILTIN_TEMPLATE_OPAQUE_UNLIT: &str = "aphyn/templates/opaque_unlit";
pub const BUILTIN_TEMPLATE_OPAQUE_LIT: &str = "aphyn/templates/opaque_lit";
pub const BUILTIN_TEMPLATE_TRANSPARENT_UNLIT: &str = "aphyn/templates/transparent_unlit";
pub const BUILTIN_TEMPLATE_TRANSPARENT_LIT: &str = "aphyn/templates/transparent_lit";
pub const BUILTIN_TEMPLATE_UI_OPAQUE_UNLIT: &str = "aphyn/templates/ui_opaque_unlit";
pub const BUILTIN_TEMPLATE_UI_TRANSPARENT_UNLIT: &str = "aphyn/templates/ui_transparent_unlit";

struct StandardVariant {
    name: &'static str,
    is_lit: bool,
    is_transparent: bool,
    is_ui: bool,
}

const STANDARD_VARIANTS: [StandardVariant; 6] = [
    StandardVariant {
        name: "opaque_unlit",
        is_lit: false,
        is_transparent: false,
        is_ui: false,
    },
    StandardVariant {
        name: "opaque_lit",
        is_lit: true,
        is_transparent: false,
        is_ui: false,
    },
    StandardVariant {
        name: "transparent_unlit",
        is_lit: false,
        is_transparent: true,
        is_ui: false,
    },
    StandardVariant {
        name: "transparent_lit",
        is_lit: true,
        is_transparent: true,
        is_ui: false,
    },
    StandardVariant {
        name: "ui_opaque_unlit",
        is_lit: false,
        is_transparent: false,
        is_ui: true,
    },
    StandardVariant {
        name: "ui_transparent_unlit",
        is_lit: false,
        is_transparent: true,
        is_ui: true,
    },
];

pub fn standard_shader_desc(is_transparent: bool) -> GpuShaderDesc {
    let blend = match is_transparent {
        false => GpuBlendState::REPLACE,
        true => GpuBlendState {
            color: GpuBlendComponent {
                src_factor: GpuBlendFactor::SrcAlpha,
                dst_factor: GpuBlendFactor::One,
                operation: GpuBlendOperation::Add,
            },
            alpha: GpuBlendComponent {
                src_factor: GpuBlendFactor::One,
                dst_factor: GpuBlendFactor::One,
                operation: GpuBlendOperation::Add,
            },
        },
    };

    GpuShaderDesc {
        topology: GpuPrimitiveTopology::TriangleList,
        front_face: GpuFrontFace::Ccw,
        cull_mode: GpuCullMode::Back,
        blend,
        write_mask: GpuColorWrites::All,
        depth_write: !is_transparent,
        depth_compare: GpuCompareFunction::LessEqual,
    }
}

// binding indices follow `standard_bindings.wgsl`
pub fn standard_material_template(shader: AssetHandle<Shader>, is_transparent: bool) -> MaterialTemplate {
    let mut template = MaterialTemplate::default();

    let values = [
        ("color", MaterialDataValue::Vec4(Vec4::splat(1.0))),
        ("emission_color", MaterialDataValue::Vec4(Vec4::splat(0.0))),
        ("metallic", MaterialDataValue::F32(0.0)),
        ("roughness", MaterialDataValue::F32(0.5)),
        ("alpha_threshold", MaterialDataValue::F32(0.5)),
    ];
    let textures = [
        ("color_tex", 1, MaterialTextureFallback::White),
        ("roughness_tex", 3, MaterialTextureFallback::White),
        ("metallic_tex", 5, MaterialTextureFallback::White),
        ("normal_tex", 7, MaterialTextureFallback::FlatNormal),
        ("emission_tex", 9, MaterialTextureFallback::White),
    ];

    let mut pass = MaterialTemplatePass {
        shader,
        order: match is_transparent {
            false => RENDER_ORDER_OPAQUE,
            true => RENDER_ORDER_TRANSPARENT,
        },
        target: match is_transparent {
            false => MaterialPassTarget::Main,
            true => MaterialPassTarget::Transparent,
        },
        ..Default::default()
    };

    for (name, default) in values {
        template.inputs.insert(name.into(), MaterialInput::new(default));
        pass.values.push(MaterialValueBinding {
            input: name.into(),
            group: 1,
            binding: 0,
            member: name.into(),
        });
    }

    for (name, binding, fallback) in textures {
        template.inputs.insert(name.into(), MaterialInput::texture(fallback));
        pass.textures.push(MaterialTextureBinding {
            input: name.into(),
            group: 1,
            texture_binding: binding,
            sampler_binding: FfiOption::Some(binding + 1),
        });
    }

    template.passes.push(pass);

    template
}

pub fn create_material_from_template_key(res: ResourcesHolderRef, template_key: &str, data: MaterialData) -> Option<Material> {
    let render_api = res.get::<RenderApiResource>()?;
    let templates = res.get::<AssetStorageResource<MaterialTemplate>>()?;
    let shaders = res.get::<AssetStorageResource<Shader>>()?;
    let template = templates.get_registered(template_key)?.clone();

    Some(render_api.create_material(templates, shaders, template, data))
}

pub fn insert_builtin_render_assets(mut world: WorldDataMut) {
    let mut res = world.resources_mut();

    let Some(render_api) = res.get::<RenderApiResource>().map(RenderApiResource::clone) else {
        return;
    };

    for variant in &STANDARD_VARIANTS {
        let defines = [
            variant.is_lit.then_some("LIT"),
            variant.is_transparent.then_some("TRANSPARENT"),
            variant.is_ui.then_some("UI"),
        ];
        let defines = defines.into_iter().flatten().collect::<Vec<_>>();

        let shader_key = format!("aphyn/shaders/{}", variant.name);
        let code = WgslComposer::with_engine_libraries()
            .compose("shader_standard.wgsl", STANDARD_SHADER_WGSL, &defines)
            .unwrap();
        let shader = render_api
            .create_shader(&shader_key, &code, standard_shader_desc(variant.is_transparent), None)
            .unwrap();

        let Some(shaders) = res.get_mut::<AssetStorageResource<Shader>>() else {
            return;
        };
        let shader = shaders.insert_and_register(shader, shader_key.into());

        let Some(templates) = res.get_mut::<AssetStorageResource<MaterialTemplate>>() else {
            return;
        };
        templates.insert_and_register(
            standard_material_template(shader, variant.is_transparent),
            format!("aphyn/templates/{}", variant.name).into(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_keys_match_variants() {
        let keys = STANDARD_VARIANTS.map(|variant| format!("aphyn/templates/{}", variant.name));

        assert_eq!(
            keys,
            [
                BUILTIN_TEMPLATE_OPAQUE_UNLIT,
                BUILTIN_TEMPLATE_OPAQUE_LIT,
                BUILTIN_TEMPLATE_TRANSPARENT_UNLIT,
                BUILTIN_TEMPLATE_TRANSPARENT_LIT,
                BUILTIN_TEMPLATE_UI_OPAQUE_UNLIT,
                BUILTIN_TEMPLATE_UI_TRANSPARENT_UNLIT,
            ]
        );
    }

    #[test]
    fn standard_template_maps_every_standard_binding() {
        let template = standard_material_template(AssetHandle::EMPTY, false);
        let pass = &template.passes[0];

        let mut texture_bindings = pass
            .textures
            .iter()
            .flat_map(|texture| [texture.texture_binding, *texture.sampler_binding.as_ref().unwrap()])
            .collect::<Vec<_>>();
        texture_bindings.sort();

        assert_eq!(texture_bindings, (1..=10).collect::<Vec<_>>());
        assert!(pass.values.iter().all(|value| value.group == 1 && value.binding == 0));
        assert_eq!(template.inputs.len(), 10);
    }
}
