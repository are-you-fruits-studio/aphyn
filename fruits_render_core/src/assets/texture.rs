use std::fmt::Debug;

use fruits_ffi::{FfiDroppable, FfiOption, FfiString};
use fruits_serialization::*;
use wgpu::{
    AddressMode, Extent3d, Origin3d, Sampler, SamplerDescriptor, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

use crate::{CreateBindGroupEntry, RenderApi, RenderData};

pub use wgpu::FilterMode;

#[derive(Debug)]
pub struct StandardTextureNative {
    pub texture: Texture,
    pub sampler: Sampler,
}

#[repr(C)]
#[derive(Serializable, Clone)]
pub struct StandardTextureAssetMetadata {
    pub raw_texture: FfiString,
    pub is_linear: bool,
    pub should_generate_mipmaps: bool,
}

impl Default for StandardTextureAssetMetadata {
    fn default() -> Self {
        Self {
            raw_texture: FfiString::new(),
            is_linear: false,
            should_generate_mipmaps: false,
        }
    }
}

#[repr(C)]
pub struct StandardTexture {
    native: FfiDroppable,
    meta: FfiOption<StandardTextureAssetMetadata>,
}

impl StandardTexture {
    pub(crate) fn new(
        render_api: &RenderApi,
        render_data: &RenderData,
        filter_mode: FilterMode,
        mut dimensions: [u32; 2],
        data: &[u8],
        meta: StandardTextureAssetMetadata,
    ) -> Self {
        dimensions = dimensions.map(|x| x.max(1));

        let bytes_per_pixel = 4;
        let bytes_per_row = dimensions[0] * bytes_per_pixel;

        let px_count = (dimensions[0] * dimensions[1]) as usize;

        let bytes_per_pixel = data.len() / px_count;

        let mut data = data;
        let mut data_vec = Vec::new();

        if bytes_per_pixel < 4 {
            for i in 0..px_count {
                let mut px = [0, 0, 0, 255];

                for j in 0..bytes_per_pixel {
                    px[j] = data[bytes_per_pixel * i + j];
                }

                data_vec.extend_from_slice(&px);
            }

            data = data_vec.as_slice();
        }

        let format = match meta.is_linear {
            true => TextureFormat::Rgba8Unorm,
            false => TextureFormat::Rgba8UnormSrgb,
        };

        let mipmaps_count = match meta.should_generate_mipmaps {
            false => 1,
            true => dimensions[0].max(dimensions[1]).ilog2() + 1
        };

        let texture = render_api.device.create_texture(&TextureDescriptor {
            label: Some(meta.raw_texture.as_str()),
            size: Extent3d {
                width: dimensions[0],
                height: dimensions[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: mipmaps_count,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: format,
            usage: TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[format],
        });
        render_api.queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            Extent3d {
                depth_or_array_layers: 1,
                width: dimensions[0],
                height: dimensions[1],
            },
        );

        let sampler = render_api.device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: filter_mode,
            min_filter: filter_mode,
            mipmap_filter: filter_mode,
            ..Default::default()
        });

        for mipmap_level_src in 0..(mipmaps_count - 1) {
            let mipmap_level_dst = mipmap_level_src + 1;

            let texture_view_src = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Mipmap Fill Src Texture View"),
                base_mip_level: mipmap_level_src,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let texture_view_dst = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Mipmap Fill Dst Texture View"),
                base_mip_level: mipmap_level_dst,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let bind_group = crate::create_bind_group(
                &render_api.device,
                Some("Mipmap Fill Bind Group"),
                &render_data.mip_fill_bind_group_layout,
                &[
                    CreateBindGroupEntry::Texture(&texture_view_src),
                    CreateBindGroupEntry::Sampler(&render_data.mip_fill_sampler),
                ],
            );

            let render_pipeline = crate::create_render_pipeline(
                &render_api.device,
                Some("Mipmap Fill Render Pipeline"),
                &render_data.mip_fill_render_pipeline_layout,
                &render_data.mip_fill_shader,
                &[],
                texture.format().into(),
                None,
                wgpu::PrimitiveTopology::TriangleList,
                None,
            );

            let mut encoder = render_api.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Mipmap Fill Command Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Mipmap Fill Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view_dst,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&render_pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.draw(0..3, mipmap_level_src..(mipmap_level_src + 1));
            }

            render_api.queue.submit([encoder.finish()]);
        }

        Self {
            native: FfiDroppable::new(StandardTextureNative {
                texture,
                sampler,
            }),
            meta: meta.into(),
        }
    }

    pub unsafe fn native(&self) -> &StandardTextureNative {
        unsafe { &*(self.native.get() as *const StandardTextureNative) }
    }

    pub fn meta(&self) -> Option<&StandardTextureAssetMetadata> {
        self.meta.as_ref()
    }
}

impl Debug for StandardTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandardTexture").field("native", &self.native.get()).finish()
    }
}

unsafe impl Send for StandardTexture where StandardTextureNative: Send {}
unsafe impl Sync for StandardTexture where StandardTextureNative: Sync {}
