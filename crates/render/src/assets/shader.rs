use aphyn_ffi::{FfiOption, FfiString};
use aphyn_render_core::{GpuShader, GpuShaderDesc, RenderApi};
use aphyn_serialization::*;

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Serializable)]
pub struct ShaderAssetMetadata {
    pub wgsl: FfiString,
    pub desc: GpuShaderDesc,
}

#[repr(C)]
#[derive(Clone)]
pub struct Shader {
    shader: GpuShader,
    meta: FfiOption<ShaderAssetMetadata>,
}

impl Shader {
    pub(crate) fn new(
        core: &RenderApi,
        label: &str,
        composed_wgsl: &str,
        desc: GpuShaderDesc,
        meta: Option<ShaderAssetMetadata>,
    ) -> Option<Self> {
        let shader = core.create_shader(Some(label), composed_wgsl, desc)?;

        Some(Self {
            shader,
            meta: meta.into(),
        })
    }

    pub fn shader(&self) -> &GpuShader {
        &self.shader
    }

    pub fn meta(&self) -> Option<&ShaderAssetMetadata> {
        self.meta.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use aphyn_render_core::{GpuBlendState, GpuCullMode};

    use super::*;

    #[test]
    fn metadata_roundtrips_through_json() {
        let meta = ShaderAssetMetadata {
            wgsl: "shaders/water.wgsl".into(),
            desc: GpuShaderDesc {
                cull_mode: GpuCullMode::None,
                blend: GpuBlendState::ALPHA_BLENDING,
                depth_write: false,
                ..Default::default()
            },
        };

        let mut on_err = |err: SerializationError| panic!("{err}");
        let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);
        let json = ctx.serialize(&meta, "").to_json();

        assert_eq!(json["wgsl"], "shaders/water.wgsl");
        assert_eq!(json["desc"]["cull_mode"], serde_json::json!({ "$enum_variant": "None" }));
        assert_eq!(json["desc"]["depth_write"], false);

        let mut restored = ShaderAssetMetadata::default();
        ctx.deserialize(&mut restored, "", &SerializedValue::from_json(&json));
        assert_eq!(restored, meta);
    }
}
