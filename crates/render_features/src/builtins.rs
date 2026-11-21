use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::{ResourcesHolderMut, ResourcesHolderRef, WorldDataMut};
use aphyn_ffi::FfiOption;
use aphyn_math::Vec4;
use aphyn_render::{
    Material, MaterialData, MaterialDataValue, MaterialInput, MaterialPassTarget, MaterialTemplate, MaterialTemplatePass,
    MaterialTextureBinding, MaterialTextureFallback, MaterialValueBinding, RenderApiResource, SHADOW_CASTER_SHADER_WGSL,
    STANDARD_SHADER_WGSL, Shader, WgslComposer,
};
use aphyn_render_core::{
    GpuBlendComponent, GpuBlendFactor, GpuBlendOperation, GpuBlendState, GpuColorWrites, GpuCompareFunction, GpuCullMode,
    GpuFrontFace, GpuPrimitiveTopology, GpuShaderDesc,
};

use crate::{RENDER_ORDER_DEPTH_PREPASS, RENDER_ORDER_OPAQUE, RENDER_ORDER_SHADOW, RENDER_ORDER_TRANSPARENT};

pub const BUILTIN_TEMPLATE_OPAQUE_UNLIT: &str = "aphyn/templates/opaque_unlit";
pub const BUILTIN_TEMPLATE_OPAQUE_LIT: &str = "aphyn/templates/opaque_lit";
pub const BUILTIN_TEMPLATE_TRANSPARENT_UNLIT: &str = "aphyn/templates/transparent_unlit";
pub const BUILTIN_TEMPLATE_TRANSPARENT_LIT: &str = "aphyn/templates/transparent_lit";
pub const BUILTIN_TEMPLATE_UI_OPAQUE_UNLIT: &str = "aphyn/templates/ui_opaque_unlit";
pub const BUILTIN_TEMPLATE_UI_TRANSPARENT_UNLIT: &str = "aphyn/templates/ui_transparent_unlit";
pub const BUILTIN_SHADER_SHADOW_CASTER: &str = "aphyn/shaders/shadow_caster";
pub const BUILTIN_SHADER_DEPTH_NORMALS: &str = "aphyn/shaders/depth_normals";

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
        depth_compare: GpuCompareFunction::GreaterEqual,
    }
}

pub fn shadow_caster_shader_desc() -> GpuShaderDesc {
    GpuShaderDesc {
        depth_write: true,
        ..standard_shader_desc(false)
    }
}

pub fn shadow_caster_template_pass(shadow_caster: AssetHandle<Shader>) -> MaterialTemplatePass {
    MaterialTemplatePass {
        shader: shadow_caster,
        order: RENDER_ORDER_SHADOW,
        target: MaterialPassTarget::Shadow,
        ..Default::default()
    }
}

const STANDARD_VALUES: [(&str, MaterialDataValue); 5] = [
    ("color", MaterialDataValue::Vec4(Vec4::splat(1.0))),
    ("emission_color", MaterialDataValue::Vec4(Vec4::splat(0.0))),
    ("metallic", MaterialDataValue::F32(0.0)),
    ("roughness", MaterialDataValue::F32(0.5)),
    ("alpha_threshold", MaterialDataValue::F32(0.5)),
];

// binding indices follow `standard_bindings.wgsl`
const STANDARD_TEXTURES: [(&str, usize, MaterialTextureFallback); 5] = [
    ("color_tex", 1, MaterialTextureFallback::White),
    ("roughness_tex", 3, MaterialTextureFallback::White),
    ("metallic_tex", 5, MaterialTextureFallback::White),
    ("normal_tex", 7, MaterialTextureFallback::FlatNormal),
    ("emission_tex", 9, MaterialTextureFallback::White),
];

fn standard_template_pass(shader: AssetHandle<Shader>, order: u64, target: MaterialPassTarget) -> MaterialTemplatePass {
    let mut pass = MaterialTemplatePass {
        shader,
        order,
        target,
        ..Default::default()
    };

    for (name, _) in STANDARD_VALUES {
        pass.values.push(MaterialValueBinding {
            input: name.into(),
            group: 1,
            binding: 0,
            member: name.into(),
        });
    }

    for (name, binding, _) in STANDARD_TEXTURES {
        pass.textures.push(MaterialTextureBinding {
            input: name.into(),
            group: 1,
            texture_binding: binding,
            sampler_binding: FfiOption::Some(binding + 1),
        });
    }

    pass
}

// maps the standard inputs, like `standard_material_template`
pub fn depth_normals_template_pass(depth_normals: AssetHandle<Shader>) -> MaterialTemplatePass {
    standard_template_pass(depth_normals, RENDER_ORDER_DEPTH_PREPASS, MaterialPassTarget::DepthNormals)
}

pub fn standard_material_template(
    shader: AssetHandle<Shader>,
    is_transparent: bool,
    shadow_caster: Option<AssetHandle<Shader>>,
    depth_normals: Option<AssetHandle<Shader>>,
) -> MaterialTemplate {
    let mut template = MaterialTemplate::default();

    for (name, default) in STANDARD_VALUES {
        template.inputs.insert(name.into(), MaterialInput::new(default));
    }

    for (name, _, fallback) in STANDARD_TEXTURES {
        template.inputs.insert(name.into(), MaterialInput::texture(fallback));
    }

    let (order, target) = match is_transparent {
        false => (RENDER_ORDER_OPAQUE, MaterialPassTarget::Main),
        true => (RENDER_ORDER_TRANSPARENT, MaterialPassTarget::Transparent),
    };
    template.passes.push(standard_template_pass(shader, order, target));

    if let Some(shadow_caster) = shadow_caster {
        template.passes.push(shadow_caster_template_pass(shadow_caster));
    }

    if let Some(depth_normals) = depth_normals {
        template.passes.push(depth_normals_template_pass(depth_normals));
    }

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
    insert_builtin_render_assets_to(world.resources_mut());
}

pub fn insert_builtin_render_assets_to(mut res: ResourcesHolderMut) {
    let Some(render_api) = res.get::<RenderApiResource>().map(RenderApiResource::clone) else {
        return;
    };

    let Some(shadow_caster) = register_builtin_shader(
        &mut res,
        &render_api,
        BUILTIN_SHADER_SHADOW_CASTER,
        ("shader_shadow_caster.wgsl", SHADOW_CASTER_SHADER_WGSL, &[]),
        shadow_caster_shader_desc(),
    ) else {
        return;
    };
    let Some(depth_normals) = register_builtin_shader(
        &mut res,
        &render_api,
        BUILTIN_SHADER_DEPTH_NORMALS,
        ("shader_standard.wgsl", STANDARD_SHADER_WGSL, &["DEPTH_NORMALS"]),
        standard_shader_desc(false),
    ) else {
        return;
    };

    for variant in &STANDARD_VARIANTS {
        let template_key = format!("aphyn/templates/{}", variant.name);

        // re-registering would orphan the handles that loaded assets already hold
        if res
            .get::<AssetStorageResource<MaterialTemplate>>()
            .is_some_and(|templates| templates.get_registered(&template_key).is_some())
        {
            continue;
        }

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
        let is_opaque_world = !variant.is_transparent && !variant.is_ui;
        let mut template = standard_material_template(
            shader,
            variant.is_transparent,
            is_opaque_world.then(|| shadow_caster.clone()),
            is_opaque_world.then(|| depth_normals.clone()),
        );
        // ui geometry is in window pixels, so the world frusta don't apply
        template.skip_frustum_culling = variant.is_ui;
        templates.insert_and_register(template, template_key.into());
    }
}

fn register_builtin_shader(
    res: &mut ResourcesHolderMut,
    render_api: &RenderApiResource,
    key: &str,
    (file, source, defines): (&str, &str, &[&str]),
    desc: GpuShaderDesc,
) -> Option<AssetHandle<Shader>> {
    let shaders = res.get_mut::<AssetStorageResource<Shader>>()?;

    if let Some(handle) = shaders.get_registered(key).cloned() {
        return Some(handle);
    }

    let code = WgslComposer::with_engine_libraries().compose(file, source, defines).unwrap();
    let shader = render_api.create_shader(key, &code, desc, None).unwrap();

    Some(shaders.insert_and_register(shader, key.into()))
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
        let template = standard_material_template(AssetHandle::EMPTY, false, None, None);
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

    #[test]
    fn shadow_caster_pass_is_appended() {
        let caster = AssetHandle::<Shader>::EMPTY;
        let template = standard_material_template(AssetHandle::EMPTY, false, Some(caster.clone()), None);

        assert_eq!(template.passes.len(), 2);
        let pass = &template.passes[1];
        assert_eq!(pass.target, MaterialPassTarget::Shadow);
        assert_eq!(pass.order, RENDER_ORDER_SHADOW);
        assert!(pass.values.is_empty() && pass.textures.is_empty());
    }

    #[test]
    fn depth_normals_pass_maps_like_the_standard_pass() {
        let template = standard_material_template(AssetHandle::EMPTY, false, None, Some(AssetHandle::EMPTY));

        assert_eq!(template.passes.len(), 2);
        let (main, prepass) = (&template.passes[0], &template.passes[1]);
        assert_eq!(prepass.target, MaterialPassTarget::DepthNormals);
        assert_eq!(prepass.order, RENDER_ORDER_DEPTH_PREPASS);
        assert_eq!((&prepass.values, &prepass.textures), (&main.values, &main.textures));
    }
}
