use std::collections::{HashMap, HashSet};

use fruits_ffi::FfiString;

use crate::*;

// todo: ffi
pub struct SystemsHolderBuilderNative {
    systems: HashMap<FfiString, SystemFfi>,
    systems_ordering: HashSet<(OrderEntry, OrderEntry)>,
    system_groups: HashMap<FfiString, HashSet<OrderEntry>>,
    types: TypesRegistryAccessFfi,
}

impl SystemsHolderBuilderNative {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        Self {
            systems: HashMap::new(),
            systems_ordering: HashSet::new(),
            system_groups: HashMap::new(),
            types,
        }
    }

    pub fn insert_system(&mut self, system: SystemFfi) -> bool {
        self.systems.insert(system.system_name().clone(), system).is_none()
    }

    pub fn order(&mut self, prev: OrderEntry, next: OrderEntry) {
        self.systems_ordering.insert((prev, next));
    }

    pub fn insert_group_child(&mut self, group: FfiString, child: OrderEntry) {
        self.system_groups.entry(group).or_default().insert(child);
    }

    pub fn clear(&mut self) {
        self.systems.clear();
        self.systems_ordering.clear();
        self.system_groups.clear();
    }

    pub fn build(&mut self) -> SystemsHolderNative {
        let new_self = Self::new(self.types.clone());
        let old_self = std::mem::replace(self, new_self);

        let systems_ordering = flatten_ordering(&old_self.systems_ordering, &old_self.system_groups);

        let systems = sort_systems_by_order(old_self.systems, &systems_ordering);

        let execution_graph = create_ordering_graph(&systems, &systems_ordering);

        SystemsHolderNative::new(systems, execution_graph, old_self.types)
    }
}
