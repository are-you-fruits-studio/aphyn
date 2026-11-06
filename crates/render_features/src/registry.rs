use aphyn_ecs::{Schedule, World};
use aphyn_ffi::{FfiIndexMap, FfiString, FfiVec};
use aphyn_render::{CurrentRenderingResource, Material, MaterialData, RenderApiResource};

use crate::{MaterialStep, MaterialTemplate, RenderFeature, RenderPassEntry, RenderSlot};

#[repr(C)]
#[derive(Default)]
pub struct RenderRegistry {
    names: FfiIndexMap<FfiString, u32>,
    slots: FfiVec<RenderSlot>,
    entries: FfiVec<RenderPassEntry>,
    materials: MaterialStep,
}

impl RenderRegistry {
    pub fn add_feature<F: RenderFeature>(&mut self, name: &str, feature: F) -> bool {
        !self.names.contains_key(name) && self.add_slot(name, RenderSlot::from_feature(feature))
    }

    pub fn add_material_template<T: MaterialTemplate>(&mut self, name: &str, template: T) -> bool {
        !self.names.contains_key(name) && self.add_slot(name, RenderSlot::from_template(template))
    }

    fn add_slot(&mut self, name: &str, slot: RenderSlot) -> bool {
        let index = u32::try_from(self.slots.len()).expect("too many render slots");

        self.slots.push(slot);
        self.names.insert(name.into(), index);

        true
    }

    pub fn create_material(&self, template: &str) -> Option<Material> {
        Some(Material {
            template: template.into(),
            data: self.template_default_data(template)?,
        })
    }

    pub fn template_default_data(&self, template: &str) -> Option<MaterialData> {
        let slot = *self.names.get(template)?;
        self.slots[slot as u64].default_data()
    }

    pub fn render_view(&mut self, world: &mut World, view: CurrentRenderingResource) {
        world.data_mut().resources_mut().insert(view);

        // todo: remove after migration
        world.execute_iteration(Schedule::RenderCamera);

        let render_api = world
            .data_mut()
            .resources()
            .get::<RenderApiResource>()
            .map(RenderApiResource::clone);

        if let Some(render_api) = render_api {
            self.collect_passes(world);

            let data = world.data_mut();
            let world_ffi = data.ffi();

            for i in 0..self.entries.len() {
                let entry = self.entries[i];
                let slot = &mut self.slots[entry.slot as u64];

                unsafe {
                    let types = (*world_ffi).types();
                    slot.run_pass(entry.index, world_ffi, types, &render_api);
                }
            }
        }

        world.data_mut().resources_mut().remove::<CurrentRenderingResource>();
    }

    pub(crate) fn collect_passes(&mut self, world: &mut World) {
        self.entries.clear();
        for i in 0..self.slots.len() {
            self.slots[i].clear_passes();
        }

        let mut data = world.data_mut();
        self.materials.gather(data.as_ref(), &self.names, &self.slots);
        self.materials.write_instances(data.as_mut());

        let world_ffi = data.ffi();
        let mut cursor = 0;

        for templates in [true, false] {
            for i in 0..self.slots.len() {
                let slot = &mut self.slots[i];
                if slot.is_template() != templates {
                    continue;
                }

                let batches = match templates {
                    true => self.materials.next_template_batches(i as u32, &mut cursor),
                    false => &[],
                };

                unsafe {
                    let types = (*world_ffi).types();
                    slot.add_passes(i as u32, world_ffi, types, &mut self.entries, batches);
                }
            }
        }

        self.entries.sort_unstable_by_key(|entry| entry.order);
    }

    #[cfg(test)]
    pub(crate) fn gathered(&self) -> &aphyn_ffi::FfiVec<crate::GatheredInstance> {
        self.materials.gathered()
    }

    #[cfg(test)]
    pub(crate) fn logged_materials(&self) -> &aphyn_ffi::FfiIndexSet<aphyn_asset_storage::AssetHandle<Material>> {
        self.materials.logged()
    }

    #[cfg(test)]
    pub(crate) fn pass_entries(&self) -> &[RenderPassEntry] {
        &self.entries
    }

    #[cfg(test)]
    fn pass_entries_capacity(&self) -> u64 {
        self.entries.capacity()
    }

    #[cfg(test)]
    fn slots_len(&self) -> u64 {
        self.slots.len()
    }

    #[cfg(test)]
    fn slot_of(&self, name: &str) -> Option<u32> {
        self.names.get(name).copied()
    }

    #[cfg(test)]
    unsafe fn slot_passes<F: RenderFeature>(&self, slot: u32) -> &FfiVec<F::Pass> {
        unsafe { self.slots[slot as u64].passes::<F>() }
    }
}

#[cfg(test)]
mod tests {
    use aphyn_ecs::*;

    use crate::*;

    #[derive(Resource, Default)]
    struct CallLog(Vec<&'static str>);

    struct FixedOrders {
        name: &'static str,
        orders: Vec<u64>,
    }

    impl FixedOrders {
        fn new(name: &'static str, orders: &[u64]) -> Self {
            Self {
                name,
                orders: orders.to_vec(),
            }
        }
    }

    impl RenderFeature for FixedOrders {
        type Pass = u64;

        fn add_passes(&mut self, mut world: WorldDataMut, passes: &mut RenderPassWriter<u64>) {
            let mut resources = world.resources_mut();
            if let Some(log) = resources.get_mut::<CallLog>() {
                log.0.push(self.name);
            }

            for &order in &self.orders {
                passes.add(order, order);
            }
        }

        fn run_pass(&mut self, _pass: &u64, _ctx: &mut RenderPassCtx) {}
    }

    fn orders(registry: &RenderRegistry) -> Vec<u64> {
        registry.pass_entries().iter().map(|e| e.order).collect()
    }

    #[test]
    fn passes_are_sorted_by_order() {
        let mut world = WorldBuilder::new().build();
        let mut registry = RenderRegistry::default();

        assert!(registry.add_feature("a", FixedOrders::new("a", &[9_000_000, 1_000_000])));
        assert!(registry.add_feature("b", FixedOrders::new("b", &[5_000_000])));

        registry.collect_passes(&mut world);

        assert_eq!(orders(&registry), [1_000_000, 5_000_000, 9_000_000]);

        let slot_a = registry.slot_of("a").unwrap();
        let slot_b = registry.slot_of("b").unwrap();
        assert_ne!(slot_a, slot_b);

        for entry in registry.pass_entries() {
            let passes = unsafe { registry.slot_passes::<FixedOrders>(entry.slot) };
            assert_eq!(passes[entry.index as u64], entry.order);
        }

        let slots: Vec<u32> = registry.pass_entries().iter().map(|e| e.slot).collect();
        assert_eq!(slots, [slot_a, slot_b, slot_a]);
    }

    #[test]
    fn features_run_in_registration_order() {
        let mut world = WorldBuilder::new().build();
        world.data_mut().resources_mut().insert(CallLog::default());

        let mut registry = RenderRegistry::default();
        registry.add_feature("a", FixedOrders::new("a", &[]));
        registry.add_feature("b", FixedOrders::new("b", &[]));

        registry.collect_passes(&mut world);

        let log = world.data_mut().resources_mut().remove::<CallLog>().unwrap();
        assert_eq!(log.0, ["a", "b"]);
    }

    #[test]
    fn duplicate_name_is_rejected() {
        let mut world = WorldBuilder::new().build();
        let mut registry = RenderRegistry::default();

        assert!(registry.add_feature("a", FixedOrders::new("a", &[1])));
        assert!(!registry.add_feature("a", FixedOrders::new("a2", &[2])));
        assert_eq!(registry.slots_len(), 1);

        registry.collect_passes(&mut world);

        assert_eq!(orders(&registry), [1]);
    }

    #[test]
    fn collecting_twice_does_not_reallocate() {
        let mut world = WorldBuilder::new().build();
        let mut registry = RenderRegistry::default();
        registry.add_feature("a", FixedOrders::new("a", &[3, 2, 1, 4, 5]));

        registry.collect_passes(&mut world);

        let slot = registry.slot_of("a").unwrap();
        let snapshot = |registry: &RenderRegistry| {
            let passes = unsafe { registry.slot_passes::<FixedOrders>(slot) };
            (
                registry.pass_entries().as_ptr(),
                registry.pass_entries_capacity(),
                passes.as_ptr(),
                passes.capacity(),
            )
        };
        let before = snapshot(&registry);

        registry.collect_passes(&mut world);

        assert_eq!(snapshot(&registry), before);
        assert_eq!(orders(&registry), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RenderRegistry>();
    }

    #[test]
    fn empty_registry_collects_nothing() {
        let mut world = WorldBuilder::new().build();
        let mut registry = RenderRegistry::default();

        registry.collect_passes(&mut world);

        assert!(registry.pass_entries().is_empty());
    }
}
