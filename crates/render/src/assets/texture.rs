use std::fmt::Debug;

use aphyn_ffi::{FfiOption, FfiString};
use aphyn_render_core::{
    GpuBinding, GpuPassOps, GpuRenderPass, GpuRenderPassTarget, GpuResourceId, GpuSampler, GpuShader, GpuTexture, GpuTextureFormat,
    RenderApi,
};
use aphyn_serialization::*;

pub use aphyn_render_core::GpuFilterMode as FilterMode;

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Serializable)]
pub enum StandardTextureFormat {
    Rgba8Unorm,
    #[default]
    Rgba8UnormSrgb,
    Rgba16Float,
    Depth32Float,
}

impl StandardTextureFormat {
    fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba8Unorm | Self::Rgba8UnormSrgb | Self::Depth32Float => 4,
            Self::Rgba16Float => 8,
        }
    }
}

impl From<StandardTextureFormat> for GpuTextureFormat {
    fn from(value: StandardTextureFormat) -> Self {
        match value {
            StandardTextureFormat::Rgba8Unorm => GpuTextureFormat::Rgba8Unorm,
            StandardTextureFormat::Rgba8UnormSrgb => GpuTextureFormat::Rgba8UnormSrgb,
            StandardTextureFormat::Rgba16Float => GpuTextureFormat::Rgba16Float,
            StandardTextureFormat::Depth32Float => GpuTextureFormat::Depth32Float,
        }
    }
}

#[repr(C)]
#[derive(Serializable, Clone)]
pub struct StandardTextureAssetMetadata {
    pub raw_texture: FfiString,
    pub format: StandardTextureFormat,
    pub should_generate_mipmaps: bool,
}

impl Default for StandardTextureAssetMetadata {
    fn default() -> Self {
        Self {
            raw_texture: FfiString::new(),
            format: StandardTextureFormat::default(),
            should_generate_mipmaps: false,
        }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct StandardTexture {
    texture: GpuTexture,
    sampler: GpuSampler,
    meta: FfiOption<StandardTextureAssetMetadata>,
}

impl StandardTexture {
    pub(crate) fn new(
        core: &RenderApi,
        mip_fill_shader: &GpuShader,
        mip_fill_sampler: &GpuSampler,
        filter_mode: FilterMode,
        dimensions: [u32; 2],
        data: &[u8],
        meta: StandardTextureAssetMetadata,
    ) -> Self {
        let dimensions = dimensions.map(|x| x.max(1) as usize);

        let is_depth = meta.format == StandardTextureFormat::Depth32Float;
        let data = if is_depth { &[] } else { data };

        let format_bytes_per_pixel = meta.format.bytes_per_pixel();
        let px_count = dimensions[0] * dimensions[1];
        let bytes_per_pixel = data.len() / px_count;

        let mut data = data;
        let mut data_vec = Vec::new();

        if !data.is_empty() && format_bytes_per_pixel == 4 && bytes_per_pixel < 4 {
            for i in 0..px_count {
                let mut px = [0, 0, 0, 255];

                for j in 0..bytes_per_pixel {
                    px[j] = data[bytes_per_pixel * i + j];
                }

                data_vec.extend_from_slice(&px);
            }

            data = data_vec.as_slice();
        }

        let mip_level_count = match meta.should_generate_mipmaps && !is_depth {
            false => 1,
            true => dimensions[0].max(dimensions[1]).ilog2() as usize + 1,
        };

        let label = Some(meta.raw_texture.as_str()).filter(|label| !label.is_empty());
        let texture = core.create_texture(label, dimensions, meta.format.into(), mip_level_count);

        if !data.is_empty() {
            core.write_texture(&texture, 0, data);
        }

        let sampler = core.create_sampler(label, filter_mode);

        for mip_level in 1..mip_level_count {
            core.draw(
                &GpuRenderPass {
                    shader: mip_fill_shader,
                    target: Some(GpuRenderPassTarget {
                        view: &texture,
                        mip_level,
                        ops: GpuPassOps::Load,
                    }),
                    depth: None,
                    bind_groups: &[&[
                        (
                            0,
                            GpuBinding::Texture {
                                texture: &texture,
                                mip_level: Some(mip_level - 1),
                            },
                        ),
                        (1, GpuBinding::Sampler(mip_fill_sampler)),
                    ]],
                    vertex_buffers: &[],
                    viewport: None,
                    scissor: None,
                },
                0..3,
                (mip_level - 1)..mip_level,
            );
        }

        Self {
            texture,
            sampler,
            meta: meta.into(),
        }
    }

    pub fn texture(&self) -> &GpuTexture {
        &self.texture
    }

    pub fn sampler(&self) -> &GpuSampler {
        &self.sampler
    }

    pub fn meta(&self) -> Option<&StandardTextureAssetMetadata> {
        self.meta.as_ref()
    }

    pub fn size(&self) -> [u32; 2] {
        self.texture.size().map(|x| x as u32)
    }

    pub fn id(&self) -> GpuResourceId {
        self.texture.id()
    }
}

impl Debug for StandardTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandardTexture").field("id", &self.id()).finish()
    }
}
