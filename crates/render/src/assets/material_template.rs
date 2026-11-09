use aphyn_asset_storage::AssetHandle;
use aphyn_ffi::{FfiIndexMap, FfiOption, FfiString, FfiVec};
use aphyn_serialization::*;

use crate::{MaterialDataValue, Shader};

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash, Serializable)]
pub enum MaterialTextureFallback {
    #[default]
    White,
    Black,
    FlatNormal,
}

#[repr(C)]
#[derive(Default, Clone, Debug, PartialEq, Serializable)]
pub struct MaterialInput {
    pub default: MaterialDataValue,
    pub texture_fallback: MaterialTextureFallback,
}

impl MaterialInput {
    pub fn new(default: MaterialDataValue) -> Self {
        Self {
            default,
            texture_fallback: MaterialTextureFallback::White,
        }
    }

    pub fn texture(fallback: MaterialTextureFallback) -> Self {
        Self {
            default: MaterialDataValue::Texture(AssetHandle::EMPTY),
            texture_fallback: fallback,
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serializable)]
pub enum MaterialPassTarget {
    #[default]
    Main,
    Transparent,
}

#[repr(C)]
#[derive(Default, Clone, Debug, PartialEq, Serializable)]
pub struct MaterialValueBinding {
    pub input: FfiString,
    pub group: usize,
    pub binding: usize,
    pub member: FfiString,
}

#[repr(C)]
#[derive(Default, Clone, Debug, PartialEq, Serializable)]
pub struct MaterialTextureBinding {
    pub input: FfiString,
    pub group: usize,
    pub texture_binding: usize,
    pub sampler_binding: FfiOption<usize>,
}

#[repr(C)]
#[derive(Default, Clone, Debug, PartialEq, Serializable)]
pub struct MaterialTemplatePass {
    pub shader: AssetHandle<Shader>,
    pub order: u64,
    pub target: MaterialPassTarget,
    pub values: FfiVec<MaterialValueBinding>,
    pub textures: FfiVec<MaterialTextureBinding>,
}

#[repr(C)]
#[derive(Default, Clone, Debug, Serializable)]
pub struct MaterialTemplate {
    pub inputs: FfiIndexMap<FfiString, MaterialInput>,
    pub passes: FfiVec<MaterialTemplatePass>,
}

impl MaterialTemplate {
    pub fn input(&self, name: &str) -> Option<&MaterialInput> {
        self.inputs.get(name)
    }
}
