use std::any::Any;

use aphyn_ffi::FfiString;

use crate::*;

pub struct SystemsHolderBuilderMut<'s> {
    systems: &'s mut SystemsHolderBuilderFfi,
    types: &'s TypesRegistryCache,
}

impl<'s> SystemsHolderBuilderMut<'s> {
    pub fn new(systems: &'s mut SystemsHolderBuilderFfi, types: &'s TypesRegistryCache) -> Self {
        Self { systems, types }
    }

    pub fn insert_system<M: 'static>(&mut self, system: impl SystemWithMarker<M> + Any) -> bool {
        self.systems.insert_system(SystemFfi::new(system, self.types.clone()))
    }

    pub fn clear(&mut self) {
        self.systems.clear();
    }

    #[must_use]
    pub fn order_system<'r, M0: 'static>(&'r mut self, s: impl SystemWithMarker<M0> + Any) -> OrderHelper<'r>
    where
        's: 'r,
    {
        let systems_holder_builder = SystemsHolderBuilderMut {
            systems: self.systems,
            types: self.types,
        };
        OrderHelper::from_system(systems_holder_builder, s)
    }

    #[must_use]
    pub fn order_group<'r>(&'r mut self, g: &'static str) -> OrderHelper<'r>
    where
        's: 'r,
    {
        let systems_holder_builder = SystemsHolderBuilderMut {
            systems: self.systems,
            types: self.types,
        };
        OrderHelper::from_group(systems_holder_builder, g)
    }

    #[must_use]
    pub fn group<'r>(&'r mut self, group: &'static str) -> GroupHelper<'r>
    where
        's: 'r,
    {
        let systems_holder_builder = SystemsHolderBuilderMut {
            systems: self.systems,
            types: self.types,
        };
        GroupHelper::new(systems_holder_builder, group)
    }
}

//

pub struct OrderHelper<'a> {
    builder: SystemsHolderBuilderMut<'a>,
    prev: OrderEntry,
}

impl<'a> OrderHelper<'a> {
    fn from_system<M0: 'static>(builder: SystemsHolderBuilderMut<'a>, previous_system: impl SystemWithMarker<M0> + Any) -> Self {
        Self {
            builder,
            prev: OrderEntry {
                entry_type: OrderEntryType::System,
                id: FfiString::from_string(std::any::type_name_of_val(&previous_system).to_string()),
            },
        }
    }
    fn from_group(builder: SystemsHolderBuilderMut<'a>, group: &'static str) -> Self {
        Self {
            builder,
            prev: OrderEntry {
                entry_type: OrderEntryType::Group,
                id: FfiString::from_string(group.to_string()),
            },
        }
    }

    pub fn before_system<M1: 'static>(self, s: impl SystemWithMarker<M1> + Any) -> OrderHelper<'a> {
        let next = OrderEntry {
            entry_type: OrderEntryType::System,
            id: FfiString::from_string(std::any::type_name_of_val(&s).to_string()),
        };

        self.builder.systems.order(self.prev, next.clone());

        OrderHelper::<'a> {
            builder: self.builder,
            prev: next,
        }
    }

    pub fn before_group(self, g: &'static str) -> OrderHelper<'a> {
        let next = OrderEntry {
            entry_type: OrderEntryType::Group,
            id: FfiString::from_string(g.to_string()),
        };

        self.builder.systems.order(self.prev, next.clone());

        OrderHelper::<'a> {
            builder: self.builder,
            prev: next,
        }
    }
}

pub struct GroupHelper<'a> {
    builder: SystemsHolderBuilderMut<'a>,
    group: &'static str,
}

impl<'a> GroupHelper<'a> {
    fn new(builder: SystemsHolderBuilderMut<'a>, group: &'static str) -> Self {
        Self { builder, group }
    }

    pub fn insert_child_system<M1: 'static>(&mut self, s: impl SystemWithMarker<M1> + Any) -> &mut Self {
        let entry = OrderEntry {
            entry_type: OrderEntryType::System,
            id: FfiString::from_string(std::any::type_name_of_val(&s).to_string()),
        };

        self.builder
            .systems
            .insert_group_child(FfiString::from_string(self.group.to_string()), entry);
        self.builder.insert_system(s);
        self
    }

    pub fn insert_child_group(&mut self, g: &'static str) -> &mut Self {
        let entry = OrderEntry {
            entry_type: OrderEntryType::Group,
            id: FfiString::from_string(g.to_string()),
        };

        self.builder
            .systems
            .insert_group_child(FfiString::from_string(self.group.to_string()), entry);
        self
    }
}
