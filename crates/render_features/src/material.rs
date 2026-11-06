use std::ffi::c_void;

use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::{Component, Resource, TypesRegistryAccessFfi, TypesRegistryCache, WorldDataMut, WorldDataRef, WorldDataUnsafeFfi};
use aphyn_ffi::{FfiDroppable, FfiIndexMap, FfiIndexSet, FfiString, FfiVec};
use aphyn_math::Mat4;
use aphyn_render::{Material, MaterialData, RenderApiResource, StandardInstanceBuffer, StandardMesh};
use aphyn_serialization::*;
use aphyn_transform::GlobalTransform;

use crate::{RenderPassCtx, RenderPassEntry, RenderPassWriter, RenderSlot, RenderSlotVtable, slot_world};

pub trait MaterialTemplate: Send + Sync + 'static {
    type Pass: Send + Sync + 'static;

    fn default_data(&self) -> MaterialData;
    fn add_passes(&mut self, world: WorldDataMut, batches: &[MaterialBatch], passes: &mut RenderPassWriter<Self::Pass>);
    fn run_pass(&mut self, pass: &Self::Pass, ctx: &mut RenderPassCtx);
}

#[repr(C)]
#[derive(Default, Component, Clone, Serializable)]
pub struct MaterialComponent {
    pub material: AssetHandle<Material>,
}

#[repr(C)]
#[derive(Default, Component, Clone, Serializable)]
pub struct MeshComponent {
    pub mesh: AssetHandle<StandardMesh>,
}

#[repr(C)]
#[derive(Clone)]
pub struct MaterialBatch {
    pub material: AssetHandle<Material>,
    pub mesh: AssetHandle<StandardMesh>,
    pub instance_start: u32,
    pub instance_count: u32,
}

#[repr(C)]
#[derive(Resource)]
pub struct MaterialInstancesResource {
    pub buffer: StandardInstanceBuffer,
}

#[repr(C)]
pub(crate) struct GatheredInstance {
    template_slot: u32,
    material: AssetHandle<Material>,
    mesh: AssetHandle<StandardMesh>,
    local_to_world: Mat4<f32>,
}

fn next_instance_capacity(current: u32, needed: u32) -> u32 {
    if needed <= current {
        current
    } else {
        needed.max(current.saturating_mul(2)).max(64)
    }
}

fn build_batches(items: &mut [GatheredInstance], batches: &mut FfiVec<MaterialBatch>) {
    batches.clear();
    items.sort_unstable_by(|a, b| (a.template_slot, &a.material, &a.mesh).cmp(&(b.template_slot, &b.material, &b.mesh)));

    let mut start = 0;
    while start < items.len() {
        let first = &items[start];
        let count = items[start..]
            .iter()
            .take_while(|item| {
                item.template_slot == first.template_slot && item.material == first.material && item.mesh == first.mesh
            })
            .count();

        batches.push(MaterialBatch {
            material: first.material.clone(),
            mesh: first.mesh.clone(),
            instance_start: start as u32,
            instance_count: count as u32,
        });
        start += count;
    }
}

#[repr(C)]
#[derive(Default)]
pub(crate) struct MaterialStep {
    gathered: FfiVec<GatheredInstance>,
    batches: FfiVec<MaterialBatch>,
    logged: FfiIndexSet<AssetHandle<Material>>,
}

impl MaterialStep {
    pub(crate) fn gather(&mut self, world: WorldDataRef, names: &FfiIndexMap<FfiString, u32>, slots: &FfiVec<RenderSlot>) {
        self.gathered.clear();
        self.batches.clear();

        let Some(materials) = world.resources().get::<AssetStorageResource<Material>>() else {
            return;
        };
        let entities = world.entities();
        // todo: reusable allocation-free ECS query
        let query = entities.query::<(&MeshComponent, &MaterialComponent, &GlobalTransform)>();

        for (mesh, material, transform) in query.iter() {
            let Some(asset) = materials.get(&material.material) else {
                continue;
            };
            let template_slot = names
                .get(asset.template.as_str())
                .copied()
                .filter(|&slot| slots[slot as u64].is_template());
            let Some(template_slot) = template_slot else {
                if self.logged.insert(material.material.clone()) {
                    eprintln!("unknown material template '{}'", asset.template.as_str());
                }
                continue;
            };

            self.gathered.push(GatheredInstance {
                template_slot,
                material: material.material.clone(),
                mesh: mesh.mesh.clone(),
                local_to_world: transform.scale_rotation.into_4x4_with_offset(transform.position),
            });
        }

        build_batches(&mut self.gathered, &mut self.batches);
    }

    pub(crate) fn write_instances(&self, world: WorldDataMut) {
        if self.gathered.is_empty() {
            return;
        }

        let mut resources = world.into_resources_mut();
        let Some(render_api) = resources.get_ptr::<RenderApiResource>() else {
            return;
        };
        let instances = resources.get_ptr::<MaterialInstancesResource>();
        let needed = u32::try_from(self.gathered.len()).expect("too many material instances");
        let write = |render_api: &RenderApiResource, buffer: &StandardInstanceBuffer| {
            render_api.write_instances_with(buffer, 0, needed, |i| self.gathered[i as u64].local_to_world);
        };

        // Safety. The pointers name two distinct resources and are not used after `insert`.
        unsafe {
            let render_api = &*render_api;
            match instances {
                Some(instances) if (*instances).buffer.len() >= needed => write(render_api, &(*instances).buffer),
                _ => {
                    let current = instances.map_or(0, |instances| (*instances).buffer.len());
                    let buffer = render_api.create_instance_buffer(next_instance_capacity(current, needed));
                    write(render_api, &buffer);
                    resources.insert(MaterialInstancesResource { buffer });
                }
            }
        }
    }

    pub(crate) fn next_template_batches(&self, slot: u32, cursor: &mut usize) -> &[MaterialBatch] {
        let start = *cursor;
        while *cursor < self.batches.len() as usize
            && self.gathered[self.batches[*cursor as u64].instance_start as u64].template_slot == slot
        {
            *cursor += 1;
        }
        &(*self.batches)[start..*cursor]
    }

    #[cfg(test)]
    pub(crate) fn gathered(&self) -> &FfiVec<GatheredInstance> {
        &self.gathered
    }

    #[cfg(test)]
    pub(crate) fn logged(&self) -> &FfiIndexSet<AssetHandle<Material>> {
        &self.logged
    }
}

struct TemplateSlotState<T: MaterialTemplate> {
    template: T,
    passes: FfiVec<T::Pass>,
    types: Option<TypesRegistryCache>,
}

impl RenderSlot {
    pub(crate) fn from_template<T: MaterialTemplate>(template: T) -> Self {
        unsafe extern "C-unwind" fn ffi_add_passes<T: MaterialTemplate>(
            this: *mut c_void,
            slot: u32,
            world: *mut WorldDataUnsafeFfi,
            types: *const TypesRegistryAccessFfi,
            entries: *mut FfiVec<RenderPassEntry>,
            batches: *const MaterialBatch,
            batches_len: u64,
        ) {
            unsafe {
                let TemplateSlotState {
                    template,
                    passes,
                    types: cache,
                } = &mut *(this as *mut TemplateSlotState<T>);
                let world = slot_world(cache, world, types);
                let batches = match batches_len {
                    0 => &[],
                    len => std::slice::from_raw_parts(batches, len as usize),
                };
                let mut writer = RenderPassWriter::new(slot, passes, &mut *entries);

                template.add_passes(world, batches, &mut writer);
            }
        }
        unsafe extern "C-unwind" fn ffi_run_pass<T: MaterialTemplate>(
            this: *mut c_void,
            index: u32,
            world: *mut WorldDataUnsafeFfi,
            types: *const TypesRegistryAccessFfi,
            render_api: *const RenderApiResource,
        ) {
            unsafe {
                let TemplateSlotState {
                    template,
                    passes,
                    types: cache,
                } = &mut *(this as *mut TemplateSlotState<T>);
                let mut ctx = RenderPassCtx {
                    render_api: &*render_api,
                    world: slot_world(cache, world, types),
                };

                template.run_pass(&passes[index as u64], &mut ctx);
            }
        }
        unsafe extern "C-unwind" fn ffi_clear_passes<T: MaterialTemplate>(this: *mut c_void) {
            unsafe {
                (*(this as *mut TemplateSlotState<T>)).passes.clear();
            }
        }
        unsafe extern "C-unwind" fn ffi_default_data<T: MaterialTemplate>(this: *mut c_void, out: *mut MaterialData) {
            unsafe {
                *out = (*(this as *const TemplateSlotState<T>)).template.default_data();
            }
        }

        Self {
            data: FfiDroppable::new(TemplateSlotState::<T> {
                template,
                passes: FfiVec::new(),
                types: None,
            }),
            vtable: &RenderSlotVtable {
                add_passes: ffi_add_passes::<T>,
                run_pass: ffi_run_pass::<T>,
                clear_passes: ffi_clear_passes::<T>,
                default_data: Some(ffi_default_data::<T>),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
    use aphyn_ecs::*;
    use aphyn_ffi::FfiVec;
    use aphyn_math::Mat4;
    use aphyn_render::{Material, MaterialData, MaterialDataValue, StandardMesh};
    use aphyn_transform::GlobalTransform;

    use super::*;
    use crate::*;

    struct TestTemplate {
        speed: f32,
    }

    impl MaterialTemplate for TestTemplate {
        type Pass = ();

        fn default_data(&self) -> MaterialData {
            let mut data = MaterialData::new();
            data.insert("speed", MaterialDataValue::F32(self.speed));
            data
        }

        fn add_passes(&mut self, _world: WorldDataMut, _batches: &[MaterialBatch], _passes: &mut RenderPassWriter<()>) {}

        fn run_pass(&mut self, _pass: &(), _ctx: &mut RenderPassCtx) {}
    }

    struct NoFeature;

    impl RenderFeature for NoFeature {
        type Pass = ();

        fn add_passes(&mut self, _world: WorldDataMut, _passes: &mut RenderPassWriter<()>) {}
        fn run_pass(&mut self, _pass: &(), _ctx: &mut RenderPassCtx) {}
    }

    fn handle<T>(index: u64) -> AssetHandle<T> {
        AssetHandle::new(aphyn_utils::index_version_collection::VersionIndex { index, version: 1 })
    }

    fn item(template_slot: u32, material: u64, mesh: u64) -> GatheredInstance {
        GatheredInstance {
            template_slot,
            material: handle(material),
            mesh: handle(mesh),
            local_to_world: Mat4::IDENTITY,
        }
    }

    fn world_with_materials(materials: AssetStorageResource<Material>) -> World {
        let mut world = WorldBuilder::new().build();
        world.data_mut().resources_mut().insert(materials);
        world
    }

    fn spawn(world: &mut World, material: AssetHandle<Material>) {
        let mut data = world.data_mut();
        let mut entities = data.entities_mut();
        let e = entities.create_entity();
        entities
            .add_component(
                e,
                MeshComponent {
                    mesh: AssetHandle::<StandardMesh>::EMPTY,
                },
            )
            .ok()
            .unwrap();
        entities.add_component(e, MaterialComponent { material }).ok().unwrap();
        entities.add_component(e, GlobalTransform::IDENTITY).ok().unwrap();
    }

    #[test]
    fn batches_group_by_template_material_mesh() {
        let (m1, m2, mesh_a, mesh_b) = (1, 2, 10, 11);
        let mut items = [
            item(1, m2, mesh_a),
            item(0, m1, mesh_a),
            item(1, m2, mesh_a),
            item(0, m1, mesh_b),
        ];
        let mut batches = FfiVec::new();

        build_batches(&mut items, &mut batches);

        let batches: Vec<_> = batches
            .iter()
            .map(|b| (b.material.clone(), b.mesh.clone(), b.instance_start, b.instance_count))
            .collect();
        assert_eq!(
            batches,
            [
                (handle(m1), handle(mesh_a), 0, 1),
                (handle(m1), handle(mesh_b), 1, 1),
                (handle(m2), handle(mesh_a), 2, 2),
            ]
        );
    }

    #[test]
    fn instance_capacity_grows_to_fit() {
        assert_eq!(next_instance_capacity(0, 3), 64);
        assert_eq!(next_instance_capacity(64, 64), 64);
        assert_eq!(next_instance_capacity(64, 65), 128);
        assert_eq!(next_instance_capacity(64, 1000), 1000);
    }

    #[test]
    fn unresolved_handles_are_skipped() {
        let mut materials = AssetStorageResource::<Material>::new();
        let valid = materials.insert(Material {
            template: "t".into(),
            data: MaterialData::new(),
        });
        let mut world = world_with_materials(materials);
        spawn(&mut world, handle(77));
        spawn(&mut world, valid.clone());

        let mut registry = RenderRegistry::default();
        assert!(registry.add_material_template("t", TestTemplate { speed: 0.0 }));

        registry.collect_passes(&mut world);

        let gathered: Vec<_> = registry.gathered().iter().map(|i| i.material.clone()).collect();
        assert_eq!(gathered, [valid]);
    }

    #[test]
    fn unknown_template_is_skipped_and_logged_once() {
        let mut materials = AssetStorageResource::<Material>::new();
        let missing = materials.insert(Material {
            template: "missing".into(),
            data: MaterialData::new(),
        });
        let mut world = world_with_materials(materials);
        spawn(&mut world, missing.clone());
        spawn(&mut world, missing.clone());

        let mut registry = RenderRegistry::default();
        assert!(registry.add_feature("missing_is_a_feature_not_a_template", NoFeature));

        for _ in 0..2 {
            registry.collect_passes(&mut world);
            assert!(registry.gathered().is_empty());
        }

        assert_eq!(registry.logged_materials().len(), 1);
        assert!(registry.logged_materials().contains(&missing));
    }

    #[test]
    fn feature_name_is_not_a_template() {
        let mut materials = AssetStorageResource::<Material>::new();
        let material = materials.insert(Material {
            template: "x".into(),
            data: MaterialData::new(),
        });
        let mut world = world_with_materials(materials);
        spawn(&mut world, material);

        let mut registry = RenderRegistry::default();
        registry.add_feature("x", NoFeature);

        registry.collect_passes(&mut world);

        assert!(registry.gathered().is_empty());
        assert!(registry.create_material("x").is_none());
    }

    #[test]
    fn create_material_uses_template_defaults() {
        let mut registry = RenderRegistry::default();
        registry.add_material_template("t", TestTemplate { speed: 0.3 });

        let material = registry.create_material("t").unwrap();
        assert_eq!(material.template.as_str(), "t");
        assert_eq!(material.data.get_f32("speed"), Some(&0.3));
        assert_eq!(registry.template_default_data("t").unwrap().get_f32("speed"), Some(&0.3));

        assert!(registry.create_material("unknown").is_none());
        assert!(registry.template_default_data("unknown").is_none());
    }

    #[test]
    fn templates_and_features_share_names() {
        let mut registry = RenderRegistry::default();

        assert!(registry.add_feature("x", NoFeature));
        assert!(!registry.add_material_template("x", TestTemplate { speed: 0.0 }));
        assert!(registry.add_material_template("y", TestTemplate { speed: 0.0 }));
        assert!(!registry.add_feature("y", NoFeature));
    }

    #[derive(Resource, Default)]
    struct SeenBatches(Vec<(&'static str, Vec<(u32, u32)>)>);

    struct RecordingTemplate(&'static str);

    impl MaterialTemplate for RecordingTemplate {
        type Pass = ();

        fn default_data(&self) -> MaterialData {
            MaterialData::new()
        }

        fn add_passes(&mut self, mut world: WorldDataMut, batches: &[MaterialBatch], _passes: &mut RenderPassWriter<()>) {
            let ranges = batches.iter().map(|b| (b.instance_start, b.instance_count)).collect();
            world
                .resources_mut()
                .get_mut::<SeenBatches>()
                .unwrap()
                .0
                .push((self.0, ranges));
        }

        fn run_pass(&mut self, _pass: &(), _ctx: &mut RenderPassCtx) {}
    }

    #[test]
    fn each_template_gets_its_contiguous_batches() {
        let mut materials = AssetStorageResource::<Material>::new();
        let a = materials.insert(Material {
            template: "a".into(),
            data: MaterialData::new(),
        });
        let c = materials.insert(Material {
            template: "c".into(),
            data: MaterialData::new(),
        });
        let mut world = world_with_materials(materials);
        world.data_mut().resources_mut().insert(SeenBatches::default());
        spawn(&mut world, c.clone());
        spawn(&mut world, a.clone());
        spawn(&mut world, c);

        let mut registry = RenderRegistry::default();
        registry.add_material_template("a", RecordingTemplate("a"));
        registry.add_material_template("b", RecordingTemplate("b"));
        registry.add_material_template("c", RecordingTemplate("c"));

        registry.collect_passes(&mut world);

        let seen = world.data_mut().resources_mut().remove::<SeenBatches>().unwrap();
        assert_eq!(seen.0, [("a", vec![(0, 1)]), ("b", vec![]), ("c", vec![(1, 2)])]);
    }
}
