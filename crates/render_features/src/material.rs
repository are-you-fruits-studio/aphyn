use std::cmp::Ordering;

use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::{EntitiesHolderMut, OrFilter, ResourcesHolderMut, ThreadPoolAccess, WithFilter, WorldDataMut};
use aphyn_ffi::{FfiIndexSet, FfiOption, FfiVec};
use aphyn_math::{Aabb, Frustum, Mat4, Vec3};
use aphyn_render::{
    CurrentRenderingResource, Material, MaterialPassTarget, MaterialTemplate, RenderApiResource, StandardInstanceBuffer,
    StandardMesh, StandardTexture, StandardVertex, write_buffer_elements_with,
};
use aphyn_render_core::{
    GpuBinding, GpuBlendState, GpuColorWrites, GpuPassOps, GpuRenderPass, GpuRenderPassDepth, GpuRenderPassTarget, GpuShader,
    GpuShaderDesc, GpuVertexBuffer, RenderApi,
};
use aphyn_transform::{GlobalDisableableComponent, GlobalTransform};

use crate::{
    AmbientOcclusionTextureResource, BatchedMeshComponent, DepthTextureResource, EnvironmentTextureResource, GlobalBindingsResource,
    MaterialComponent, MeshComponent, RENDER_ORDER_TRANSPARENT_COMPOSITE, RenderFeature, RenderPassCtx, RenderPassWriter,
    SceneNormalsTextureResource, ShadowAtlasResource, SsrTexturesResource, TransparentTargetResource, create_engine_shader,
    draw_fullscreen, fullscreen_shader_desc, texture_binding,
};

#[repr(C, u8)]
#[derive(Clone)]
enum ItemMesh {
    Instanced(AssetHandle<StandardMesh>),
    Batched(*const BatchedMeshComponent),
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct MaterialItem {
    template: AssetHandle<MaterialTemplate>,
    material: AssetHandle<Material>,
    mesh: ItemMesh,
    local_to_world: Mat4<f32>,
    in_camera: bool,
    // bit `i` set: drawn into shadow view `i`
    shadow_mask: u64,
}

impl MaterialItem {
    fn instanced_mesh(&self) -> Option<&AssetHandle<StandardMesh>> {
        match &self.mesh {
            ItemMesh::Instanced(mesh) => Some(mesh),
            ItemMesh::Batched(_) => None,
        }
    }

    fn cmp_sort_key(&self, other: &Self) -> Ordering {
        let mesh_key = |item: &Self| match &item.mesh {
            ItemMesh::Instanced(mesh) => (0u8, mesh.clone()),
            ItemMesh::Batched(_) => (1u8, AssetHandle::EMPTY),
        };

        (&self.template, &self.material, mesh_key(self)).cmp(&(&other.template, &other.material, mesh_key(other)))
    }
}

#[repr(C, u8)]
#[derive(Clone)]
enum DrawGeometry {
    Instanced {
        mesh: AssetHandle<StandardMesh>,
        instance_start: u32,
        instance_count: u32,
        // per shadow view `(instance_start, instance_count)` in `shadow_ranges`, empty when nothing casts
        shadow_ranges_start: u32,
        shadow_ranges_count: u32,
    },
    Batched {
        vertex_start: u32,
        vertex_count: u32,
    },
}

#[repr(C)]
#[derive(Clone)]
struct MaterialDraw {
    order: u64,
    template: AssetHandle<MaterialTemplate>,
    material: AssetHandle<Material>,
    pass: u32,
    geometry: DrawGeometry,
}

#[repr(C)]
#[derive(Default)]
pub struct MaterialRenderFeature {
    items: FfiVec<MaterialItem>,
    geometry: FfiVec<(u32, u32, DrawGeometry)>,
    shadow_frusta: FfiVec<Frustum>,
    shadow_ranges: FfiVec<(u32, u32)>,
    matrices: FfiVec<Mat4<f32>>,
    draws: FfiVec<MaterialDraw>,
    logged: FfiIndexSet<AssetHandle<Material>>,
    instances: FfiOption<StandardInstanceBuffer>,
    identity_instance: FfiOption<StandardInstanceBuffer>,
    batched_vertices: FfiOption<GpuVertexBuffer>,
    transparent_composite: FfiOption<GpuShader>,
}

// Safety. The batched mesh pointers in `items` are only dereferenced during `add_passes`, and the vec is cleared before reuse.
unsafe impl Send for MaterialRenderFeature {}
unsafe impl Sync for MaterialRenderFeature {}

impl MaterialRenderFeature {
    fn gather(&mut self, res: &ResourcesHolderMut, entities: &EntitiesHolderMut) {
        let Some(materials) = res.get::<AssetStorageResource<Material>>() else {
            return;
        };
        let templates = res.get::<AssetStorageResource<MaterialTemplate>>();
        let meshes = res.get::<AssetStorageResource<StandardMesh>>();

        let camera = res
            .get::<CurrentRenderingResource>()
            .map(|current| Frustum::from_world_to_clip(&current.world_to_clip()));

        self.shadow_frusta.clear();
        if let Some(atlas) = res.get::<ShadowAtlasResource>() {
            for view in atlas.views() {
                self.shadow_frusta.push(Frustum::from_world_to_clip(&view.world_to_clip));
            }
        }
        let all_shadow_views = match self.shadow_frusta.len() {
            64.. => u64::MAX,
            count => (1u64 << count) - 1,
        };

        // todo: reusable allocation-free ECS query
        let query = entities.query_filtered::<(
            Option<&MeshComponent>,
            Option<&BatchedMeshComponent>,
            &MaterialComponent,
            Option<&GlobalTransform>,
            Option<&GlobalDisableableComponent>,
        ), OrFilter<(WithFilter<MeshComponent>, WithFilter<BatchedMeshComponent>)>>();

        for (mesh, batched, material, transform, disableable) in query.iter() {
            if disableable.is_some_and(|disableable| disableable.is_disabled) {
                continue;
            }

            let Some(asset) = materials.get(&material.material) else {
                continue;
            };

            let local_to_world = transform.map_or(Mat4::IDENTITY, |transform| {
                transform.scale_rotation.into_4x4_with_offset(transform.position)
            });
            let template = templates.and_then(|templates| templates.get(&asset.template));
            let is_culled = template.is_some_and(|template| !template.skip_frustum_culling);
            let shadow_views = match template.is_some_and(has_shadow_pass) {
                true => all_shadow_views,
                false => 0,
            };

            let mut push = |mesh, in_camera, shadow_mask| {
                self.items.push(MaterialItem {
                    template: asset.template.clone(),
                    material: material.material.clone(),
                    mesh,
                    local_to_world,
                    in_camera,
                    shadow_mask,
                })
            };

            if let Some(mesh) = mesh {
                let bounds = meshes
                    .and_then(|meshes| meshes.get(&mesh.mesh))
                    .filter(|_| is_culled)
                    .map(|asset| asset.aabb().transformed(&local_to_world));

                let (in_camera, shadow_mask) = visibility(bounds, camera.as_ref(), &self.shadow_frusta, shadow_views);

                if in_camera || shadow_mask != 0 {
                    push(ItemMesh::Instanced(mesh.mesh.clone()), in_camera, shadow_mask);
                }
            }
            if let Some(batched) = batched {
                push(ItemMesh::Batched(batched), true, shadow_views);
            }
        }

        self.items.sort_unstable_by(MaterialItem::cmp_sort_key);
    }

    fn upload_material_changes(res: &mut ResourcesHolderMut, items: &[MaterialItem], render_api: &RenderApiResource) {
        let Some(materials) = res.get_mut::<AssetStorageResource<Material>>() else {
            return;
        };

        for chunk in items.chunk_by(|a, b| a.material == b.material) {
            if let Some(material) = materials.get_mut(&chunk[0].material) {
                render_api.upload_material_changes(material);
            }
        }
    }

    fn upload_geometry(&mut self, render_api: &RenderApiResource) {
        let core = render_api.core();
        self.geometry.clear();

        self.shadow_ranges.clear();
        let views_count = self.shadow_frusta.len() as u32;

        let instance_count = self
            .items
            .iter()
            .filter(|item| item.instanced_mesh().is_some())
            .map(|item| item.in_camera as u32 + item.shadow_mask.count_ones())
            .sum::<u32>();
        let vertex_count = self
            .items
            .iter()
            .map(|item| match item.mesh {
                ItemMesh::Instanced(_) => 0,
                ItemMesh::Batched(batched) => unsafe { (*batched).indices.len() as usize },
            })
            .sum::<usize>();

        let instance_capacity = self.instances.as_ref().map_or(0, StandardInstanceBuffer::len);
        if instance_capacity < instance_count {
            self.instances = FfiOption::Some(render_api.create_instance_buffer(instance_count.max(instance_capacity * 2).max(64)));
        }

        let vertex_capacity = self.batched_vertices.as_ref().map_or(0, GpuVertexBuffer::len);
        if vertex_capacity < vertex_count {
            let len = vertex_count.max(vertex_capacity * 2).max(1024);
            self.batched_vertices =
                FfiOption::Some(core.create_vertex_buffer(Some("batched vertices"), len, StandardVertex::desc()));
        }

        if self.identity_instance.is_none() {
            let identity = render_api.create_instance_buffer(1);
            render_api.write_instances(&identity, 0, &[Mat4::IDENTITY]);
            self.identity_instance = FfiOption::Some(identity);
        }

        let mut instance_start = 0;
        let mut vertex_start = 0;
        let mut index = 0;

        for chunk in self.items.chunk_by(|a, b| a.material == b.material) {
            let batched_start = chunk.partition_point(|item| item.instanced_mesh().is_some());

            for instances in chunk[..batched_start].chunk_by(|a, b| a.instanced_mesh() == b.instanced_mesh()) {
                let camera_start = instance_start;
                let camera_count = write_instances_filtered(
                    render_api,
                    &self.instances,
                    &mut self.matrices,
                    instances,
                    instance_start,
                    |item| item.in_camera,
                );
                instance_start += camera_count;

                let shadow_ranges_start = self.shadow_ranges.len() as u32;
                let casts = instances.iter().any(|item| item.shadow_mask != 0);
                let shadow_ranges_count = if casts { views_count } else { 0 };

                for view in 0..shadow_ranges_count {
                    let count = write_instances_filtered(
                        render_api,
                        &self.instances,
                        &mut self.matrices,
                        instances,
                        instance_start,
                        |item| item.shadow_mask & (1 << view) != 0,
                    );
                    self.shadow_ranges.push((instance_start, count));
                    instance_start += count;
                }

                let range = (index as u32, (index + instances.len()) as u32);
                self.geometry.push((
                    range.0,
                    range.1,
                    DrawGeometry::Instanced {
                        mesh: instances[0].instanced_mesh().unwrap().clone(),
                        instance_start: camera_start,
                        instance_count: camera_count,
                        shadow_ranges_start,
                        shadow_ranges_count,
                    },
                ));

                index += instances.len();
            }

            let batched = &chunk[batched_start..];

            if !batched.is_empty() {
                let start = vertex_start;

                for item in batched {
                    let ItemMesh::Batched(mesh) = item.mesh else {
                        continue;
                    };

                    if let FfiOption::Some(buffer) = &self.batched_vertices {
                        vertex_start += unsafe { write_batched_mesh(core, buffer, &*mesh, &item.local_to_world, vertex_start) };
                    }
                }

                self.geometry.push((
                    index as u32,
                    (index + batched.len()) as u32,
                    DrawGeometry::Batched {
                        vertex_start: start as u32,
                        vertex_count: (vertex_start - start) as u32,
                    },
                ));

                index += batched.len();
            }
        }
    }

    fn collect_draws(&mut self, res: &ResourcesHolderMut, has_render_api: bool) -> bool {
        self.draws.clear();

        let (Some(materials), Some(templates)) = (
            res.get::<AssetStorageResource<Material>>(),
            res.get::<AssetStorageResource<MaterialTemplate>>(),
        ) else {
            return false;
        };

        let mut has_transparent = false;
        let mut geometry_index = 0;
        let mut item_index = 0;

        for chunk in self.items.chunk_by(|a, b| a.material == b.material) {
            let chunk_end = (item_index + chunk.len()) as u32;
            item_index += chunk.len();

            let geometry_start = geometry_index;
            while geometry_index < self.geometry.len() as usize && self.geometry[geometry_index as u64].1 <= chunk_end {
                geometry_index += 1;
            }
            let geometry = &self.geometry.as_slice()[geometry_start..geometry_index];

            let handle = &chunk[0].material;
            let material = materials.get(handle).unwrap();
            let template = templates.get(&material.template);

            let Some(template) = template.filter(|_| material.gpu().is_built()) else {
                if self.logged.insert(handle.clone()) {
                    match template {
                        None => eprintln!("material {handle:?} skipped: its template is not loaded"),
                        Some(_) if has_render_api => eprintln!("material {handle:?} skipped: its GPU data was never built"),
                        Some(_) => {}
                    }
                }
                continue;
            };

            for (pass_index, pass) in template.passes.iter().enumerate() {
                if material.gpu().pass(pass_index).is_none() {
                    continue;
                }

                has_transparent |= pass.target == MaterialPassTarget::Transparent;

                for (_, _, geometry) in geometry {
                    self.draws.push(MaterialDraw {
                        order: pass.order,
                        template: material.template.clone(),
                        material: handle.clone(),
                        pass: pass_index as u32,
                        geometry: geometry.clone(),
                    });
                }
            }
        }

        self.draws.sort_by_key(|draw| draw.order);

        has_transparent
    }

    fn draw_order(&mut self, order: u64, ctx: &mut RenderPassCtx) {
        let render_api = ctx.render_api;
        let core = render_api.core();
        let resources = ctx.world.resources();

        let (
            Some(current),
            Some(depth),
            Some(transparent),
            Some(globals),
            Some(materials),
            Some(templates),
            Some(meshes),
            Some(textures),
            Some(atlas),
            Some(normals),
            Some(ambient_occlusion),
            Some(environment),
            Some(ssr),
        ) = (
            resources.get::<CurrentRenderingResource>(),
            resources.get::<DepthTextureResource>(),
            resources.get::<TransparentTargetResource>(),
            resources.get::<GlobalBindingsResource>(),
            resources.get::<AssetStorageResource<Material>>(),
            resources.get::<AssetStorageResource<MaterialTemplate>>(),
            resources.get::<AssetStorageResource<StandardMesh>>(),
            resources.get::<AssetStorageResource<StandardTexture>>(),
            resources.get::<ShadowAtlasResource>(),
            resources.get::<SceneNormalsTextureResource>(),
            resources.get::<AmbientOcclusionTextureResource>(),
            resources.get::<EnvironmentTextureResource>(),
            resources.get::<SsrTexturesResource>(),
        )
        else {
            return;
        };

        let start = self.draws.partition_point(|draw| draw.order < order);
        let end = start + self.draws.as_slice()[start..].partition_point(|draw| draw.order == order);

        for draw in &self.draws.as_slice()[start..end] {
            let (Some(material), Some(template)) = (materials.get(&draw.material), templates.get(&draw.template)) else {
                continue;
            };
            let (Some(template_pass), Some(gpu_pass)) =
                (template.passes.get(draw.pass as u64), material.gpu().pass(draw.pass as usize))
            else {
                continue;
            };

            let shader = gpu_pass.shader();
            let is_shadow = template_pass.target == MaterialPassTarget::Shadow;
            let target = match template_pass.target {
                MaterialPassTarget::Main => Some(current.target.texture()),
                MaterialPassTarget::Transparent => Some(transparent.texture.texture()),
                MaterialPassTarget::Shadow => None,
                MaterialPassTarget::DepthNormals => Some(normals.texture.texture()),
            };
            let is_depth_normals = template_pass.target == MaterialPassTarget::DepthNormals;
            let label = match template_pass.target {
                MaterialPassTarget::Main => "material main",
                MaterialPassTarget::Transparent => "material transparent",
                MaterialPassTarget::Shadow => "material shadow",
                MaterialPassTarget::DepthNormals => "material depth normals",
            };
            let depth_view = match is_shadow {
                true => atlas.texture(),
                false => depth.texture.texture(),
            };
            let view_count = match is_shadow {
                true => atlas.views().len(),
                false => 1,
            };

            for view_index in 0..view_count {
                // todo: avoid allocating the bind groups per draw
                let mut groups = vec![Vec::new(); shader.bind_group_layouts().len().max(1)];

                if is_shadow {
                    groups[0].push((0, GpuBinding::Buffer(atlas.caster_uniform(view_index))));
                } else {
                    groups[0].push((0, GpuBinding::Buffer(globals.bindings.uniform_buffer())));
                    groups[0].push((1, GpuBinding::Buffer(globals.bindings.lights_buffer())));
                    groups[0].push((2, GpuBinding::Buffer(globals.bindings.shadow_views_buffer())));
                    groups[0].push((3, texture_binding(atlas.texture())));
                    groups[0].push((4, GpuBinding::Sampler(atlas.sampler())));
                    groups[0].push((5, GpuBinding::Buffer(globals.bindings.cluster_grid_buffer())));
                    groups[0].push((6, GpuBinding::Buffer(globals.bindings.cluster_indices_buffer())));

                    // the depth-normals pass renders into the scene normals
                    if !is_depth_normals {
                        groups[0].push((7, texture_binding(normals.texture.texture())));
                    }

                    groups[0].push((8, texture_binding(&ambient_occlusion.texture)));
                    groups[0].push((9, texture_binding(&environment.texture)));
                    groups[0].push((10, GpuBinding::Sampler(&environment.sampler)));
                    groups[0].push((11, GpuBinding::Buffer(globals.bindings.ssr_buffer())));
                    groups[0].push((12, texture_binding(&ssr.depth)));
                    groups[0].push((13, texture_binding(&ssr.history)));
                    groups[0].push((14, GpuBinding::Sampler(&ssr.sampler)));
                }

                for uniform in gpu_pass.uniforms() {
                    if let Some(group) = groups.get_mut(uniform.group()) {
                        group.push((uniform.binding(), GpuBinding::Buffer(uniform.buffer())));
                    }
                }

                for texture in template_pass.textures.iter() {
                    let Some(group) = groups.get_mut(texture.group) else {
                        continue;
                    };

                    let resolved = material
                        .data
                        .get_texture(texture.input.as_str())
                        .and_then(|handle| textures.get(handle))
                        .unwrap_or_else(|| {
                            let fallback = template
                                .input(texture.input.as_str())
                                .map(|input| input.texture_fallback)
                                .unwrap_or_default();
                            render_api.fallback_texture(fallback)
                        });

                    group.push((texture.texture_binding, texture_binding(resolved.texture())));
                    if let FfiOption::Some(sampler_binding) = texture.sampler_binding {
                        group.push((sampler_binding, GpuBinding::Sampler(resolved.sampler())));
                    }
                }

                let group_refs = groups.iter().map(Vec::as_slice).collect::<Vec<_>>();

                let pass = |vertex_buffers| GpuRenderPass {
                    label: Some(label),
                    shader,
                    target: target.map(|view| GpuRenderPassTarget {
                        view,
                        mip_level: 0,
                        ops: GpuPassOps::Load,
                    }),
                    depth: Some(GpuRenderPassDepth {
                        view: depth_view,
                        mip_level: 0,
                        ops: GpuPassOps::Load,
                    }),
                    bind_groups: &group_refs,
                    vertex_buffers,
                    viewport: is_shadow.then(|| atlas.viewport(view_index)),
                    scissor: None,
                };

                match &draw.geometry {
                    DrawGeometry::Instanced {
                        mesh,
                        instance_start,
                        instance_count,
                        shadow_ranges_start,
                        shadow_ranges_count,
                    } => {
                        let (Some(mesh), FfiOption::Some(instances)) = (meshes.get(mesh), &self.instances) else {
                            continue;
                        };

                        let (instance_start, instance_count) = match is_shadow {
                            false => (*instance_start, *instance_count),
                            true if (view_index as u32) < *shadow_ranges_count => {
                                self.shadow_ranges[(*shadow_ranges_start + view_index as u32) as u64]
                            }
                            true => (0, 0),
                        };

                        if mesh.indices_count() == 0 || instance_count == 0 {
                            continue;
                        }

                        let [instance_start, instance_count] = instances.clamp_range(instance_start, instance_count);

                        core.draw_indexed(
                            &pass(&[mesh.vertex_buffer(), instances.buffer()]),
                            mesh.index_buffer(),
                            0..mesh.indices_count(),
                            instance_start as usize..(instance_start + instance_count) as usize,
                        );
                    }
                    DrawGeometry::Batched {
                        vertex_start,
                        vertex_count,
                    } => {
                        let (FfiOption::Some(vertices), FfiOption::Some(identity)) =
                            (&self.batched_vertices, &self.identity_instance)
                        else {
                            continue;
                        };

                        if *vertex_count == 0 {
                            continue;
                        }

                        core.draw(
                            &pass(&[vertices, identity.buffer()]),
                            *vertex_start as usize..(*vertex_start + *vertex_count) as usize,
                            0..1,
                        );
                    }
                }
            }
        }
    }

    fn composite_transparent(&mut self, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let resources = ctx.world.resources();

        let (Some(current), Some(transparent)) = (
            resources.get::<CurrentRenderingResource>(),
            resources.get::<TransparentTargetResource>(),
        ) else {
            return;
        };

        if self.transparent_composite.is_none() {
            self.transparent_composite = FfiOption::Some(create_engine_shader(
                core,
                "shader_transparent_final.wgsl",
                include_str!("./assets/shader_transparent_final.wgsl"),
                GpuShaderDesc {
                    write_mask: GpuColorWrites::Color,
                    ..fullscreen_shader_desc(GpuBlendState::ALPHA_BLENDING)
                },
            ));
        }

        let FfiOption::Some(shader) = &self.transparent_composite else {
            return;
        };

        draw_fullscreen(
            core,
            "transparent composite",
            shader,
            current.target.texture(),
            GpuPassOps::Load,
            &[
                (0, texture_binding(transparent.texture.texture())),
                (1, GpuBinding::Sampler(transparent.texture.sampler())),
            ],
            None,
            0..1,
        );
    }

    #[cfg(test)]
    pub(crate) fn items(&self) -> &[MaterialItem] {
        &self.items
    }

    #[cfg(test)]
    pub(crate) fn logged(&self) -> &FfiIndexSet<AssetHandle<Material>> {
        &self.logged
    }
}

// (in camera, shadow view mask); unbounded items are visible everywhere
fn visibility(bounds: Option<Aabb>, camera: Option<&Frustum>, shadow_frusta: &[Frustum], shadow_views: u64) -> (bool, u64) {
    let Some(bounds) = bounds else {
        return (true, shadow_views);
    };

    let in_camera = camera.is_none_or(|camera| camera.intersects_aabb(&bounds));
    let shadow_mask = match shadow_views {
        0 => 0,
        _ => shadow_frusta
            .iter()
            .enumerate()
            .filter(|(_, frustum)| frustum.intersects_aabb(&bounds))
            .fold(0u64, |mask, (view, _)| mask | 1 << view),
    };

    (in_camera, shadow_mask & shadow_views)
}

fn has_shadow_pass(template: &MaterialTemplate) -> bool {
    template.passes.iter().any(|pass| pass.target == MaterialPassTarget::Shadow)
}

fn write_instances_filtered(
    render_api: &RenderApiResource,
    buffer: &FfiOption<StandardInstanceBuffer>,
    matrices: &mut FfiVec<Mat4<f32>>,
    items: &[MaterialItem],
    start: u32,
    is_visible: impl Fn(&MaterialItem) -> bool,
) -> u32 {
    matrices.clear();
    for item in items.iter().filter(|item| is_visible(item)) {
        matrices.push(item.local_to_world);
    }

    if let FfiOption::Some(buffer) = buffer
        && !matrices.is_empty()
    {
        render_api.write_instances(buffer, start, matrices);
    }

    matrices.len() as u32
}

// returns how many vertices were written
fn write_batched_mesh(
    core: &RenderApi,
    buffer: &GpuVertexBuffer,
    batched_mesh: &BatchedMeshComponent,
    local_to_world: &Mat4<f32>,
    start: usize,
) -> usize {
    let indices = batched_mesh.indices.as_slice();
    let count = indices.len().min(buffer.len().saturating_sub(start));

    write_buffer_elements_with(core, buffer, start, count, |i| {
        let mut vertex = batched_mesh.vertices[indices[i] as u64];

        vertex.position = local_to_world
            .mul_with_projection(Vec3::from_array(vertex.position))
            .into_array();
        vertex.normal = local_to_world
            .mul_with_projection_as_dir(Vec3::from_array(vertex.normal))
            .into_array();
        let [tx, ty, tz, handedness] = vertex.tangent;
        let [tx, ty, tz] = local_to_world.mul_with_projection_as_dir(Vec3::new(tx, ty, tz)).into_array();
        vertex.tangent = [tx, ty, tz, handedness];

        vertex
    });

    count
}

impl RenderFeature for MaterialRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        self.items.clear();

        let (mut res, entities, _) = world.into_tuple_mut();
        self.gather(&res, &entities);

        let render_api = res.get::<RenderApiResource>().map(RenderApiResource::clone);

        if let Some(render_api) = &render_api {
            Self::upload_material_changes(&mut res, &self.items, render_api);
            self.upload_geometry(render_api);
        } else {
            self.geometry.clear();
        }

        let has_transparent = self.collect_draws(&res, render_api.is_some());

        self.items.clear();

        if render_api.is_none() {
            return;
        }

        for i in 0..self.draws.len() as usize {
            let order = self.draws[i as u64].order;

            if i == 0 || self.draws[(i - 1) as u64].order != order {
                passes.add(order, move |this, ctx| this.draw_order(order, ctx));
            }
        }

        if has_transparent {
            passes.add(RENDER_ORDER_TRANSPARENT_COMPOSITE, |this, ctx| {
                this.composite_transparent(ctx)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
    use aphyn_ecs::*;
    use aphyn_render::{Material, MaterialData, MaterialTemplate};
    use aphyn_transform::GlobalTransform;
    use aphyn_utils::index_version_collection::VersionIndex;

    use super::*;
    use crate::*;

    fn handle<T>(index: u64) -> AssetHandle<T> {
        AssetHandle::new(VersionIndex { index, version: 1 })
    }

    fn spawn(world: &mut WorldData, material: AssetHandle<Material>, instanced: bool, batched: bool) {
        let mut data = world.as_mut();
        let mut entities = data.entities_mut();
        let e = entities.create_entity();
        if instanced {
            entities.add_component(e, MeshComponent { mesh: handle(1) }).ok().unwrap();
        }
        if batched {
            entities.add_component(e, BatchedMeshComponent::default()).ok().unwrap();
        }
        entities.add_component(e, MaterialComponent { material }).ok().unwrap();
        entities.add_component(e, GlobalTransform::IDENTITY).ok().unwrap();
    }

    fn feature(render: &RenderFeatureRegistry) -> &MaterialRenderFeature {
        render.feature::<MaterialRenderFeature>(RENDER_FEATURE_MATERIAL)
    }

    fn item(template: u64, material: u64, mesh: Option<u64>, batched: &BatchedMeshComponent) -> MaterialItem {
        MaterialItem {
            template: handle(template),
            material: handle(material),
            mesh: match mesh {
                Some(mesh) => ItemMesh::Instanced(handle(mesh)),
                None => ItemMesh::Batched(batched),
            },
            local_to_world: Mat4::IDENTITY,
            in_camera: true,
            shadow_mask: 0,
        }
    }

    #[test]
    fn items_sort_by_template_material_kind_mesh() {
        let b = &BatchedMeshComponent::default();
        let mut items = [
            item(1, 5, None, b),
            item(1, 5, Some(11), b),
            item(0, 6, Some(10), b),
            item(1, 4, Some(10), b),
            item(1, 5, Some(10), b),
            item(0, 6, None, b),
        ];

        items.sort_unstable_by(MaterialItem::cmp_sort_key);

        let keys: Vec<_> = items
            .iter()
            .map(|item| {
                (
                    item.template.index().index,
                    item.material.index().index,
                    item.instanced_mesh().map(|mesh| mesh.index().index),
                )
            })
            .collect();
        assert_eq!(
            keys,
            [
                (0, 6, Some(10)),
                (0, 6, None),
                (1, 4, Some(10)),
                (1, 5, Some(10)),
                (1, 5, Some(11)),
                (1, 5, None),
            ]
        );
    }

    #[test]
    fn materials_without_template_or_gpu_are_logged_once() {
        let mut templates = AssetStorageResource::<MaterialTemplate>::new();
        let template = templates.insert(MaterialTemplate::default());

        let mut materials = AssetStorageResource::<Material>::new();
        let unbuilt = materials.insert(Material::new(template, MaterialData::new()));
        let orphan = materials.insert(Material::new(handle(42), MaterialData::new()));

        let mut world = WorldData::default();
        {
            let mut data = world.as_mut();
            let mut resources = data.resources_mut();
            resources.insert(templates);
            resources.insert(materials);
        }
        spawn(&mut world, handle(77), true, false);
        spawn(&mut world, unbuilt.clone(), true, true);
        spawn(&mut world, orphan.clone(), false, true);
        spawn(&mut world, orphan.clone(), true, false);

        let mut render = RenderFeatureRegistry::default();
        assert!(render.add_feature(RENDER_FEATURE_MATERIAL, MaterialRenderFeature::default()));

        for _ in 0..2 {
            render.collect_passes(&mut world, ThreadPoolAccess::single_threaded(), None);
            assert!(render.pass_entries().is_empty());
            assert!(feature(&render).items().is_empty());
        }

        let logged = feature(&render).logged();
        assert_eq!(logged.len(), 2);
        assert!(logged.contains(&unbuilt));
        assert!(logged.contains(&orphan));
    }

    #[test]
    fn visibility_culls_per_frustum() {
        use aphyn_math::{Aabb, Frustum, Vec3, orthographic_proj_matrix, perspective_proj_matrix};

        let camera = Frustum::from_world_to_clip(&perspective_proj_matrix(1.5, 0.1, 100.0, 1.0));
        // a light box around the origin, reaching behind the camera
        let light =
            Frustum::from_world_to_clip(&(orthographic_proj_matrix(20.0, 0.0, 40.0, 1.0) * Mat4::offset(Vec3::new(0.0, 0.0, 20.0))));
        let far_light =
            Frustum::from_world_to_clip(&(orthographic_proj_matrix(2.0, 0.0, 2.0, 1.0) * Mat4::offset(Vec3::new(500.0, 0.0, 0.0))));
        let shadows = [light, far_light];
        let cube = |z: f32| Some(Aabb::new(Vec3::new(-0.5, -0.5, z - 0.5), Vec3::new(0.5, 0.5, z + 0.5)));

        assert_eq!(visibility(cube(5.0), Some(&camera), &shadows, 0b11), (true, 0b01));
        assert_eq!(
            visibility(cube(-5.0), Some(&camera), &shadows, 0b11),
            (false, 0b01),
            "off-screen caster"
        );
        assert_eq!(visibility(cube(-50.0), Some(&camera), &shadows, 0b11), (false, 0));
        assert_eq!(
            visibility(cube(-5.0), Some(&camera), &shadows, 0),
            (false, 0),
            "no shadow pass"
        );
        assert_eq!(
            visibility(cube(-5.0), None, &shadows, 0b11),
            (true, 0b01),
            "no camera, no camera culling"
        );
        assert_eq!(visibility(None, Some(&camera), &shadows, 0b11), (true, 0b11), "unbounded");
    }

    #[test]
    fn unloaded_meshes_are_not_culled() {
        let mut templates = AssetStorageResource::<MaterialTemplate>::new();
        let template = templates.insert(MaterialTemplate::default());
        let mut materials = AssetStorageResource::<Material>::new();
        let material = materials.insert(Material::new(template, MaterialData::new()));

        let mut world = WorldData::default();
        {
            let mut data = world.as_mut();
            let mut resources = data.resources_mut();
            resources.insert(templates);
            resources.insert(materials);
        }
        spawn(&mut world, material, true, true);

        let mut feature = MaterialRenderFeature::default();
        let (res, entities, _) = world.as_mut().into_tuple_mut();
        feature.gather(&res, &entities);

        assert_eq!(feature.items().len(), 2);
        assert!(feature.items().iter().all(|item| item.in_camera));
    }

    #[test]
    fn feature_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MaterialRenderFeature>();
    }
}
