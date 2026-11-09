use aphyn_ecs::WorldData;
use aphyn_ffi::{FfiIndexMap, FfiString, FfiVec};
use aphyn_render::{CurrentRenderingResource, RenderApiResource};

use crate::{RenderFeature, RenderFeatureFfi, RenderPassEntry, RenderPassInput, RenderPasses};

#[repr(C)]
#[derive(Default)]
pub struct RenderFeatureRegistry {
    names: FfiIndexMap<FfiString, u32>,
    slots: FfiVec<RenderFeatureFfi>,
    passes: RenderPasses,
    entries: FfiVec<RenderPassEntry>,
}

// Safety. Pass closures are `Send`, and the pass vec is only touched through `&mut self`.
unsafe impl Send for RenderFeatureRegistry {}
unsafe impl Sync for RenderFeatureRegistry {}

impl RenderFeatureRegistry {
    pub fn add_feature<F: RenderFeature>(&mut self, name: &str, feature: F) -> bool {
        if self.names.contains_key(name) {
            return false;
        }

        let index = u32::try_from(self.slots.len()).expect("too many render slots");
        self.slots.push(RenderFeatureFfi::from_feature(feature));
        self.names.insert(name.into(), index);

        true
    }

    pub fn render_view(&mut self, world: &mut WorldData, view: CurrentRenderingResource) {
        world.resources_mut().insert(view);

        let render_api = world.resources().get::<RenderApiResource>().map(RenderApiResource::clone);

        if let Some(render_api) = render_api {
            self.collect_passes(world);

            let data = world.as_mut();
            let world_ffi = data.ffi();

            for i in 0..self.entries.len() {
                let entry = self.entries[i];
                let input = RenderPassInput {
                    this: self.slots[entry.slot as u64].state(),
                    world: world_ffi,
                    types: unsafe { (*world_ffi).types() },
                    render_api: &render_api,
                };

                self.passes.call(entry.index as usize, input);
            }

            self.passes.clear();
        }

        world.resources_mut().remove::<CurrentRenderingResource>();
    }

    pub(crate) fn collect_passes(&mut self, world: &mut WorldData) {
        self.entries.clear();
        self.passes.clear();

        let world_ffi = world.as_mut().ffi();

        for i in 0..self.slots.len() {
            unsafe {
                let types = (*world_ffi).types();
                self.slots[i].add_passes(i as u32, world_ffi, types, &mut self.passes, &mut self.entries);
            }
        }

        self.entries.sort_unstable_by_key(|entry| entry.order);
    }

    #[cfg(test)]
    pub(crate) fn feature<F: RenderFeature>(&self, name: &str) -> &F {
        let slot = self.names.get(name).copied().unwrap();
        unsafe { &(*(self.slots[slot as u64].state() as *const crate::SlotState<F>)).value }
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
    fn passes_len(&self) -> usize {
        self.passes.len()
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
        fn add_passes(&mut self, mut world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
            let mut resources = world.resources_mut();
            if let Some(log) = resources.get_mut::<CallLog>() {
                log.0.push(self.name);
            }

            for &order in &self.orders {
                passes.add(order, |_, _| {});
            }
        }
    }

    fn orders(registry: &RenderFeatureRegistry) -> Vec<u64> {
        registry.pass_entries().iter().map(|e| e.order).collect()
    }

    #[test]
    fn passes_are_sorted_by_order() {
        let mut world = WorldData::default();
        let mut registry = RenderFeatureRegistry::default();

        assert!(registry.add_feature("a", FixedOrders::new("a", &[9_000_000, 1_000_000])));
        assert!(registry.add_feature("b", FixedOrders::new("b", &[5_000_000])));

        registry.collect_passes(&mut world);

        assert_eq!(orders(&registry), [1_000_000, 5_000_000, 9_000_000]);

        let slot_a = registry.slot_of("a").unwrap();
        let slot_b = registry.slot_of("b").unwrap();
        assert_ne!(slot_a, slot_b);

        let mut indices: Vec<u32> = registry.pass_entries().iter().map(|e| e.index).collect();
        indices.sort_unstable();
        assert_eq!(registry.passes_len(), 3);
        assert_eq!(indices, [0, 1, 2]);

        let slots: Vec<u32> = registry.pass_entries().iter().map(|e| e.slot).collect();
        assert_eq!(slots, [slot_a, slot_b, slot_a]);
    }

    #[test]
    fn features_run_in_registration_order() {
        let mut world = WorldData::default();
        world.as_mut().resources_mut().insert(CallLog::default());

        let mut registry = RenderFeatureRegistry::default();
        registry.add_feature("a", FixedOrders::new("a", &[]));
        registry.add_feature("b", FixedOrders::new("b", &[]));

        registry.collect_passes(&mut world);

        let log = world.as_mut().resources_mut().remove::<CallLog>().unwrap();
        assert_eq!(log.0, ["a", "b"]);
    }

    #[test]
    fn duplicate_name_is_rejected() {
        let mut world = WorldData::default();
        let mut registry = RenderFeatureRegistry::default();

        assert!(registry.add_feature("a", FixedOrders::new("a", &[1])));
        assert!(!registry.add_feature("a", FixedOrders::new("a2", &[2])));
        assert_eq!(registry.slots_len(), 1);

        registry.collect_passes(&mut world);

        assert_eq!(orders(&registry), [1]);
    }

    #[test]
    fn collecting_twice_does_not_reallocate() {
        let mut world = WorldData::default();
        let mut registry = RenderFeatureRegistry::default();
        registry.add_feature("a", FixedOrders::new("a", &[3, 2, 1, 4, 5]));

        registry.collect_passes(&mut world);

        let snapshot = |registry: &RenderFeatureRegistry| (registry.pass_entries().as_ptr(), registry.pass_entries_capacity());
        let before = snapshot(&registry);

        registry.collect_passes(&mut world);

        assert_eq!(snapshot(&registry), before);
        assert_eq!(orders(&registry), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RenderFeatureRegistry>();
    }

    #[test]
    fn empty_registry_collects_nothing() {
        let mut world = WorldData::default();
        let mut registry = RenderFeatureRegistry::default();

        registry.collect_passes(&mut world);

        assert!(registry.pass_entries().is_empty());
    }
}
