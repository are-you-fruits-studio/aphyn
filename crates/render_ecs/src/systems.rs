use core::f32;
use std::collections::HashMap;

use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::*;
use aphyn_math::*;
use aphyn_render::*;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt}, *,
};

use aphyn_transform::*;
use crate::{utils::*, *};

use super::{
    DepthTextureResource, GizmosRenderResource, GizmosResource, StandardRenderResource,
    components::{StandardMaterialComponent, StandardMeshComponent},
};

// todo: refactor this file's duplications

pub fn create_standard_render_resource(mut world: WorldDataMut) {
    let render_state = unsafe { world.as_ref().resources().get::<RenderApiResource>().unwrap().raw() };

    let bind_group_layout_transparent_final = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Transparent Target Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerNonFiltering,
        ],
    );

    let pipeline_layout_transparent_final = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Transparent Final Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout_transparent_final],
        push_constant_ranges: &[],
    });

    let render_pipeline_transparent_shader = create_engine_shader_module(
        render_state.device(),
        "shader_transparent_final.wgsl",
        include_str!("./assets/shader_transparent_final.wgsl"),
        &[],
    );

    let render_pipeline_transparent_final = create_render_pipeline(
        render_state.device(),
        Some("Transparent Final Render Pipeline"),
        &pipeline_layout_transparent_final,
        &render_pipeline_transparent_shader,
        &[],
        wgpu::ColorTargetState {
            format: RENDER_TARGET_FORMAT,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::COLOR,
        },
        None,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
    );

    let instances = render_state.create_instance_buffer(INSTANCES_PER_DRAW_MAX as u32);

    let identity_instance = render_state.create_instance_buffer(1);
    render_state.write_instances(&identity_instance, 0, &[Mat4::IDENTITY]);

    let batched_vertex_buffer = render_state.device().create_buffer(&BufferDescriptor {
        label: Some("Batch vertex buffer"),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        size: (TRIANGLES_PER_BATCHED_DRAW_MAX * 3 * size_of::<StandardVertex>()) as u64,
        mapped_at_creation: false,
    });

    let globals = render_state.create_global_bindings();

    world.as_mut()
        .resources_mut()
        .insert(StandardRenderResource {
            globals,
            instances,
            identity_instance,
            batched_vertex_buffer,
            lights_count: 0,
            render_pipeline_transparent_final,
            bind_group_layout_transparent_final,
            camera_pos: Vec3::default(),
            camera_proj_matrix: Mat4::IDENTITY,
        });
}

pub fn recreate_depth_texture_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    let render_state = unsafe { render_api.raw() };

    let mut contains_depth = false;

    if let Some(depth_res) = world.as_ref().resources().get::<DepthTextureResource>() {
        contains_depth = true;

        if depth_res.texture.size() == screen_size {
            return;
        }
    }

    let texture = render_state.create_texture(
        FilterMode::Nearest,
        screen_size,
        &[],
        StandardTextureAssetMetadata {
            format: StandardTextureFormat::Depth32Float,
            ..Default::default()
        },
    );

    let depth_res = DepthTextureResource { texture };

    if contains_depth {
        *world.resources_mut().get_mut().unwrap() = depth_res;
    } else {
        world.resources_mut().insert(depth_res);
    }
}

pub fn recreate_transparent_target_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    let render_state = unsafe { render_api.raw() };

    let mut contains_transparent_target = false;

    if let Some(transparent_target_res) = world.as_ref().resources().get::<TransparentTargetTextureResource>() {
        contains_transparent_target = true;

        if transparent_target_res.texture.size() == screen_size {
            return;
        }
    }

    let texture = render_state.create_texture(
        FilterMode::Nearest,
        screen_size,
        &[],
        StandardTextureAssetMetadata {
            format: StandardTextureFormat::Rgba16Float,
            ..Default::default()
        },
    );

    let standard_render_res = world.as_ref().resources().get::<StandardRenderResource>().unwrap();

    let bind_group = create_bind_group(
        render_state.device(),
        Some("Transparent Target Bind Group"),
        &standard_render_res.bind_group_layout_transparent_final,
        &[
            CreateBindGroupEntry::Texture(unsafe { &texture.native().view }),
            CreateBindGroupEntry::Sampler(unsafe { &texture.native().sampler }),
        ],
    );

    let transparent_target_res = TransparentTargetTextureResource {
        texture,
        bind_group,
    };

    if contains_transparent_target {
        *world.resources_mut().get_mut().unwrap() = transparent_target_res;
    } else {
        world.resources_mut().insert(transparent_target_res);
    }
}

pub fn recreate_exposure_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    let render_state = unsafe { render_api.raw() };

    let mut contains_render_res = false;

    if let Some(render_res) = world.as_ref().resources().get::<ExposureRenderResource>() {
        contains_render_res = true;

        let are_same_size = true
            && render_res.texture.size().width == screen_size[0]
            && render_res.texture.size().height == screen_size[1];

        if are_same_size && render_res.target_texture == unsafe { current_rendering.target.native() }.texture {
            return;
        }
    }

    let buffer_uniform = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Exposure Uniform Buffer"),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        contents: aphyn_utils::mem::as_bytes(&0.0_f32),
    });

    let texture = render_state.device().create_texture(&TextureDescriptor {
        label: Some(&format!("Exposure Temp Texture")),
        size: Extent3d {
            width: screen_size[0],
            height: screen_size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&Default::default());

    let sampler = render_state.device().create_sampler(&SamplerDescriptor {
        label: Some("Exposure Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: FilterMode::Nearest,
        compare: None,
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    let bind_group_layout = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Exposure Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerNonFiltering,
            CreateBindGroupLayoutEntry::BufferUniform,
        ],
    );

    let bind_group = create_bind_group(
        render_state.device(),
        Some("Exposure Bind Group"),
        &bind_group_layout,
        &[
            CreateBindGroupEntry::Texture(unsafe { &current_rendering.target.native().view }),
            CreateBindGroupEntry::Sampler(&sampler),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
        ],
    );

    let render_pipeline_layout = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Exposure Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let shader = create_engine_shader_module(
        render_state.device(),
        "shader_exposure.wgsl",
        include_str!("./assets/shader_exposure.wgsl"),
        &[],
    );

    let render_pipeline = create_render_pipeline(
        render_state.device(),
        Some("Exposure Render Pipeline"),
        &render_pipeline_layout,
        &shader,
        &[],
        wgpu::ColorTargetState {
            format: texture.format(),
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        },
        None,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
    );

    let render_res = ExposureRenderResource {
        target_texture: unsafe { current_rendering.target.native() }.texture.clone(),
        buffer_uniform,
        texture,
        texture_view,
        sampler,
        bind_group_layout,
        bind_group,
        render_pipeline,
    };

    if contains_render_res {
        *world.resources_mut().get_mut().unwrap() = render_res;
    } else {
        world.resources_mut().insert(render_res);
    }
}

pub fn recreate_bloom_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    let render_state = unsafe { render_api.raw() };

    let mut contains_transparent_target = false;

    if let Some(render_res) = world.as_ref().resources().get::<BloomRenderResource>() {
        contains_transparent_target = true;

        let mut are_same_size = true;

        for texture in &render_res.textures {
            are_same_size &= texture.size().width == screen_size[0];
            are_same_size &= texture.size().height == screen_size[1];
        }

        if are_same_size && render_res.target_texture == unsafe { current_rendering.target.native() }.texture {
            return;
        }
    }

    let mut blur_layers: [f32; BLUR_LAYERS_COUNT] = [
        8.0,
        4.0,
        2.0,
        1.0,
    ];

    let layers_intensity_sum = blur_layers.into_iter().sum::<f32>();

    blur_layers.iter_mut().for_each(|i| *i /= layers_intensity_sum);

    let storage_layers = blur_layers.into_iter().enumerate().flat_map(|(blur_layer, blur_intensity)| {
        let uv_scale = 1.0 / (1 << blur_layer) as f32;
        let uv_scale_offset = Vec4::<f32>::new(uv_scale, uv_scale, 0.0, 0.0);
        
        [
            BloomStorageLayer {
                intensity: blur_intensity,
                uv_scale_offset: uv_scale_offset,
                direction: Vec2::new(1.0, 0.0),
                _padding0: 0.0,
            },
            BloomStorageLayer {
                intensity: blur_intensity,
                uv_scale_offset: uv_scale_offset,
                direction: Vec2::new(0.0, 1.0),
                _padding0: 0.0,
            },
        ]
    }).collect::<Vec<_>>();

    let buffer_storage_layers = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Bloom Storage Uniform Buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        contents: aphyn_utils::mem::as_bytes_slice(&storage_layers),
    });

    let buffer_uniform = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Bloom Uniform Buffer"),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        contents: aphyn_utils::mem::as_bytes(&BloomUniform::default()),
    });

    let textures = std::array::from_fn::<_, 3, _>(|i| render_state.device().create_texture(&TextureDescriptor {
        label: Some(&format!("Bloom Buffer {i}")),
        size: Extent3d {
            width: screen_size[0],
            height: screen_size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }));

    let sampler_linear = render_state.device().create_sampler(&SamplerDescriptor {
        label: Some("Bloom Gather Threshold Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        compare: None,
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    let sampler_nearest = render_state.device().create_sampler(&SamplerDescriptor {
        label: Some("Bloom Gather Threshold Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: FilterMode::Nearest,
        compare: None,
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    let bind_group_layout_gather = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Bloom Gather Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerNonFiltering,
            CreateBindGroupLayoutEntry::BufferUniform,
            CreateBindGroupLayoutEntry::BufferStorage,
        ],
    );

    let bind_group_gather = create_bind_group(
        render_state.device(),
        Some("Bloom Gather Threshold Bind Group"),
        &bind_group_layout_gather,
        &[
            CreateBindGroupEntry::Texture(unsafe { &current_rendering.target.native().view }),
            CreateBindGroupEntry::Sampler(&sampler_nearest),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_layout_blur = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Bloom Blur Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerNonFiltering,
            CreateBindGroupLayoutEntry::BufferUniform,
            CreateBindGroupLayoutEntry::BufferStorage,
        ],
    );

    let bind_group_blur_horiz_0 = create_bind_group(
        render_state.device(),
        Some("Bloom Blur Horiz 0 Bind Group"),
        &bind_group_layout_blur,
        &[
            CreateBindGroupEntry::Texture(&textures[0].create_view(&Default::default())),
            CreateBindGroupEntry::Sampler(&sampler_nearest),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_blur_horiz_1 = create_bind_group(
        render_state.device(),
        Some("Bloom Blur Horiz 1 Bind Group"),
        &bind_group_layout_blur,
        &[
            CreateBindGroupEntry::Texture(&textures[1].create_view(&Default::default())),
            CreateBindGroupEntry::Sampler(&sampler_nearest),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_blur_vert = create_bind_group(
        render_state.device(),
        Some("Bloom Blur Vert Bind Group"),
        &bind_group_layout_blur,
        &[
            CreateBindGroupEntry::Texture(&textures[2].create_view(&Default::default())),
            CreateBindGroupEntry::Sampler(&sampler_nearest),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_layout_downscale = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Bloom Downscale Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerFiltering,
            CreateBindGroupLayoutEntry::BufferUniform,
            CreateBindGroupLayoutEntry::BufferStorage,
        ],
    );

    let bind_group_downscale_0 = create_bind_group(
        render_state.device(),
        Some("Bloom Downscale 0 Bind Group"),
        &bind_group_layout_downscale,
        &[
            CreateBindGroupEntry::Texture(&textures[0].create_view(&Default::default())),
            CreateBindGroupEntry::Sampler(&sampler_linear),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_downscale_1 = create_bind_group(
        render_state.device(),
        Some("Bloom Downscale 1 Bind Group"),
        &bind_group_layout_downscale,
        &[
            CreateBindGroupEntry::Texture(&textures[1].create_view(&Default::default())),
            CreateBindGroupEntry::Sampler(&sampler_linear),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_layout_apply = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Bloom Apply Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerFiltering,
            CreateBindGroupLayoutEntry::BufferUniform,
            CreateBindGroupLayoutEntry::BufferStorage,
        ],
    );

    let bind_group_apply_0 = create_bind_group(
        render_state.device(),
        Some("Bloom Apply 0 Bind Group"),
        &bind_group_layout_apply,
        &[
            CreateBindGroupEntry::Texture(&textures[0].create_view(&Default::default())),
            CreateBindGroupEntry::Sampler(&sampler_linear),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_apply_1 = create_bind_group(
        render_state.device(),
        Some("Bloom Apply 0 Bind Group"),
        &bind_group_layout_apply,
        &[
            CreateBindGroupEntry::Texture(&textures[1].create_view(&Default::default())),
            CreateBindGroupEntry::Sampler(&sampler_linear),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let render_pipeline_layout_gather = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Bloom Gather Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout_gather],
        push_constant_ranges: &[],
    });

    let render_pipeline_layout_blur = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Bloom Blur Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout_blur],
        push_constant_ranges: &[],
    });

    let render_pipeline_layout_downscale = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Bloom Downscale Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout_downscale],
        push_constant_ranges: &[],
    });

    let render_pipeline_layout_apply = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Bloom Downscale Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout_apply],
        push_constant_ranges: &[],
    });

    let shader_gather = create_engine_shader_module(
        render_state.device(),
        "shader_bloom_gather_threshold.wgsl",
        include_str!("./assets/shader_bloom_gather_threshold.wgsl"),
        &[],
    );

    let shader_blur = create_engine_shader_module(
        render_state.device(),
        "shader_bloom_blur.wgsl",
        include_str!("./assets/shader_bloom_blur.wgsl"),
        &[],
    );

    let shader_downscale = create_engine_shader_module(
        render_state.device(),
        "shader_bloom_downscale.wgsl",
        include_str!("./assets/shader_bloom_downscale.wgsl"),
        &[],
    );

    let shader_apply = create_engine_shader_module(
        render_state.device(),
        "shader_bloom_apply.wgsl",
        include_str!("./assets/shader_bloom_apply.wgsl"),
        &[],
    );

    let render_pipeline_gather = create_render_pipeline(
        render_state.device(),
        Some("Bloom Gather Threshold Render Pipeline"),
        &render_pipeline_layout_gather,
        &shader_gather,
        &[],
        wgpu::ColorTargetState {
            format: textures[0].format(),
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        },
        None,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
    );

    let render_pipeline_blur = create_render_pipeline(
        render_state.device(),
        Some("Bloom Blur Horiz 0 Render Pipeline"),
        &render_pipeline_layout_blur,
        &shader_blur,
        &[],
        wgpu::ColorTargetState {
            format: textures[0].format(),
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        },
        None,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
    );

    let render_pipeline_downscale = create_render_pipeline(
        render_state.device(),
        Some("Bloom Downscale Render Pipeline"),
        &render_pipeline_layout_downscale,
        &shader_downscale,
        &[],
        wgpu::ColorTargetState {
            format: textures[0].format(),
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        },
        None,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
    );

    let render_pipeline_apply = create_render_pipeline(
        render_state.device(),
        Some("Bloom Apply Render Pipeline"),
        &render_pipeline_layout_apply,
        &shader_apply,
        &[],
        wgpu::ColorTargetState {
            format: RENDER_TARGET_FORMAT,
            blend: Some(wgpu::BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        },
        None,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
    );

    let render_res = BloomRenderResource {
        target_texture: unsafe { current_rendering.target.native() }.texture.clone(),
        buffer_storage_layers,
        buffer_uniform,
        textures,
        bind_group_gather,
        bind_group_blur_horiz_0,
        bind_group_blur_horiz_1,
        bind_group_blur_vert,
        bind_group_downscale_0,
        bind_group_downscale_1,
        bind_group_apply_0,
        bind_group_apply_1,
        render_pipeline_gather,
        render_pipeline_blur,
        render_pipeline_downscale,
        render_pipeline_apply,
    };

    if contains_transparent_target {
        *world.resources_mut().get_mut().unwrap() = render_res;
    } else {
        world.resources_mut().insert(render_res);
    }
}

pub fn recreate_color_grading_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    let render_state = unsafe { render_api.raw() };

    let mut contains_render_res = false;

    if let Some(render_res) = world.as_ref().resources().get::<ColorGradingRenderResource>() {
        contains_render_res = true;

        let are_same_size = true
            && render_res.texture.size().width == screen_size[0]
            && render_res.texture.size().height == screen_size[1];

        if are_same_size && render_res.target_texture == unsafe { current_rendering.target.native() }.texture {
            return;
        }
    }

    let texture = render_state.device().create_texture(&TextureDescriptor {
        label: Some(&format!("Color Grading Temp Texture")),
        size: Extent3d {
            width: screen_size[0],
            height: screen_size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&Default::default());

    let sampler = render_state.device().create_sampler(&SamplerDescriptor {
        label: Some("Color Grading Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: FilterMode::Nearest,
        compare: None,
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    let bind_group_layout = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Color Grading Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerNonFiltering,
        ],
    );

    let bind_group = create_bind_group(
        render_state.device(),
        Some("Color Grading Bind Group"),
        &bind_group_layout,
        &[
            CreateBindGroupEntry::Texture(unsafe { &current_rendering.target.native().view }),
            CreateBindGroupEntry::Sampler(&sampler),
        ],
    );

    let render_pipeline_layout = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Color Grading Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let shader = create_engine_shader_module(
        render_state.device(),
        "shader_color_grading_aces.wgsl",
        include_str!("./assets/shader_color_grading_aces.wgsl"),
        &[],
    );

    let render_pipeline = create_render_pipeline(
        render_state.device(),
        Some("Color Grading Render Pipeline"),
        &render_pipeline_layout,
        &shader,
        &[],
        wgpu::ColorTargetState {
            format: texture.format(),
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        },
        None,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
    );

    let render_res = ColorGradingRenderResource {
        target_texture: unsafe { current_rendering.target.native() }.texture.clone(),
        texture,
        texture_view,
        sampler,
        bind_group_layout,
        bind_group,
        render_pipeline,
    };

    if contains_render_res {
        *world.resources_mut().get_mut().unwrap() = render_res;
    } else {
        world.resources_mut().insert(render_res);
    }
}

pub fn create_gizmos_render_resource(mut world: WorldDataMut) {
    let render_state = unsafe { world.as_ref().resources().get::<RenderApiResource>().unwrap().raw() };

    let vertex_buffer = render_state.device().create_buffer(&BufferDescriptor {
        label: Some("Gizmos Vertex Buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        size: (GIZMO_LINES_PER_DRAW_MAX * size_of::<[Vec4<f32>; 2]>()) as u64,
        mapped_at_creation: false,
    });

    let color_buffer = render_state.device().create_buffer(&BufferDescriptor {
        label: Some("Gizmos Color Buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        size: (GIZMO_LINES_PER_DRAW_MAX * size_of::<Vec4<f32>>()) as u64,
        mapped_at_creation: false,
    });

    let transform_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Gizmos Transform Buffer"),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        contents: aphyn_utils::mem::as_bytes(Mat4::<f32>::IDENTITY.as_array()),
    });

    let bind_group_layout = aphyn_render::create_bind_group_layout(
        render_state.device(),
        Some("Gizmos Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::BufferStorage,
            CreateBindGroupLayoutEntry::BufferStorage,
            CreateBindGroupLayoutEntry::BufferUniform,
        ],
    );

    let bind_group = create_bind_group(
        render_state.device(),
        Some("Gizmos Bind Group"),
        &bind_group_layout,
        &[
            CreateBindGroupEntry::Buffer(&vertex_buffer),
            CreateBindGroupEntry::Buffer(&color_buffer),
            CreateBindGroupEntry::Buffer(&transform_buffer),
        ],
    );

    let pipeline_layout = render_state.device().create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Gizmos Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let shader = create_engine_shader_module(
        render_state.device(),
        "shader_gizmo.wgsl",
        include_str!("./assets/shader_gizmo.wgsl"),
        &[],
    );

    let pipeline = create_render_pipeline(
        render_state.device(),
        Some("Gizmos Render Pipeline"),
        &pipeline_layout,
        &shader,
        &[],
        wgpu::ColorTargetState {
            format: RENDER_TARGET_FORMAT,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        },
        None,
        PrimitiveTopology::LineList,
        None,
    );

    world.as_mut()
        .resources_mut()
        .insert(GizmosRenderResource {
            vertex_buffer,
            color_buffer,
            transform_buffer,
            bind_group,
            pipeline,
            frame_lines: Vec::new(),
        });
}

pub fn update_camera_uniform(
    current_rendering: Res<CurrentRenderingResource>,
    mut standard_render_res: ResMut<StandardRenderResource>,
) {
    standard_render_res.camera_proj_matrix = current_rendering.world_to_clip();
    standard_render_res.camera_pos = current_rendering.camera_position;
}

pub fn clear_depth(render_api: Res<RenderApiResource>, depth_res: Res<DepthTextureResource>) {
    let render_state = unsafe { render_api.raw() };

    let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Clear Depth Encoder"),
    });

    {
        let mut _render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Clear Depth Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: unsafe { &depth_res.texture.native().view },
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
    }

    render_state.queue().submit(std::iter::once(encoder.finish()));
}

pub fn clear_transparent_target(render_api: Res<RenderApiResource>, transparent_target_res: Res<TransparentTargetTextureResource>) {
    let render_state = unsafe { render_api.raw() };

    let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Clear Transparent Target Encoder"),
    });

    {
        let mut _render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Clear Transparent Target Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: unsafe { &transparent_target_res.texture.native().view },
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
    }

    render_state.queue().submit(std::iter::once(encoder.finish()));
}

pub fn update_lights_buffer(
    light_q: WorldQuery<(
        Option<&GlobalTransform>,
        &StandardLightComponent,
    )>,
    render_api: Res<RenderApiResource>,
    current_rendering: Res<CurrentRenderingResource>,
    mut standard_render_res: ResMut<StandardRenderResource>,
) {
    let mut lights = light_q
        .iter()
        .map(|(light_transform, light_c)| {
            let light_transform = light_transform.copied().unwrap_or_default();

            utils::light_from_components(light_c, &light_transform)
        })
        .chain(current_rendering.light);

    let lights_count = (light_q.len() as usize + current_rendering.light.is_some() as usize).min(LIGHTS_COUNT_MAX) as u32;

    // elements are written in index order, so the iterator can be consumed sequentially
    render_api.write_lights_with(&standard_render_res.globals, lights_count, |_| {
        lights.next().map(Into::into).unwrap_or_default()
    });

    standard_render_res.lights_count = lights_count;
}

pub fn update_global_uniforms(
    render_api: Res<RenderApiResource>,
    current_rendering: Res<CurrentRenderingResource>,
    screen_space_res: Res<ScreenSpaceResource>,
    standard_render_res: Res<StandardRenderResource>,
) {
    let target_size = current_rendering.target_size();

    let uniform = StandardUniformGlobal {
        world_to_clip: standard_render_res.camera_proj_matrix,
        window_to_clip: crate::utils::create_window_to_clip_matrix(
            target_size[0] as f32,
            target_size[1] as f32,
            screen_space_res.near,
            screen_space_res.far,
        ),
        lights_count: standard_render_res.lights_count,
        camera_position_world: standard_render_res.camera_pos,
    };

    render_api.write_global_uniform(&standard_render_res.globals, uniform);
}

pub fn render_opaque(
    mesh_q: WorldQuery<(
        Option<&GlobalTransform>,
        Option<&StandardMeshComponent>,
        Option<&BatchedMeshComponent>,
        &StandardMaterialComponent,
        Option<&GlobalDisableableComponent>,
    ), OrFilter<(
        WithFilter<StandardMeshComponent>,
        WithFilter<BatchedMeshComponent>,
    )>>,
    render_api: Res<RenderApiResource>,
    current_rendering: Res<CurrentRenderingResource>,
    mut materials: ResMut<AssetStorageResource<StandardMaterial>>,
    textures: Res<AssetStorageResource<StandardTexture>>,
    shaders: Res<AssetStorageResource<StandardShader>>,
    meshes: Res<AssetStorageResource<StandardMesh>>,
    standard_render_res: Res<StandardRenderResource>,
    depth_res: Res<DepthTextureResource>,
) {
    if mesh_q.len() == 0 {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let view = unsafe { &current_rendering.target.native().view };

    // todo: try using single vec and ordering it based on materials/meshes/batching-or-instancing. Separate it while iterating. Reuse it as SystemData to avoid reallocation
    // order: material -> instance/batch -> mesh

    #[derive(Clone)]
    enum MeshTy<'a> {
        Instanced(AssetHandle<StandardMesh>),
        Batched(&'a BatchedMeshComponent)
    }

    let mut render_requests = Vec::new();

    for (transform, standard_mesh, batched_mesh, render_material, disableable) in mesh_q.iter() {
        if disableable.copied().unwrap_or_default().is_disabled {
            continue;
        }

        let Some(material) = materials.get(&render_material.material) else {
            continue;
        };

        if material.meta().shader.is_transparent() {
            continue;
        }

        let mat = match transform {
            Some(transform) => transform.scale_rotation.into_4x4_with_offset(transform.position),
            None => Mat4::IDENTITY,
        };

        if let Some(standard_mesh) = standard_mesh {
            render_requests.push((render_material.material.clone(), MeshTy::Instanced(standard_mesh.mesh.clone()), mat));
        }
        
        if let Some(batched_mesh) = batched_mesh {
            render_requests.push((render_material.material.clone(), MeshTy::Batched(batched_mesh), mat));
        }
    }

    render_requests.sort_by(|l, r| {
        let (l_mesh, r_mesh) = match (&l.1, &r.1) {
            (MeshTy::Instanced(_), MeshTy::Batched(_)) => return std::cmp::Ordering::Less,
            (MeshTy::Batched(_), MeshTy::Instanced(_)) => return std::cmp::Ordering::Greater,
            (MeshTy::Batched(_), MeshTy::Batched(_)) => return std::cmp::Ordering::Equal,
            (MeshTy::Instanced(l_mesh), MeshTy::Instanced(r_mesh)) => (l_mesh, r_mesh),
        };

        // todo: order by material first to allow materials ordering between each other.
        match l.0.cmp(&r.0) {
            std::cmp::Ordering::Equal => (),
            result => return result,
        }

        l_mesh.cmp(r_mesh)
    });

    for mesh_ty_chunk in render_requests.chunk_by(|l, r| std::mem::discriminant(&l.1) == std::mem::discriminant(&r.1)) {
        match mesh_ty_chunk[0].1 {
            MeshTy::Instanced(_) => {
                for material_chunk in mesh_ty_chunk.chunk_by(|l, r| l.0 == r.0) {
                    let Some(material) = materials.get_mut(&material_chunk[0].0) else {
                        continue;
                    };

                    material.update_gpu_data_if_dirty(&render_state, &textures);

                    let custom_shader = crate::utils::custom_shader(material, &shaders);

                    let mesh_chunks = material_chunk.chunk_by(|l, r| {
                        let MeshTy::Instanced(l_mesh) = &l.1 else {
                            unreachable!();
                        };
                        let MeshTy::Instanced(r_mesh) = &r.1 else {
                            unreachable!();
                        };
                        l_mesh == r_mesh
                    });

                    for mesh_chunk in mesh_chunks {
                        let MeshTy::Instanced(mesh) = &mesh_chunk[0].1 else {
                            unreachable!();
                        };
                        let Some(mesh) = meshes.get(&mesh) else {
                            continue;
                        };

                        for matrix_chunk in mesh_chunk.chunks(INSTANCES_PER_DRAW_MAX) {
                            let instance_count = matrix_chunk.len() as u32;

                            render_api.write_instances_with(&standard_render_res.instances, 0, instance_count, |i| matrix_chunk[i as usize].2);

                            render_api.draw_instanced(&StandardInstancedDraw {
                                target: &current_rendering.target,
                                depth: &depth_res.texture,
                                globals: &standard_render_res.globals,
                                material,
                                custom_shader,
                                mesh,
                                instances: &standard_render_res.instances,
                                instance_start: 0,
                                instance_count,
                            });
                        }
                    }
                }
            },
            MeshTy::Batched(_) => {
                for material_chunk in mesh_ty_chunk.chunk_by(|l, r| l.0 == r.0) {
                    let Some(material) = materials.get_mut(&material_chunk[0].0) else {
                        continue;
                    };

                    material.update_gpu_data_if_dirty(&render_state, &textures);

                    let Some(render_pipeline) = render_state.material_pipeline(material, crate::utils::custom_shader(material, &shaders)) else {
                        continue;
                    };
                    let bind_group = unsafe { &material.native().bind_group };

                    let mut flush = |vertex_count| {
                        submit_render_batched(&render_state, &standard_render_res, vertex_count, view, &depth_res, render_pipeline, bind_group);
                    };

                    let mut batch_len = 0;

                    for (_, batched_mesh, mat) in material_chunk {
                        let MeshTy::Batched(batched_mesh) = batched_mesh else {
                            unreachable!();
                        };

                        write_batched_mesh(&render_state, &standard_render_res, batched_mesh, mat, &mut batch_len, &mut flush);
                    }

                    if batch_len > 0 {
                        flush(batch_len as u32);
                    }
                }
            },
        }
    }
}

// todo: a lot of duplication
pub fn render_transparent_instanced(
    query: WorldQuery<(
        Option<&GlobalTransform>,
        &StandardMeshComponent,
        &StandardMaterialComponent,
        Option<&GlobalDisableableComponent>,
    )>,
    render_api: Res<RenderApiResource>,
    standard_render_res: Res<StandardRenderResource>,
    depth_res: Res<DepthTextureResource>,
    transparent_target_res: Res<TransparentTargetTextureResource>,
    meshes: Res<AssetStorageResource<StandardMesh>>,
    mut materials: ResMut<AssetStorageResource<StandardMaterial>>,
    textures: Res<AssetStorageResource<StandardTexture>>,
    shaders: Res<AssetStorageResource<StandardShader>>,
) {
    if query.len() == 0 {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let mut instanced_matrices = HashMap::new();

    for (transform, render_mesh, render_material, disableable) in query.iter() {
        if disableable.copied().unwrap_or_default().is_disabled {
            continue;
        }

        let Some(material) = materials.get(&render_material.material) else {
            continue;
        };

        if !material.meta().shader.is_transparent() {
            continue;
        }

        let mat = match transform {
            Some(transform) => transform.scale_rotation.into_4x4_with_offset(transform.position),
            None => Mat4::IDENTITY,
        };
        instanced_matrices
            .entry((render_mesh.mesh.clone(), render_material.material.clone()))
            .or_insert_with(|| Vec::new())
            .push(mat);
    }

    for ((mesh, material), matrices) in instanced_matrices.iter() {
        let Some(mesh) = meshes.get(&mesh) else {
            continue;
        };
        let Some(material) = materials.get_mut(&material) else {
            continue;
        };

        material.update_gpu_data_if_dirty(&render_state, &textures);

        let custom_shader = crate::utils::custom_shader(material, &shaders);

        for matrices in matrices.chunks(INSTANCES_PER_DRAW_MAX) {
            let instance_count = matrices.len() as u32;

            render_api.write_instances(&standard_render_res.instances, 0, matrices);

            render_api.draw_instanced(&StandardInstancedDraw {
                target: &transparent_target_res.texture,
                depth: &depth_res.texture,
                globals: &standard_render_res.globals,
                material,
                custom_shader,
                mesh,
                instances: &standard_render_res.instances,
                instance_start: 0,
                instance_count,
            });
        }
    }
}

// todo: a lot of duplication
pub fn render_transparent_batched(
    query: WorldQuery<(
        Option<&GlobalTransform>,
        &BatchedMeshComponent,
        &StandardMaterialComponent,
        Option<&GlobalDisableableComponent>,
    )>,
    render_api: Res<RenderApiResource>,
    standard_render_res: Res<StandardRenderResource>,
    depth_res: Res<DepthTextureResource>,
    transparent_target_res: Res<TransparentTargetTextureResource>,
    mut materials: ResMut<AssetStorageResource<StandardMaterial>>,
    textures: Res<AssetStorageResource<StandardTexture>>,
    shaders: Res<AssetStorageResource<StandardShader>>,
) {
    if query.len() == 0 {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let view = unsafe { &transparent_target_res.texture.native().view };

    let mut batched_meshes_by_material = HashMap::new();

    for (transform, batched_mesh, render_material, disableable) in query.iter() {
        if disableable.copied().unwrap_or_default().is_disabled {
            continue;
        }

        let Some(material) = materials.get(&render_material.material) else {
            continue;
        };

        if !material.meta().shader.is_transparent() {
            continue;
        }

        let mat = match transform {
            Some(transform) => transform.scale_rotation.into_4x4_with_offset(transform.position),
            None => Mat4::IDENTITY,
        };
        batched_meshes_by_material
            .entry(render_material.material.clone())
            .or_insert_with(|| Vec::new())
            .push((mat, batched_mesh));
    }

    for (material, matrices_and_meshes) in &batched_meshes_by_material {
        let Some(material) = materials.get_mut(&material) else {
            continue;
        };

        material.update_gpu_data_if_dirty(&render_state, &textures);

        let Some(render_pipeline) = render_state.material_pipeline(material, crate::utils::custom_shader(material, &shaders)) else {
            continue;
        };
        let bind_group = unsafe { &material.native().bind_group };

        let mut flush = |vertex_count| {
            submit_render_batched(&render_state, &standard_render_res, vertex_count, view, &depth_res, render_pipeline, bind_group);
        };

        let mut batch_len = 0;

        for (mat, batched_mesh) in matrices_and_meshes {
            write_batched_mesh(&render_state, &standard_render_res, batched_mesh, mat, &mut batch_len, &mut flush);
        }

        if batch_len > 0 {
            flush(batch_len as u32);
        }
    }
}

pub fn render_transparent_final_system(
    render_api: Res<RenderApiResource>,
    standard_render_res: Res<StandardRenderResource>,
    transparent_target_res: Res<TransparentTargetTextureResource>,
    current_rendering: Res<CurrentRenderingResource>,
) {
    let render_state = unsafe { render_api.raw() };

    let view = unsafe { &current_rendering.target.native().view };

    let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Transparent Final Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Transparent Final Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        render_pass.set_pipeline(&standard_render_res.render_pipeline_transparent_final);
        render_pass.set_bind_group(0, &transparent_target_res.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    render_state.queue().submit(std::iter::once(encoder.finish()));
}

pub fn render_exposure_system(
    render_api: Res<RenderApiResource>,
    exposure_res: Res<ExposureResource>,
    exposure_render_resource: Res<ExposureRenderResource>,
    current_rendering: Res<CurrentRenderingResource>,
) {
    if !exposure_res.is_enabled {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let exposure_multiplier = 2.0_f32.powf(exposure_res.exposure);

    render_state.queue().write_buffer(&exposure_render_resource.buffer_uniform, 0, aphyn_utils::mem::as_bytes(&exposure_multiplier));

    let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Exposure Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Exposure Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &exposure_render_resource.texture.create_view(&Default::default()),
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        render_pass.set_pipeline(&exposure_render_resource.render_pipeline);
        render_pass.set_bind_group(0, &exposure_render_resource.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    {
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &exposure_render_resource.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            TexelCopyTextureInfo {
                texture: &unsafe { current_rendering.target.native() }.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            unsafe { current_rendering.target.native() }.texture.size(),
        );
    }

    render_state.queue().submit([encoder.finish()]);
}

pub fn render_bloom_system(
    render_api: Res<RenderApiResource>,
    bloom_res: Res<BloomResource>,
    bloom_render_resource: Res<BloomRenderResource>,
    current_rendering: Res<CurrentRenderingResource>,
) {
    if !bloom_res.is_enabled {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let screen_size = current_rendering.target_size().map(|i| i as f32);

    render_state.queue().write_buffer(&bloom_render_resource.buffer_uniform, 0, aphyn_utils::mem::as_bytes(&BloomUniform {
        _padding0: 0.0,
        intensity: bloom_res.intensity,
        threshold: Vec2::<f32>::new(bloom_res.threshold, bloom_res.threshold_softening),
    }));

    let mut command_buffers = Vec::new();

    let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Gather Bloom Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Gather Bloom Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &bloom_render_resource.textures[0].create_view(&Default::default()),
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        render_pass.set_pipeline(&bloom_render_resource.render_pipeline_gather);
        render_pass.set_bind_group(0, &bloom_render_resource.bind_group_gather, &[]);
        render_pass.draw(0..3, 0..1);
    }

    command_buffers.push(encoder.finish());

    //

    let instance_id_to_range = |i| i..(i + 1);

    let fn_viewport_scale_by_layer = |blur_layer: usize| {
        let downscale_uv_output = 1.0 / (1 << blur_layer) as f32;
        [screen_size[0] * downscale_uv_output, screen_size[1] * downscale_uv_output]
    };

    let fn_instance_id_by_layer = |blur_layer: usize| blur_layer as u32 * 2;

    for blur_layer in 0..BLUR_LAYERS_COUNT {
        let next_blur_layer = blur_layer + 1;

        if next_blur_layer < BLUR_LAYERS_COUNT {
            let (
                downscale_target_view,
                downscale_bind_group,
            ) = if next_blur_layer % 2 == 0 {
                (
                    &bloom_render_resource.textures[0].create_view(&Default::default()),
                    &bloom_render_resource.bind_group_downscale_1,
                )
            } else {
                (
                    &bloom_render_resource.textures[1].create_view(&Default::default()),
                    &bloom_render_resource.bind_group_downscale_0,
                )
            };

            let downscale_instance_id = fn_instance_id_by_layer(next_blur_layer);
            let downscale_viewport_scale = fn_viewport_scale_by_layer(next_blur_layer);

            let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Downscale Bloom Render Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Downscale Bloom Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &downscale_target_view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(Color::BLACK),
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

                render_pass.set_viewport(0.0, 0.0, downscale_viewport_scale[0], downscale_viewport_scale[1], 0.0, 1.0);
                render_pass.set_pipeline(&bloom_render_resource.render_pipeline_downscale);
                render_pass.set_bind_group(0, downscale_bind_group, &[]);
                render_pass.draw(0..3, instance_id_to_range(downscale_instance_id - 2));
            }

            command_buffers.push(encoder.finish());
        }

        //

        let (
            blur_init_texture,
            blur_init_bind_group,
        ) = if blur_layer % 2 == 0 {
            (
                &bloom_render_resource.textures[0],
                &bloom_render_resource.bind_group_blur_horiz_0,
            )
        } else {
            (
                &bloom_render_resource.textures[1],
                &bloom_render_resource.bind_group_blur_horiz_1,
            )
        };

        let blur_instance_id = fn_instance_id_by_layer(blur_layer);
        let blur_viewport_scale = fn_viewport_scale_by_layer(blur_layer);

        let blur_dir_entries = [
            (blur_init_bind_group, &bloom_render_resource.textures[2]),
            (&bloom_render_resource.bind_group_blur_vert, blur_init_texture),
        ];

        for (dir_i, (bind_group, target_tex)) in blur_dir_entries.into_iter().enumerate() {
            let view = target_tex.create_view(&Default::default());

            let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Blur Bloom Render Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Blur Bloom Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(Color::BLACK),
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

                render_pass.set_viewport(0.0, 0.0, blur_viewport_scale[0], blur_viewport_scale[1], 0.0, 1.0);
                render_pass.set_pipeline(&bloom_render_resource.render_pipeline_blur);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.draw(0..3, instance_id_to_range(blur_instance_id + dir_i as u32));
            }

            command_buffers.push(encoder.finish());
        }

        //

        let apply_bind_group = if blur_layer % 2 == 0 {
            &bloom_render_resource.bind_group_apply_0
        } else {
            &bloom_render_resource.bind_group_apply_1
        };

        let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Apply Bloom Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Apply Bloom Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: unsafe { &current_rendering.target.native().view },
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(&bloom_render_resource.render_pipeline_apply);
            render_pass.set_bind_group(0, apply_bind_group, &[]);
            render_pass.draw(0..3, instance_id_to_range(blur_instance_id));
        }

        command_buffers.push(encoder.finish());
    }

    render_state.queue().submit(command_buffers);
}

pub fn render_color_grading_system(
    render_api: Res<RenderApiResource>,
    color_grading_res: Res<ColorGradingResource>,
    color_grading_render_resource: Res<ColorGradingRenderResource>,
    current_rendering: Res<CurrentRenderingResource>,
) {
    if color_grading_res.ty.is_none() {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Color Grading Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Color Grading Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &color_grading_render_resource.texture.create_view(&Default::default()),
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        render_pass.set_pipeline(&color_grading_render_resource.render_pipeline);
        render_pass.set_bind_group(0, &color_grading_render_resource.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    {
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &color_grading_render_resource.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            TexelCopyTextureInfo {
                texture: &unsafe { current_rendering.target.native() }.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            unsafe { current_rendering.target.native() }.texture.size(),
        );
    }

    render_state.queue().submit([encoder.finish()]);
}

pub fn prepare_gizmos(mut gizmos: ResMut<GizmosResource>, mut gizmos_render_res: ResMut<GizmosRenderResource>) {
    gizmos_render_res.frame_lines.clear();

    for (space, lines) in gizmos.spaces() {
        gizmos_render_res.frame_lines.push((*space, lines.to_vec()));
        lines.clear();
    }
}

pub fn render_gizmos(
    mut gizmos_render_res: ResMut<GizmosRenderResource>,
    current_rendering: Res<CurrentRenderingResource>,
    screen_space_res: Res<ScreenSpaceResource>,
    standard_render_res: Res<StandardRenderResource>,
    render_api: Res<RenderApiResource>,
) {
    let view = unsafe { &current_rendering.target.native().view };

    let render_state = unsafe { render_api.raw() };

    let target_size = current_rendering.target_size();

    let GizmosRenderResource {
        vertex_buffer,
        color_buffer,
        transform_buffer,
        pipeline,
        bind_group,
        frame_lines,
    } = &mut *gizmos_render_res;

    for (space, lines) in frame_lines.iter() {
        if lines.is_empty() {
            continue;
        }

        let transform = match space {
            RenderSpace::Clip => Mat4::<f32>::IDENTITY,
            RenderSpace::Window => crate::utils::create_window_to_clip_matrix(
                target_size[0] as f32,
                target_size[1] as f32,
                screen_space_res.near,
                screen_space_res.far,
            ),
            RenderSpace::World => standard_render_res.camera_proj_matrix,
        };

        render_state.queue().write_buffer(
            transform_buffer,
            0,
            aphyn_utils::mem::as_bytes(transform.as_array()),
        );

        for lines in lines.chunks(GIZMO_LINES_PER_DRAW_MAX) {
            let count = lines.len();

            write_buffer_elements_with(render_state.queue(), vertex_buffer, 0, count as u64, |i| {
                let line = &lines[i as usize];
                [
                    Vec4::new(line.start.x, line.start.y, line.start.z, 1.0),
                    Vec4::new(line.end.x, line.end.y, line.end.z, 1.0),
                ]
            });
            write_buffer_elements_with(render_state.queue(), color_buffer, 0, count as u64, |i| lines[i as usize].color);

            let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Gizmos Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Gizmos Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, &*bind_group, &[]);
                render_pass.draw(0..(count as u32 * 2), 0..1);
            }

            render_state.queue().submit(std::iter::once(encoder.finish()));
        }
    }
}

//

fn write_batched_mesh(
    render_state: &RenderState,
    standard_render_res: &StandardRenderResource,
    batched_mesh: &BatchedMeshComponent,
    mat: &Mat4<f32>,
    batch_len: &mut usize,
    mut flush: impl FnMut(u32),
) {
    const BATCH_CAPACITY: usize = TRIANGLES_PER_BATCHED_DRAW_MAX * 3;

    // todo: to Image/Text components, the gamma curve should not be here with batching
    let srgb_to_linear = |x: f32| x.powf(2.2);

    let mut indices = batched_mesh.indices.as_slice();

    while !indices.is_empty() {
        let count = indices.len().min(BATCH_CAPACITY - *batch_len);

        write_buffer_elements_with(
            render_state.queue(),
            &standard_render_res.batched_vertex_buffer,
            *batch_len as u64,
            count as u64,
            |i| {
                let mut vertex = batched_mesh.vertices[indices[i as usize] as u64];

                vertex.position = mat.mul_with_projection(Vec3::from_array(vertex.position)).into_array();
                vertex.normal = mat.mul_with_projection_as_dir(Vec3::from_array(vertex.normal)).into_array();
                vertex.color = [
                    srgb_to_linear(vertex.color[0]),
                    srgb_to_linear(vertex.color[1]),
                    srgb_to_linear(vertex.color[2]),
                    vertex.color[3],
                ];

                vertex
            },
        );

        *batch_len += count;
        indices = &indices[count..];

        if *batch_len == BATCH_CAPACITY {
            flush(BATCH_CAPACITY as u32);
            *batch_len = 0;
        }
    }
}

fn submit_render_batched(
    render_state: &RenderState,
    standard_render_res: &StandardRenderResource,
    vertex_count: u32,
    render_target_view: &TextureView,
    depth_res: &DepthTextureResource,
    render_pipeline: &RenderPipeline,
    bind_group: &BindGroup,
) {
    let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: render_target_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: unsafe { &depth_res.texture.native().view },
                depth_ops: Some(Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        render_pass.set_pipeline(render_pipeline);
        render_pass.set_bind_group(0, unsafe { &standard_render_res.globals.native().bind_group }, &[]);
        render_pass.set_bind_group(1, bind_group, &[]);
        render_pass.set_vertex_buffer(1, unsafe { standard_render_res.identity_instance.native() }.buffer.slice(..));
        render_pass.set_vertex_buffer(0, standard_render_res.batched_vertex_buffer.slice(..));
        render_pass.draw(0..vertex_count, 0..1);
    }

    render_state.queue().submit(std::iter::once(encoder.finish()));
}