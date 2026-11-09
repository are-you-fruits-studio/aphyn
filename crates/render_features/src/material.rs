use std::cmp::Ordering;

use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::{EntitiesHolderMut, OrFilter, ResourcesHolderMut, WithFilter, WorldDataMut};
use aphyn_ffi::{FfiIndexSet, FfiOption, FfiVec};
use aphyn_math::{Mat4, Vec3};
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
    BatchedMeshComponent, DepthTextureResource, GlobalBindingsResource, MaterialComponent, MeshComponent,
    RENDER_ORDER_TRANSPARENT_COMPOSITE, RenderFeature, RenderPassCtx, RenderPassWriter, TransparentTargetResource,
    create_engine_shader, draw_fullscreen, fullscreen_shader_desc, texture_binding,
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
            let mut push = |mesh| {
                self.items.push(MaterialItem {
                    template: asset.template.clone(),
                    material: material.material.clone(),
                    mesh,
                    local_to_world,
                })
            };

            if let Some(mesh) = mesh {
                push(ItemMesh::Instanced(mesh.mesh.clone()));
            }
            if let Some(batched) = batched {
                push(ItemMesh::Batched(batched));
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

        let instance_count = self.items.iter().filter(|item| item.instanced_mesh().is_some()).count() as u32;
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
                let count = instances.len() as u32;

                if let FfiOption::Some(buffer) = &self.instances {
                    render_api.write_instances_with(buffer, instance_start, count, |i| instances[i as usize].local_to_world);
                }

                let range = (index as u32, (index + instances.len()) as u32);
                self.geometry.push((
                    range.0,
                    range.1,
                    DrawGeometry::Instanced {
                        mesh: instances[0].instanced_mesh().unwrap().clone(),
                        instance_start,
                        instance_count: count,
                    },
                ));

                instance_start += count;
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
        ) = (
            resources.get::<CurrentRenderingResource>(),
            resources.get::<DepthTextureResource>(),
            resources.get::<TransparentTargetResource>(),
            resources.get::<GlobalBindingsResource>(),
            resources.get::<AssetStorageResource<Material>>(),
            resources.get::<AssetStorageResource<MaterialTemplate>>(),
            resources.get::<AssetStorageResource<StandardMesh>>(),
            resources.get::<AssetStorageResource<StandardTexture>>(),
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
            let target = match template_pass.target {
                MaterialPassTarget::Main => current.target.texture(),
                MaterialPassTarget::Transparent => transparent.texture.texture(),
            };

            // todo: avoid allocating the bind groups per draw
            let mut groups = vec![Vec::new(); shader.bind_group_layouts().len().max(1)];
            groups[0].push((0, GpuBinding::Buffer(globals.bindings.uniform_buffer())));
            groups[0].push((1, GpuBinding::Buffer(globals.bindings.lights_buffer())));

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
                shader,
                target: Some(GpuRenderPassTarget {
                    view: target,
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                depth: Some(GpuRenderPassDepth {
                    view: depth.texture.texture(),
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                bind_groups: &group_refs,
                vertex_buffers,
                viewport: None,
                scissor: None,
            };

            match &draw.geometry {
                DrawGeometry::Instanced {
                    mesh,
                    instance_start,
                    instance_count,
                } => {
                    let (Some(mesh), FfiOption::Some(instances)) = (meshes.get(mesh), &self.instances) else {
                        continue;
                    };

                    if mesh.indices_count() == 0 {
                        continue;
                    }

                    let [instance_start, instance_count] = instances.clamp_range(*instance_start, *instance_count);

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
                    let (FfiOption::Some(vertices), FfiOption::Some(identity)) = (&self.batched_vertices, &self.identity_instance)
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

// returns how many vertices were written
fn write_batched_mesh(
    core: &RenderApi,
    buffer: &GpuVertexBuffer,
    batched_mesh: &BatchedMeshComponent,
    local_to_world: &Mat4<f32>,
    start: usize,
) -> usize {
    // todo: to Image/Text components, the gamma curve should not be here with batching
    let srgb_to_linear = |x: f32| x.powf(2.2);

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
        vertex.color = [
            srgb_to_linear(vertex.color[0]),
            srgb_to_linear(vertex.color[1]),
            srgb_to_linear(vertex.color[2]),
            vertex.color[3],
        ];

        vertex
    });

    count
}

impl RenderFeature for MaterialRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
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
            render.collect_passes(&mut world);
            assert!(render.pass_entries().is_empty());
            assert!(feature(&render).items().is_empty());
        }

        let logged = feature(&render).logged();
        assert_eq!(logged.len(), 2);
        assert!(logged.contains(&unbuilt));
        assert!(logged.contains(&orphan));
    }

    #[test]
    fn feature_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MaterialRenderFeature>();
    }
}
