use core::f32;
use std::collections::HashMap;

use fruits_asset_storage::AssetHandle;
use fruits_ecs::*;
use fruits_math::*;
use fruits_render_core::*;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt}, wgt::TextureViewDescriptor, *,
};

use fruits_transform::*;
use crate::{utils::*, *};

use super::{
    DepthTextureResource, GizmosRenderResource, GizmosResource, StandardRenderResource,
    components::{StandardMaterialComponent, StandardMeshComponent},
};

// todo: refactor this file's duplications

pub fn create_standard_render_resource(mut world: WorldDataMut) {
    let render_state = unsafe { world.as_ref().resources().get::<RenderApiResource>().unwrap().raw() };

    let bind_group_layout_transparent_final = fruits_render_core::create_bind_group_layout(
        render_state.device(),
        Some("Transparent Target Bind Group Layout"),
        &[
            CreateBindGroupLayoutEntry::Texture,
            CreateBindGroupLayoutEntry::SamplerNonFiltering,
        ],
    );

    let pipeline_layout_standard = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Standard Pipeline Layout"),
        bind_group_layouts: &[
            &render_state.render_data().bind_group_layout_global,
            &render_state.render_data().bind_group_layout_material,
        ],
        push_constant_ranges: &[],
    });

    let pipeline_layout_transparent_final = render_state.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Transparent Final Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout_transparent_final],
        push_constant_ranges: &[],
    });

    let create_render_pipeline_fn = |is_lit: bool, is_transparent: bool| -> RenderPipeline {
        let defs = [is_lit.then_some("LIT"), is_transparent.then_some("TRANSPARENT")];
        let defs = defs.into_iter().flatten().collect::<Vec<_>>();
        let shader = create_engine_shader_module(
            render_state.device(),
            "shader_standard.wgsl",
            include_str!("./assets/shader_standard.wgsl"),
            &defs,
        );

        let color_target_state = match is_transparent {
            false => wgpu::ColorTargetState {
                format: RENDER_TARGET_FORMAT,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            },
            true => wgpu::ColorTargetState {
                format: TRANSPARENT_TARGET_FORMAT,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            },
        };

        let render_pipeline = create_render_pipeline(
            render_state.device(),
            Some("Standard Render Pipeline"),
            &pipeline_layout_standard,
            &shader,
            &[StandardVertex::desc(), StandardInstance::desc()],
            color_target_state,
            Some(DepthStencilState {
                bias: Default::default(),
                depth_compare: wgpu::CompareFunction::LessEqual,
                depth_write_enabled: !is_transparent,
                format: DEPTH_TEXTURE_FORMAT,
                stencil: Default::default(),
            }),
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
        );

        render_pipeline
    };

    let render_pipeline_opaque_unlit = create_render_pipeline_fn(false, false);
    let render_pipeline_opaque_lit = create_render_pipeline_fn(true, false);
    let render_pipeline_transparent_unlit = create_render_pipeline_fn(false, true);
    let render_pipeline_transparent_lit = create_render_pipeline_fn(true, true);

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

    let instance_cpu_buffer =
        vec![Mat4::<f32>::IDENTITY; INSTANCES_PER_DRAW_MAX].into_boxed_slice();

    let instance_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Instance Buffer"),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes_slice(&instance_cpu_buffer),
    });

    let batched_vertex_cpu_buffer =
        vec![StandardVertex::default(); TRIANGLES_PER_BATCHED_DRAW_MAX * 3].into_boxed_slice();

    let batched_vertex_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Batch vertex buffer"),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes_slice(&batched_vertex_cpu_buffer),
    });

    let lights_cpu_buffer =
        vec![StandardGenericLight::default(); LIGHTS_COUNT_MAX].into_boxed_slice();

    let lights_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Lights buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes_slice(&lights_cpu_buffer),
    });

    let global_uniform_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Standard Global Uniform Buffer"),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes(&[StandardUniformGlobal::default()]),
    });

    let global_bind_group = create_bind_group(
        render_state.device(),
        Some("Standard Global Bind Group"),
        &render_state.render_data().bind_group_layout_global, 
        &[
            CreateBindGroupEntry::Buffer(&global_uniform_buffer),
            CreateBindGroupEntry::Buffer(&lights_buffer),
        ],
    );

    world.as_mut()
        .resources_mut()
        .insert(StandardRenderResource {
            pipeline_layout: pipeline_layout_standard,
            global_uniform_buffer,
            global_bind_group,
            instance_buffer,
            batched_vertex_buffer,
            lights_buffer,
            lights_count: 0,
            render_pipeline_opaque_lit,
            render_pipeline_opaque_unlit,
            render_pipeline_transparent_lit,
            render_pipeline_transparent_unlit,
            render_pipeline_transparent_final,
            bind_group_layout_transparent_final,
            camera_pos: Vec3::default(),
            camera_proj_matrix: Mat4::IDENTITY,
        });

    world.as_mut()
        .resources_mut()
        .insert(BatchedVertexCpuBufferResource(batched_vertex_cpu_buffer));

    world.as_mut()
        .resources_mut()
        .insert(InstancedInstanceCpuBufferResource(instance_cpu_buffer));

    world.as_mut()
        .resources_mut()
        .insert(LightsCpuBufferResource(lights_cpu_buffer));
}

pub fn recreate_depth_texture_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    let render_state = unsafe { render_api.raw() };

    let mut contains_depth = false;

    if let Some(depth_res) = world.as_ref().resources().get::<DepthTextureResource>() {
        contains_depth = true;

        let are_same_size =
            { depth_res.texture.size().width == screen_size[0] && depth_res.texture.size().height == screen_size[1] };

        if are_same_size {
            return;
        }
    }

    let texture = render_state.device().create_texture(&TextureDescriptor {
        label: Some("Depth Buffer"),
        size: Extent3d {
            width: screen_size[0],
            height: screen_size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: DEPTH_TEXTURE_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&TextureViewDescriptor::default());

    let sampler = render_state.device().create_sampler(&SamplerDescriptor {
        label: Some("Depth Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual),
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    let depth_res = DepthTextureResource {
        texture,
        texture_view,
        sampler,
    };

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

        let are_same_size = {
            transparent_target_res.texture.size().width == screen_size[0]
                && transparent_target_res.texture.size().height == screen_size[1]
        };

        if are_same_size {
            return;
        }
    }

    let texture = render_state.device().create_texture(&TextureDescriptor {
        label: Some("Transparent Target Buffer"),
        size: Extent3d {
            width: screen_size[0],
            height: screen_size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TRANSPARENT_TARGET_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&TextureViewDescriptor::default());

    let sampler = render_state.device().create_sampler(&SamplerDescriptor {
        label: Some("Transparent Target Sampler"),
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

    let standard_render_res = world.as_ref().resources().get::<StandardRenderResource>().unwrap();

    let bind_group = create_bind_group(
        render_state.device(),
        Some("Transparent Target Bind Group"),
        &standard_render_res.bind_group_layout_transparent_final,
        &[
            CreateBindGroupEntry::Texture(&texture_view),
            CreateBindGroupEntry::Sampler(&sampler),
        ],
    );

    let transparent_target_res = TransparentTargetTextureResource {
        texture,
        texture_view,
        sampler,
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

        if are_same_size && render_res.target_texture == current_rendering.target_texture {
            return;
        }
    }

    let buffer_uniform = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Exposure Uniform Buffer"),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes(&0.0_f32),
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

    let bind_group_layout = fruits_render_core::create_bind_group_layout(
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
            CreateBindGroupEntry::Texture(&current_rendering.target_view),
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
        target_texture: current_rendering.target_texture.clone(),
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

        if are_same_size && render_res.target_texture == current_rendering.target_texture {
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
        contents: fruits_utils::mem::as_bytes_slice(&storage_layers),
    });

    let buffer_uniform = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Bloom Uniform Buffer"),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes(&BloomUniform::default()),
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

    let bind_group_layout_gather = fruits_render_core::create_bind_group_layout(
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
            CreateBindGroupEntry::Texture(&current_rendering.target_view),
            CreateBindGroupEntry::Sampler(&sampler_nearest),
            CreateBindGroupEntry::Buffer(&buffer_uniform),
            CreateBindGroupEntry::Buffer(&buffer_storage_layers),
        ],
    );

    let bind_group_layout_blur = fruits_render_core::create_bind_group_layout(
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

    let bind_group_layout_downscale = fruits_render_core::create_bind_group_layout(
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

    let bind_group_layout_apply = fruits_render_core::create_bind_group_layout(
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
        target_texture: current_rendering.target_texture.clone(),
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

        if are_same_size && render_res.target_texture == current_rendering.target_texture {
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

    let bind_group_layout = fruits_render_core::create_bind_group_layout(
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
            CreateBindGroupEntry::Texture(&current_rendering.target_view),
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
        target_texture: current_rendering.target_texture.clone(),
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

    let vertices_cpu_buffer = vec![[Vec4::splat(0.0); 2]; GIZMO_LINES_PER_DRAW_MAX].into_boxed_slice();
    let colors_cpu_buffer = vec![Vec4::splat(0.0); GIZMO_LINES_PER_DRAW_MAX].into_boxed_slice();

    let vertex_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Gizmos Vertex Buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes_slice(&vertices_cpu_buffer),
    });

    let color_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Gizmos Color Buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes_slice(&colors_cpu_buffer),
    });

    let transform_buffer = render_state.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Gizmos Transform Buffer"),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        contents: fruits_utils::mem::as_bytes(Mat4::<f32>::IDENTITY.as_array()),
    });

    let bind_group_layout = fruits_render_core::create_bind_group_layout(
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
            vertices_cpu_buffer,
            colors_cpu_buffer,
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
                view: &depth_res.texture_view,
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
                view: &transparent_target_res.texture_view,
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
    mut lights_cpu_buffer: ResMut<LightsCpuBufferResource>,
) {
    let render_state = unsafe { render_api.raw() };

    let component_lights = light_q.iter().map(|(light_transform, light_c)| {
        let light_transform = light_transform.copied().unwrap_or_default();

        utils::light_from_components(light_c, &light_transform)
    });

    let lights = component_lights.chain(current_rendering.light).take(LIGHTS_COUNT_MAX);

    let mut lights_count = 0;

    for (i, light) in lights.enumerate() {
        lights_cpu_buffer.0[i] = light.into();
        lights_count += 1;
    }

    standard_render_res.lights_count = lights_count;

    let lights_bytes = fruits_utils::mem::as_bytes_slice(&lights_cpu_buffer.0);

    render_state
        .queue()
        .write_buffer(&standard_render_res.lights_buffer, 0, lights_bytes);

    render_state.queue().submit([]);
}

pub fn update_global_uniforms(
    render_api: Res<RenderApiResource>,
    standard_render_res: Res<StandardRenderResource>,
) {
    let render_state = unsafe { render_api.raw() };

    let uniform = StandardUniformGlobal {
        lights_count: standard_render_res.lights_count,
        camera_position_world: standard_render_res.camera_pos,
    };

    render_state.queue().write_buffer(
        &standard_render_res.global_uniform_buffer,
        0,
        fruits_utils::mem::as_bytes(&[uniform]),
    );
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
    screen_space_res: Res<ScreenSpaceResource>,
    materials: Res<AssetStorageResource<StandardMaterial>>,
    meshes: Res<AssetStorageResource<StandardMesh>>,
    standard_render_res: Res<StandardRenderResource>,
    depth_res: Res<DepthTextureResource>,
    mut batched_vertex_cpu_buffer: ResMut<BatchedVertexCpuBufferResource>,
    mut instanced_instance_cpu_buffer: ResMut<InstancedInstanceCpuBufferResource>,
) {
    if mesh_q.len() == 0 {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let view = &current_rendering.target_view;

    let window_size = current_rendering.target_size();

    let window_to_clip_mat = crate::utils::create_window_to_clip_matrix(
        window_size[0] as f32,
        window_size[1] as f32,
        screen_space_res.near,
        screen_space_res.far,
    );

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

        if material.meta().alpha_threshold.is_none() {
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
                    let material = &material_chunk[0].0;

                    let Some(material) = materials.get(&material) else {
                        continue;
                    };
                        
                    let (render_pipeline, bind_group) = crate::utils::get_render_data(
                        material,
                        &standard_render_res,
                        &render_state,
                        window_to_clip_mat,
                    );

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
                        
                        let mesh = unsafe { mesh.native() };

                        if mesh.indices_count == 0 {
                            continue;
                        }

                        for matrix_chunk in mesh_chunk.chunks(INSTANCES_PER_DRAW_MAX) {
                            for (i, (_, _, matrix)) in matrix_chunk.iter().enumerate() {
                                instanced_instance_cpu_buffer.0[i] = *matrix;
                            }

                            let matrices_bytes = fruits_utils::mem::as_bytes_slice(&instanced_instance_cpu_buffer.0[..matrix_chunk.len()]);

                            render_state
                                .queue()
                                .write_buffer(&standard_render_res.instance_buffer, 0, matrices_bytes);
                            
                            let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
                                label: Some("Render Encoder"),
                            });

                            {
                                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                                    label: Some("Render Pass"),
                                    color_attachments: &[Some(RenderPassColorAttachment {
                                        view: view,
                                        resolve_target: None,
                                        ops: Operations {
                                            load: LoadOp::Load,
                                            store: StoreOp::Store,
                                        },
                                        depth_slice: None,
                                    })],
                                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                        view: &depth_res.texture_view,
                                        depth_ops: Some(Operations {
                                            load: LoadOp::Load,
                                            store: StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    }),
                                    ..Default::default()
                                });

                                render_pass.set_pipeline(render_pipeline);
                                render_pass.set_bind_group(0, &standard_render_res.global_bind_group, &[]);
                                render_pass.set_bind_group(1, bind_group, &[]);
                                render_pass.set_vertex_buffer(1, standard_render_res.instance_buffer.slice(..));
                                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                                render_pass.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint16);
                                render_pass.draw_indexed(0..(mesh.indices_count as u32), 0, 0..(matrix_chunk.len() as u32));
                            }

                            render_state.queue().submit(std::iter::once(encoder.finish()));
                        }
                    }
                }
            },
            MeshTy::Batched(_) => {
                render_state.queue().write_buffer(
                    &standard_render_res.instance_buffer,
                    0,
                    fruits_utils::mem::as_bytes(&Mat4::<f32>::IDENTITY),
                );
                render_state.queue().submit([]);

                let batch_cpu_buffer = &mut batched_vertex_cpu_buffer.0;

                for material_chunks in mesh_ty_chunk.chunk_by(|l, r| l.0 == r.0) {
                    let material = &material_chunks[0].0;

                    let Some(material) = materials.get(&material) else {
                        continue;
                    };

                    let (render_pipeline, bind_group) = crate::utils::get_render_data(
                        material,
                        &standard_render_res,
                        &render_state,
                        window_to_clip_mat,
                    );

                    let mut batch_buffer_i = 0;

                    // todo: to Image/Text components, the gamma curve should not be here with batching
                    let srgb_to_linear = |x: f32| x.powf(2.2);
                    for (_, batched_mesh, mat) in material_chunks {
                        let MeshTy::Batched(batched_mesh) = batched_mesh else {
                            unreachable!();
                        };

                        for &i in &batched_mesh.indices {
                            let mut vertex = batched_mesh.vertices[i as u64];

                            vertex.position = mat.mul_with_projection(Vec3::from_array(vertex.position)).into_array();
                            vertex.normal = mat.mul_with_projection_as_dir(Vec3::from_array(vertex.normal)).into_array();
                            vertex.color = [
                                srgb_to_linear(vertex.color[0]),
                                srgb_to_linear(vertex.color[1]),
                                srgb_to_linear(vertex.color[2]),
                                vertex.color[3],
                            ];

                            batch_cpu_buffer[batch_buffer_i] = vertex;

                            batch_buffer_i += 1;

                            if batch_buffer_i < batch_cpu_buffer.len() {
                                continue;
                            }

                            batch_buffer_i = 0;

                            submit_render_batched(
                                &render_state,
                                &standard_render_res,
                                &batch_cpu_buffer[..],
                                view,
                                &depth_res,
                                render_pipeline,
                                bind_group,
                            );
                        }
                    }

                    if batch_buffer_i > 0 {
                        submit_render_batched(
                            &render_state,
                            &standard_render_res,
                            &batch_cpu_buffer[..batch_buffer_i],
                            view,
                            &depth_res,
                            render_pipeline,
                            bind_group,
                        );
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
    screen_space_res: Res<ScreenSpaceResource>,
    standard_render_res: Res<StandardRenderResource>,
    depth_res: Res<DepthTextureResource>,
    transparent_target_res: Res<TransparentTargetTextureResource>,
    current_rendering: Res<CurrentRenderingResource>,
    meshes: Res<AssetStorageResource<StandardMesh>>,
    materials: Res<AssetStorageResource<StandardMaterial>>,
) {
    if query.len() == 0 {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let view = &transparent_target_res.texture_view;

    let window_size = current_rendering.target_size();

    let window_to_clip_mat = crate::utils::create_window_to_clip_matrix(
        window_size[0] as f32,
        window_size[1] as f32,
        screen_space_res.near,
        screen_space_res.far,
    );

    let mut instanced_matrices = HashMap::new();

    for (transform, render_mesh, render_material, disableable) in query.iter() {
        if disableable.copied().unwrap_or_default().is_disabled {
            continue;
        }

        let Some(material) = materials.get(&render_material.material) else {
            continue;
        };

        if material.meta().alpha_threshold.is_some() {
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
        let Some(material) = materials.get(&material) else {
            continue;
        };

        let mesh = unsafe { mesh.native() };

        if mesh.indices_count == 0 {
            continue;
        }

        let (render_pipeline, bind_group) = crate::utils::get_render_data(
            material,
            &standard_render_res,
            &render_state,
            window_to_clip_mat,
        );

        for matrices in matrices.chunks(INSTANCES_PER_DRAW_MAX) {
            let matrices_bytes = fruits_utils::mem::as_bytes_slice(matrices);

            render_state
                .queue()
                .write_buffer(&standard_render_res.instance_buffer, 0, matrices_bytes);

            let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_res.texture_view,
                        depth_ops: Some(Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });

                render_pass.set_pipeline(render_pipeline);
                render_pass.set_bind_group(0, &standard_render_res.global_bind_group, &[]);
                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.set_vertex_buffer(1, standard_render_res.instance_buffer.slice(..));
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..(mesh.indices_count as u32), 0, 0..(matrices.len() as u32));
            }

            render_state.queue().submit(std::iter::once(encoder.finish()));
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
    screen_space_res: Res<ScreenSpaceResource>,
    standard_render_res: Res<StandardRenderResource>,
    depth_res: Res<DepthTextureResource>,
    transparent_target_res: Res<TransparentTargetTextureResource>,
    current_rendering: Res<CurrentRenderingResource>,
    materials: Res<AssetStorageResource<StandardMaterial>>,
    mut batched_vertex_cpu_buffer: ResMut<BatchedVertexCpuBufferResource>,
) {
    if query.len() == 0 {
        return;
    }

    let render_state = unsafe { render_api.raw() };

    let view = &transparent_target_res.texture_view;

    let window_size = current_rendering.target_size();

    let window_to_clip_mat = crate::utils::create_window_to_clip_matrix(
        window_size[0] as f32,
        window_size[1] as f32,
        screen_space_res.near,
        screen_space_res.far,
    );

    let mut batched_meshes_by_material = HashMap::new();

    for (transform, batched_mesh, render_material, disableable) in query.iter() {
        if disableable.copied().unwrap_or_default().is_disabled {
            continue;
        }

        let Some(material) = materials.get(&render_material.material) else {
            continue;
        };

        if material.meta().alpha_threshold.is_some() {
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

    render_state.queue().write_buffer(
        &standard_render_res.instance_buffer,
        0,
        fruits_utils::mem::as_bytes(&Mat4::<f32>::IDENTITY),
    );
    render_state.queue().submit([]);

    let batch_cpu_buffer = &mut batched_vertex_cpu_buffer.0;

    for (material, matrices_and_meshes) in &batched_meshes_by_material {
        let Some(material) = materials.get(&material) else {
            continue;
        };

        let (render_pipeline, bind_group) = crate::utils::get_render_data(
            material,
            &standard_render_res,
            &render_state,
            window_to_clip_mat,
        );

        let mut batch_buffer_i = 0;

        let srgb_to_linear = |x: f32| x.powf(2.2);
        for (mat, batched_mesh) in matrices_and_meshes {
            for &i in &batched_mesh.indices {
                let mut vertex = batched_mesh.vertices[i as u64];

                vertex.position = mat.mul_with_projection(Vec3::from_array(vertex.position)).into_array();
                vertex.normal = mat.mul_with_projection_as_dir(Vec3::from_array(vertex.normal)).into_array();
                vertex.color = [
                    srgb_to_linear(vertex.color[0]),
                    srgb_to_linear(vertex.color[1]),
                    srgb_to_linear(vertex.color[2]),
                    vertex.color[3],
                ];

                batch_cpu_buffer[batch_buffer_i] = vertex;

                batch_buffer_i += 1;

                if batch_buffer_i < batch_cpu_buffer.len() {
                    continue;
                }

                batch_buffer_i = 0;

                submit_render_batched(
                    &render_state,
                    &standard_render_res,
                    &batch_cpu_buffer[..],
                    &view,
                    &depth_res,
                    render_pipeline,
                    bind_group,
                );
            }
        }

        if batch_buffer_i > 0 {
            submit_render_batched(
                &render_state,
                &standard_render_res,
                &batch_cpu_buffer[..batch_buffer_i],
                &view,
                &depth_res,
                render_pipeline,
                bind_group,
            );
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

    let view = &current_rendering.target_view;

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

    render_state.queue().write_buffer(&exposure_render_resource.buffer_uniform, 0, fruits_utils::mem::as_bytes(&exposure_multiplier));

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
                texture: &current_rendering.target_texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            current_rendering.target_texture.size(),
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

    render_state.queue().write_buffer(&bloom_render_resource.buffer_uniform, 0, fruits_utils::mem::as_bytes(&BloomUniform {
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
                    view: &current_rendering.target_view,
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
                texture: &current_rendering.target_texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            current_rendering.target_texture.size(),
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
    let view = &current_rendering.target_view;

    let render_state = unsafe { render_api.raw() };

    let target_size = current_rendering.target_size();

    let GizmosRenderResource {
        vertex_buffer,
        color_buffer,
        transform_buffer,
        pipeline,
        bind_group,
        vertices_cpu_buffer,
        colors_cpu_buffer,
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
            fruits_utils::mem::as_bytes(transform.as_array()),
        );

        for lines in lines.chunks(GIZMO_LINES_PER_DRAW_MAX) {
            let count = lines.len();

            for (i, line) in lines.iter().enumerate() {
                vertices_cpu_buffer[i] = [
                    Vec4::new(line.start.x, line.start.y, line.start.z, 1.0),
                    Vec4::new(line.end.x, line.end.y, line.end.z, 1.0),
                ];
                colors_cpu_buffer[i] = line.color;
            }

            render_state.queue().write_buffer(
                vertex_buffer,
                0,
                fruits_utils::mem::as_bytes_slice(&vertices_cpu_buffer[..count]),
            );
            render_state.queue().write_buffer(
                color_buffer,
                0,
                fruits_utils::mem::as_bytes_slice(&colors_cpu_buffer[..count]),
            );

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

fn submit_render_batched(
    render_state: &RenderState,
    standard_render_res: &StandardRenderResource,
    batched_cpu_slice: &[StandardVertex],
    render_target_view: &TextureView,
    depth_res: &DepthTextureResource,
    render_pipeline: &RenderPipeline,
    bind_group: &BindGroup,
) {
    render_state.queue().write_buffer(
        &standard_render_res.batched_vertex_buffer,
        0,
        fruits_utils::mem::as_bytes_slice(batched_cpu_slice),
    );

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
                view: &depth_res.texture_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        render_pass.set_pipeline(render_pipeline);
        render_pass.set_bind_group(0, &standard_render_res.global_bind_group, &[]);
        render_pass.set_bind_group(1, bind_group, &[]);
        render_pass.set_vertex_buffer(1, standard_render_res.instance_buffer.slice(..));
        render_pass.set_vertex_buffer(0, standard_render_res.batched_vertex_buffer.slice(..));
        render_pass.draw(0..(batched_cpu_slice.len() as u32), 0..1);
    }

    render_state.queue().submit(std::iter::once(encoder.finish()));
}