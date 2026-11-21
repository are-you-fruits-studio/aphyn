use aphyn_ffi::{FfiOption, FfiString};
use aphyn_reflection::*;
use aphyn_render_core::{GpuComputeShader, RenderApi};
use aphyn_serialization::*;

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Serializable, Reflectable)]
pub struct ComputeShaderAssetMetadata {
    pub wgsl: FfiString,
}

#[repr(C)]
#[derive(Clone)]
pub struct ComputeShader {
    shader: GpuComputeShader,
    meta: FfiOption<ComputeShaderAssetMetadata>,
}

impl ComputeShader {
    pub(crate) fn new(core: &RenderApi, label: &str, composed_wgsl: &str, meta: Option<ComputeShaderAssetMetadata>) -> Option<Self> {
        let shader = core.create_compute_shader(Some(label), composed_wgsl)?;

        Some(Self {
            shader,
            meta: meta.into(),
        })
    }

    pub fn shader(&self) -> &GpuComputeShader {
        &self.shader
    }

    pub fn meta(&self) -> Option<&ComputeShaderAssetMetadata> {
        self.meta.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_roundtrips_through_json() {
        let meta = ComputeShaderAssetMetadata {
            wgsl: "shaders/particles.wgsl".into(),
        };

        let mut on_err = |err: SerializationError| panic!("{err}");
        let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);
        let json = ctx.serialize(&meta, "").to_json();

        assert_eq!(json["wgsl"], "shaders/particles.wgsl");

        let mut restored = ComputeShaderAssetMetadata::default();
        ctx.deserialize(&mut restored, "", &SerializedValue::from_json(&json));
        assert_eq!(restored, meta);
    }
}
