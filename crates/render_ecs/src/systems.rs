use core::f32;
use std::{collections::HashMap, ops::Range};

use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::*;
use aphyn_math::*;
use aphyn_render::*;
use aphyn_render_core::{
    GpuBinding, GpuBlendComponent, GpuBlendFactor, GpuBlendOperation, GpuBlendState, GpuColorWrites, GpuCullMode, GpuPassOps,
    GpuPrimitiveTopology, GpuRenderPass, GpuRenderPassTarget, GpuSampler, GpuShader, GpuShaderDesc, GpuTexture, GpuTextureFormat,
    GpuUniformStorageBuffer, GpuViewport, RenderApi,
};

use crate::{utils::*, *};
use aphyn_transform::*;

use super::{
    DepthTextureResource, GizmosRenderResource, GizmosResource, StandardRenderResource,
    components::{StandardMaterialComponent, StandardMeshComponent},
};

// todo: refactor this file's duplications

fn fullscreen_shader_desc(blend: GpuBlendState) -> GpuShaderDesc {
    GpuShaderDesc {
        blend,
        depth_write: false,
        ..Default::default()
    }
}

fn target_texture_size(current_rendering: &CurrentRenderingResource) -> [usize; 2] {
    current_rendering.target.texture().size()
}

pub fn create_standard_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();
    let core = render_api.core();

    let shader_transparent_final = create_engine_shader(
        core,
        "shader_transparent_final.wgsl",
        include_str!("./assets/shader_transparent_final.wgsl"),
        GpuShaderDesc {
            write_mask: GpuColorWrites::Color,
            ..fullscreen_shader_desc(GpuBlendState::ALPHA_BLENDING)
        },
    );

    let instances = render_api.create_instance_buffer(INSTANCES_PER_DRAW_MAX as u32);

    let identity_instance = render_api.create_instance_buffer(1);
    render_api.write_instances(&identity_instance, 0, &[Mat4::IDENTITY]);

    let batched_vertex_buffer = core.create_vertex_buffer(
        Some("batched vertices"),
        TRIANGLES_PER_BATCHED_DRAW_MAX * 3,
        StandardVertex::desc(),
    );

    let globals = render_api.create_global_bindings();

    world.as_mut().resources_mut().insert(StandardRenderResource {
        globals,
        instances,
        identity_instance,
        batched_vertex_buffer,
        lights_count: 0,
        shader_transparent_final,
        camera_pos: Vec3::default(),
        camera_proj_matrix: Mat4::IDENTITY,
    });
}

pub fn recreate_depth_texture_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    if let Some(depth_res) = world.as_ref().resources().get::<DepthTextureResource>() {
        if depth_res.texture.size() == screen_size {
            return;
        }
    }

    let texture = render_api.create_texture(
        FilterMode::Nearest,
        screen_size,
        &[],
        StandardTextureAssetMetadata {
            format: StandardTextureFormat::Depth32Float,
            ..Default::default()
        },
    );

    world.resources_mut().insert(DepthTextureResource { texture });
}

pub fn recreate_transparent_target_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = current_rendering.target_size();

    if let Some(transparent_target_res) = world.as_ref().resources().get::<TransparentTargetTextureResource>() {
        if transparent_target_res.texture.size() == screen_size {
            return;
        }
    }

    let texture = render_api.create_texture(
        FilterMode::Nearest,
        screen_size,
        &[],
        StandardTextureAssetMetadata {
            format: StandardTextureFormat::Rgba16Float,
            ..Default::default()
        },
    );

    world.resources_mut().insert(TransparentTargetTextureResource { texture });
}

pub fn recreate_exposure_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();
    let core = render_api.core();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = target_texture_size(current_rendering);

    let contains_render_res = match world.as_ref().resources().get::<ExposureRenderResource>() {
        Some(render_res) if render_res.texture.size() == screen_size => return,
        Some(_) => true,
        None => false,
    };

    let texture = core.create_texture(Some("exposure temp"), screen_size, RENDER_TARGET_FORMAT, 1);

    if contains_render_res {
        world.resources_mut().get_mut::<ExposureRenderResource>().unwrap().texture = texture;
        return;
    }

    let buffer_uniform = core.create_uniform_storage_buffer(Some("exposure uniform"), size_of::<f32>());
    write_buffer_elements_with(core, &buffer_uniform, 0, 1, |_| 0.0_f32);

    let sampler = core.create_sampler(Some("exposure"), FilterMode::Nearest);

    let shader = create_engine_shader(
        core,
        "shader_exposure.wgsl",
        include_str!("./assets/shader_exposure.wgsl"),
        fullscreen_shader_desc(GpuBlendState::REPLACE),
    );

    world.resources_mut().insert(ExposureRenderResource {
        buffer_uniform,
        texture,
        sampler,
        shader,
    });
}

pub fn recreate_bloom_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();
    let core = render_api.core();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = target_texture_size(current_rendering);

    let contains_render_res = match world.as_ref().resources().get::<BloomRenderResource>() {
        Some(render_res) if render_res.textures.iter().all(|texture| texture.size() == screen_size) => return,
        Some(_) => true,
        None => false,
    };

    let textures = std::array::from_fn::<_, 3, _>(|i| {
        core.create_texture(
            Some(&format!("bloom buffer {i}")),
            screen_size,
            GpuTextureFormat::Rgba16Float,
            1,
        )
    });

    if contains_render_res {
        world.resources_mut().get_mut::<BloomRenderResource>().unwrap().textures = textures;
        return;
    }

    let mut blur_layers: [f32; BLUR_LAYERS_COUNT] = [8.0, 4.0, 2.0, 1.0];

    let layers_intensity_sum = blur_layers.into_iter().sum::<f32>();

    blur_layers.iter_mut().for_each(|i| *i /= layers_intensity_sum);

    let storage_layers = blur_layers
        .into_iter()
        .enumerate()
        .flat_map(|(blur_layer, blur_intensity)| {
            let uv_scale = 1.0 / (1 << blur_layer) as f32;
            let uv_scale_offset = Vec4::<f32>::new(uv_scale, uv_scale, 0.0, 0.0);

            [
                BloomStorageLayer {
                    intensity: blur_intensity,
                    uv_scale_offset,
                    direction: Vec2::new(1.0, 0.0),
                    _padding0: 0.0,
                },
                BloomStorageLayer {
                    intensity: blur_intensity,
                    uv_scale_offset,
                    direction: Vec2::new(0.0, 1.0),
                    _padding0: 0.0,
                },
            ]
        })
        .collect::<Vec<_>>();

    let buffer_storage_layers = core.create_uniform_storage_buffer(
        Some("bloom storage layers"),
        storage_layers.len() * size_of::<BloomStorageLayer>(),
    );
    core.write_uniform_storage_buffer_with(&buffer_storage_layers, 0, buffer_storage_layers.size(), |bytes| {
        bytes.copy_from_slice(aphyn_utils::mem::as_bytes_slice(&storage_layers))
    });

    let buffer_uniform = core.create_uniform_storage_buffer(Some("bloom uniform"), size_of::<BloomUniform>());
    write_buffer_elements_with(core, &buffer_uniform, 0, 1, |_| BloomUniform::default());

    let sampler_linear = core.create_sampler(Some("bloom linear"), FilterMode::Linear);
    let sampler_nearest = core.create_sampler(Some("bloom nearest"), FilterMode::Nearest);

    let shader_gather = create_engine_shader(
        core,
        "shader_bloom_gather_threshold.wgsl",
        include_str!("./assets/shader_bloom_gather_threshold.wgsl"),
        fullscreen_shader_desc(GpuBlendState::REPLACE),
    );

    let shader_blur = create_engine_shader(
        core,
        "shader_bloom_blur.wgsl",
        include_str!("./assets/shader_bloom_blur.wgsl"),
        fullscreen_shader_desc(GpuBlendState::REPLACE),
    );

    let shader_downscale = create_engine_shader(
        core,
        "shader_bloom_downscale.wgsl",
        include_str!("./assets/shader_bloom_downscale.wgsl"),
        fullscreen_shader_desc(GpuBlendState::REPLACE),
    );

    let additive = GpuBlendComponent {
        src_factor: GpuBlendFactor::One,
        dst_factor: GpuBlendFactor::One,
        operation: GpuBlendOperation::Add,
    };

    let shader_apply = create_engine_shader(
        core,
        "shader_bloom_apply.wgsl",
        include_str!("./assets/shader_bloom_apply.wgsl"),
        fullscreen_shader_desc(GpuBlendState {
            color: additive,
            alpha: additive,
        }),
    );

    world.resources_mut().insert(BloomRenderResource {
        buffer_storage_layers,
        buffer_uniform,
        textures,
        sampler_linear,
        sampler_nearest,
        shader_gather,
        shader_blur,
        shader_downscale,
        shader_apply,
    });
}

pub fn recreate_color_grading_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();
    let core = render_api.core();

    let current_rendering = world.as_ref().resources().get::<CurrentRenderingResource>().unwrap();

    let screen_size = target_texture_size(current_rendering);

    let contains_render_res = match world.as_ref().resources().get::<ColorGradingRenderResource>() {
        Some(render_res) if render_res.texture.size() == screen_size => return,
        Some(_) => true,
        None => false,
    };

    let texture = core.create_texture(Some("color grading temp"), screen_size, RENDER_TARGET_FORMAT, 1);

    if contains_render_res {
        world.resources_mut().get_mut::<ColorGradingRenderResource>().unwrap().texture = texture;
        return;
    }

    let sampler = core.create_sampler(Some("color grading"), FilterMode::Nearest);

    let shader = create_engine_shader(
        core,
        "shader_color_grading_aces.wgsl",
        include_str!("./assets/shader_color_grading_aces.wgsl"),
        fullscreen_shader_desc(GpuBlendState::REPLACE),
    );

    world.resources_mut().insert(ColorGradingRenderResource {
        texture,
        sampler,
        shader,
    });
}

pub fn create_gizmos_render_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();
    let core = render_api.core();

    let vertex_buffer = core.create_uniform_storage_buffer(
        Some("gizmos vertices"),
        GIZMO_LINES_PER_DRAW_MAX * size_of::<[Vec4<f32>; 2]>(),
    );

    let color_buffer = core.create_uniform_storage_buffer(Some("gizmos colors"), GIZMO_LINES_PER_DRAW_MAX * size_of::<Vec4<f32>>());

    let transform_buffer = core.create_uniform_storage_buffer(Some("gizmos transform"), size_of::<Mat4<f32>>());
    write_buffer_elements_with(core, &transform_buffer, 0, 1, |_| *Mat4::<f32>::IDENTITY.as_array());

    let shader = create_engine_shader(
        core,
        "shader_gizmo.wgsl",
        include_str!("./assets/shader_gizmo.wgsl"),
        GpuShaderDesc {
            topology: GpuPrimitiveTopology::LineList,
            cull_mode: GpuCullMode::None,
            ..fullscreen_shader_desc(GpuBlendState::REPLACE)
        },
    );

    world.as_mut().resources_mut().insert(GizmosRenderResource {
        vertex_buffer,
        color_buffer,
        transform_buffer,
        shader,
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
    render_api.core().clear_depth_texture(depth_res.texture.texture(), 0, 1.0);
}

pub fn clear_transparent_target(render_api: Res<RenderApiResource>, transparent_target_res: Res<TransparentTargetTextureResource>) {
    render_api
        .core()
        .clear_color_texture(transparent_target_res.texture.texture(), 0, [0.0; 4]);
}

pub fn update_lights_buffer(
    light_q: WorldQuery<(Option<&GlobalTransform>, &StandardLightComponent)>,
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
    mesh_q: WorldQuery<
        (
            Option<&GlobalTransform>,
            Option<&StandardMeshComponent>,
            Option<&BatchedMeshComponent>,
            &StandardMaterialComponent,
            Option<&GlobalDisableableComponent>,
        ),
        OrFilter<(WithFilter<StandardMeshComponent>, WithFilter<BatchedMeshComponent>)>,
    >,
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

    // todo: try using single vec and ordering it based on materials/meshes/batching-or-instancing. Separate it while iterating. Reuse it as SystemData to avoid reallocation
    // order: material -> instance/batch -> mesh

    #[derive(Clone)]
    enum MeshTy<'a> {
        Instanced(AssetHandle<StandardMesh>),
        Batched(&'a BatchedMeshComponent),
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
            render_requests.push((
                render_material.material.clone(),
                MeshTy::Instanced(standard_mesh.mesh.clone()),
                mat,
            ));
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

                    material.update_gpu_data_if_dirty(&render_api);

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

                            render_api.write_instances_with(&standard_render_res.instances, 0, instance_count, |i| {
                                matrix_chunk[i as usize].2
                            });

                            render_api.draw_instanced(&StandardInstancedDraw {
                                target: &current_rendering.target,
                                depth: &depth_res.texture,
                                globals: &standard_render_res.globals,
                                material,
                                custom_shader,
                                textures: &textures,
                                mesh,
                                instances: &standard_render_res.instances,
                                instance_start: 0,
                                instance_count,
                            });
                        }
                    }
                }
            }
            MeshTy::Batched(_) => {
                for material_chunk in mesh_ty_chunk.chunk_by(|l, r| l.0 == r.0) {
                    let Some(material) = materials.get_mut(&material_chunk[0].0) else {
                        continue;
                    };

                    material.update_gpu_data_if_dirty(&render_api);

                    let material = &*material;
                    let custom_shader = crate::utils::custom_shader(material, &shaders);

                    let mut flush = |vertex_count| {
                        submit_render_batched(
                            &render_api,
                            &standard_render_res,
                            &current_rendering.target,
                            &depth_res,
                            material,
                            custom_shader,
                            &textures,
                            vertex_count,
                        );
                    };

                    let mut batch_len = 0;

                    for (_, batched_mesh, mat) in material_chunk {
                        let MeshTy::Batched(batched_mesh) = batched_mesh else {
                            unreachable!();
                        };

                        write_batched_mesh(
                            render_api.core(),
                            &standard_render_res,
                            batched_mesh,
                            mat,
                            &mut batch_len,
                            &mut flush,
                        );
                    }

                    if batch_len > 0 {
                        flush(batch_len);
                    }
                }
            }
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

        material.update_gpu_data_if_dirty(&render_api);

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
                textures: &textures,
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

        material.update_gpu_data_if_dirty(&render_api);

        let material = &*material;
        let custom_shader = crate::utils::custom_shader(material, &shaders);

        let mut flush = |vertex_count| {
            submit_render_batched(
                &render_api,
                &standard_render_res,
                &transparent_target_res.texture,
                &depth_res,
                material,
                custom_shader,
                &textures,
                vertex_count,
            );
        };

        let mut batch_len = 0;

        for (mat, batched_mesh) in matrices_and_meshes {
            write_batched_mesh(
                render_api.core(),
                &standard_render_res,
                batched_mesh,
                mat,
                &mut batch_len,
                &mut flush,
            );
        }

        if batch_len > 0 {
            flush(batch_len);
        }
    }
}

pub fn render_transparent_final_system(
    render_api: Res<RenderApiResource>,
    standard_render_res: Res<StandardRenderResource>,
    transparent_target_res: Res<TransparentTargetTextureResource>,
    current_rendering: Res<CurrentRenderingResource>,
) {
    let transparent_target = &transparent_target_res.texture;

    draw_fullscreen(
        render_api.core(),
        &standard_render_res.shader_transparent_final,
        current_rendering.target.texture(),
        GpuPassOps::Load,
        &[
            (0, texture_binding(transparent_target.texture())),
            (1, GpuBinding::Sampler(transparent_target.sampler())),
        ],
        None,
        0..1,
    );
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

    let core = render_api.core();

    let exposure_multiplier = 2.0_f32.powf(exposure_res.exposure);

    write_buffer_elements_with(core, &exposure_render_resource.buffer_uniform, 0, 1, |_| exposure_multiplier);

    let target = current_rendering.target.texture();

    draw_fullscreen(
        core,
        &exposure_render_resource.shader,
        &exposure_render_resource.texture,
        GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]),
        &[
            (0, texture_binding(target)),
            (1, GpuBinding::Sampler(&exposure_render_resource.sampler)),
            (2, GpuBinding::Buffer(&exposure_render_resource.buffer_uniform)),
        ],
        None,
        0..1,
    );

    core.copy_texture(&exposure_render_resource.texture, target);
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

    let core = render_api.core();

    let screen_size = current_rendering.target_size().map(|i| i as f32);

    write_buffer_elements_with(core, &bloom_render_resource.buffer_uniform, 0, 1, |_| BloomUniform {
        _padding0: 0.0,
        intensity: bloom_res.intensity,
        threshold: Vec2::<f32>::new(bloom_res.threshold, bloom_res.threshold_softening),
    });

    let BloomRenderResource {
        buffer_storage_layers,
        buffer_uniform,
        textures,
        sampler_linear,
        sampler_nearest,
        shader_gather,
        shader_blur,
        shader_downscale,
        shader_apply,
    } = &*bloom_render_resource;

    let bindings = |texture, sampler| bloom_bindings(buffer_uniform, buffer_storage_layers, texture, sampler);

    let clear_black = GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]);

    draw_fullscreen(
        core,
        shader_gather,
        &textures[0],
        clear_black,
        &bindings(current_rendering.target.texture(), sampler_nearest),
        None,
        0..1,
    );

    //

    let instance_id_to_range = |i| i..(i + 1);

    let fn_viewport_by_layer = |blur_layer: usize| {
        let downscale_uv_output = 1.0 / (1 << blur_layer) as f32;

        GpuViewport {
            x: 0.0,
            y: 0.0,
            width: screen_size[0] * downscale_uv_output,
            height: screen_size[1] * downscale_uv_output,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    };

    let fn_instance_id_by_layer = |blur_layer: usize| blur_layer * 2;

    for blur_layer in 0..BLUR_LAYERS_COUNT {
        let next_blur_layer = blur_layer + 1;

        if next_blur_layer < BLUR_LAYERS_COUNT {
            let (downscale_target, downscale_source) = if next_blur_layer % 2 == 0 {
                (&textures[0], &textures[1])
            } else {
                (&textures[1], &textures[0])
            };

            let downscale_instance_id = fn_instance_id_by_layer(next_blur_layer);

            draw_fullscreen(
                core,
                shader_downscale,
                downscale_target,
                clear_black,
                &bindings(downscale_source, sampler_linear),
                Some(fn_viewport_by_layer(next_blur_layer)),
                instance_id_to_range(downscale_instance_id - 2),
            );
        }

        //

        let blur_init_texture = &textures[blur_layer % 2];

        let blur_instance_id = fn_instance_id_by_layer(blur_layer);
        let blur_viewport = fn_viewport_by_layer(blur_layer);

        let blur_dir_entries = [(blur_init_texture, &textures[2]), (&textures[2], blur_init_texture)];

        for (dir_i, (source, target)) in blur_dir_entries.into_iter().enumerate() {
            draw_fullscreen(
                core,
                shader_blur,
                target,
                clear_black,
                &bindings(source, sampler_nearest),
                Some(blur_viewport),
                instance_id_to_range(blur_instance_id + dir_i),
            );
        }

        //

        draw_fullscreen(
            core,
            shader_apply,
            current_rendering.target.texture(),
            GpuPassOps::Load,
            &bindings(blur_init_texture, sampler_linear),
            None,
            instance_id_to_range(blur_instance_id),
        );
    }
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

    let core = render_api.core();

    let target = current_rendering.target.texture();

    draw_fullscreen(
        core,
        &color_grading_render_resource.shader,
        &color_grading_render_resource.texture,
        GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]),
        &[
            (0, texture_binding(target)),
            (1, GpuBinding::Sampler(&color_grading_render_resource.sampler)),
        ],
        None,
        0..1,
    );

    core.copy_texture(&color_grading_render_resource.texture, target);
}

pub fn prepare_gizmos(mut gizmos: ResMut<GizmosResource>, mut gizmos_render_res: ResMut<GizmosRenderResource>) {
    gizmos_render_res.frame_lines.clear();

    for (space, lines) in gizmos.spaces() {
        gizmos_render_res.frame_lines.push((*space, lines.to_vec()));
        lines.clear();
    }
}

pub fn render_gizmos(
    gizmos_render_res: Res<GizmosRenderResource>,
    current_rendering: Res<CurrentRenderingResource>,
    screen_space_res: Res<ScreenSpaceResource>,
    standard_render_res: Res<StandardRenderResource>,
    render_api: Res<RenderApiResource>,
) {
    let core = render_api.core();

    let target_size = current_rendering.target_size();

    let GizmosRenderResource {
        vertex_buffer,
        color_buffer,
        transform_buffer,
        shader,
        frame_lines,
    } = &*gizmos_render_res;

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

        write_buffer_elements_with(core, transform_buffer, 0, 1, |_| *transform.as_array());

        for lines in lines.chunks(GIZMO_LINES_PER_DRAW_MAX) {
            let count = lines.len();

            write_buffer_elements_with(core, vertex_buffer, 0, count, |i| {
                let line = &lines[i];
                [
                    Vec4::new(line.start.x, line.start.y, line.start.z, 1.0),
                    Vec4::new(line.end.x, line.end.y, line.end.z, 1.0),
                ]
            });
            write_buffer_elements_with(core, color_buffer, 0, count, |i| lines[i].color);

            core.draw(
                &GpuRenderPass {
                    shader,
                    target: Some(GpuRenderPassTarget {
                        view: current_rendering.target.texture(),
                        mip_level: 0,
                        ops: GpuPassOps::Load,
                    }),
                    depth: None,
                    bind_groups: &[&[
                        (0, GpuBinding::Buffer(vertex_buffer)),
                        (1, GpuBinding::Buffer(color_buffer)),
                        (2, GpuBinding::Buffer(transform_buffer)),
                    ]],
                    vertex_buffers: &[],
                    viewport: None,
                    scissor: None,
                },
                0..(count * 2),
                0..1,
            );
        }
    }
}

//

fn texture_binding(texture: &GpuTexture) -> GpuBinding<'_> {
    GpuBinding::Texture {
        texture,
        mip_level: None,
    }
}

fn draw_fullscreen(
    core: &RenderApi,
    shader: &GpuShader,
    target: &GpuTexture,
    ops: GpuPassOps<[f32; 4]>,
    bindings: &[(usize, GpuBinding)],
    viewport: Option<GpuViewport>,
    instances: Range<usize>,
) -> bool {
    core.draw(
        &GpuRenderPass {
            shader,
            target: Some(GpuRenderPassTarget {
                view: target,
                mip_level: 0,
                ops,
            }),
            depth: None,
            bind_groups: &[bindings],
            vertex_buffers: &[],
            viewport,
            scissor: None,
        },
        0..3,
        instances,
    )
}

fn write_batched_mesh(
    core: &RenderApi,
    standard_render_res: &StandardRenderResource,
    batched_mesh: &BatchedMeshComponent,
    mat: &Mat4<f32>,
    batch_len: &mut usize,
    mut flush: impl FnMut(usize),
) {
    const BATCH_CAPACITY: usize = TRIANGLES_PER_BATCHED_DRAW_MAX * 3;

    // todo: to Image/Text components, the gamma curve should not be here with batching
    let srgb_to_linear = |x: f32| x.powf(2.2);

    let mut indices = batched_mesh.indices.as_slice();

    while !indices.is_empty() {
        let count = indices.len().min(BATCH_CAPACITY - *batch_len);

        write_buffer_elements_with(core, &standard_render_res.batched_vertex_buffer, *batch_len, count, |i| {
            let mut vertex = batched_mesh.vertices[indices[i] as u64];

            vertex.position = mat.mul_with_projection(Vec3::from_array(vertex.position)).into_array();
            vertex.normal = mat.mul_with_projection_as_dir(Vec3::from_array(vertex.normal)).into_array();
            vertex.color = [
                srgb_to_linear(vertex.color[0]),
                srgb_to_linear(vertex.color[1]),
                srgb_to_linear(vertex.color[2]),
                vertex.color[3],
            ];

            vertex
        });

        *batch_len += count;
        indices = &indices[count..];

        if *batch_len == BATCH_CAPACITY {
            flush(BATCH_CAPACITY);
            *batch_len = 0;
        }
    }
}

fn submit_render_batched(
    render_api: &RenderApiResource,
    standard_render_res: &StandardRenderResource,
    target: &StandardTexture,
    depth_res: &DepthTextureResource,
    material: &StandardMaterial,
    custom_shader: Option<&StandardShader>,
    textures: &AssetStorageResource<StandardTexture>,
    vertex_count: usize,
) {
    render_api.draw_batched(&StandardBatchedDraw {
        target,
        depth: &depth_res.texture,
        globals: &standard_render_res.globals,
        material,
        custom_shader,
        textures,
        vertices: &standard_render_res.batched_vertex_buffer,
        vertex_count,
        identity_instance: &standard_render_res.identity_instance,
    });
}

fn bloom_bindings<'a>(
    uniform: &'a GpuUniformStorageBuffer,
    storage_layers: &'a GpuUniformStorageBuffer,
    texture: &'a GpuTexture,
    sampler: &'a GpuSampler,
) -> [(usize, GpuBinding<'a>); 4] {
    [
        (0, texture_binding(texture)),
        (1, GpuBinding::Sampler(sampler)),
        (2, GpuBinding::Buffer(uniform)),
        (3, GpuBinding::Buffer(storage_layers)),
    ]
}
