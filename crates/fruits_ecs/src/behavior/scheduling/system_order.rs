use std::collections::{HashMap, HashSet};

use fruits_ffi::{FfiString, FfiVec};
use fruits_utils::graph::Graph;

use crate::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum OrderEntryType {
    System,
    Group,
}

#[repr(C)]
#[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct OrderEntry {
    pub id: FfiString,
    pub entry_type: OrderEntryType,
}

pub fn create_ordering_graph(ordered_systems: &[SystemFfi], explicit_ordering: &HashSet<(FfiString, FfiString)>) -> OrderGraph {
    let system_index_by_name = ordered_systems
        .iter()
        .enumerate()
        .map(|(i, s)| (s.system_name().clone(), i))
        .collect::<HashMap<_, _>>();

    let mut directions = vec![HashSet::<usize>::new(); ordered_systems.len()];

    for (previous_id, next_id) in explicit_ordering.iter() {
        let Some(&previous_index) = system_index_by_name.get(previous_id) else {
            continue;
        };

        let Some(&next_index) = system_index_by_name.get(next_id) else {
            continue;
        };

        directions[previous_index].insert(next_index);
    }

    let mut system_by_data_readonly = HashMap::<u64, HashSet<usize>>::new();
    let mut system_by_data_mutable = HashMap::<u64, HashSet<usize>>::new();
    let mut systems_global_readonly = HashSet::<usize>::new();
    let mut systems_global_mutable = HashSet::<usize>::new();
    let mut systems_all = HashSet::<usize>::new();
    let mut systems_any_mutable = HashSet::<usize>::new();

    let data_usage_extractors: [fn(&WorldDataUsage) -> &WorldPartDataUsage; _] = [
        (|u| u.resources()),
        (|u| u.entities()),
        (|u| u.events()),
    ];

    for data_usage_extractor in data_usage_extractors {
        system_by_data_readonly.clear();
        system_by_data_mutable.clear();
        systems_global_readonly.clear();
        systems_global_mutable.clear();
        systems_all.clear();
        systems_any_mutable.clear();

        for (system_index, system) in ordered_systems.iter().enumerate() {
            let data_usage = system.data_usage().world();
            let data_usage = data_usage_extractor(data_usage);

            match data_usage {
                WorldPartDataUsage::ByType(data_usage) => {
                    for data_usage in data_usage {
                        let DataUsageEntry {
                            type_id,
                            details: DataUsageDetails { is_mut, .. },
                        } = data_usage;

                        for &other_mutable_system_index in system_by_data_mutable.get(type_id).iter().flat_map(|m| m.iter()) {
                            directions[other_mutable_system_index].insert(system_index);
                        }

                        if *is_mut {
                            for &other_readonly_system_index in system_by_data_readonly.get(type_id).iter().flat_map(|m| m.iter()) {
                                directions[other_readonly_system_index].insert(system_index);
                            }

                            system_by_data_mutable.entry(*type_id).or_default().insert(system_index);
                        } else {
                            system_by_data_readonly.entry(*type_id).or_default().insert(system_index);
                        }

                        for &other_global_mutable_system_index in &systems_global_mutable {
                            directions[other_global_mutable_system_index].insert(system_index);
                        }
                        if *is_mut {
                            for &other_global_readonly_system_index in &systems_global_readonly {
                                directions[other_global_readonly_system_index].insert(system_index);
                            }
                        }

                        if *is_mut {
                            systems_any_mutable.insert(system_index);
                        }
                    }
                }
                WorldPartDataUsage::Global { is_mut } => {
                    if *is_mut {
                        for &other_system_index in systems_all.iter() {
                            directions[other_system_index].insert(system_index);
                        }
                    } else {
                        for &other_system_index in systems_any_mutable.iter() {
                            directions[other_system_index].insert(system_index);
                        }
                    }

                    if *is_mut {
                        systems_global_mutable.insert(system_index);
                    } else {
                        systems_global_readonly.insert(system_index);
                    }

                    if *is_mut {
                        systems_any_mutable.insert(system_index);
                    }
                }
            }

            systems_all.insert(system_index);
        }
    }

    let directions = directions
        .into_iter()
        .map(|v| v.into_iter().map(|i| i as u64).collect::<FfiVec<_>>())
        .collect::<FfiVec<_>>();

    OrderGraph::new(directions).unwrap()
}

pub fn sort_systems_by_order(
    mut systems: HashMap<FfiString, SystemFfi>,
    systems_ordering: &HashSet<(FfiString, FfiString)>,
) -> FfiVec<SystemFfi> {
    let mut systems_ordering_ordered = systems_ordering.iter().cloned().collect::<Vec<_>>();
    systems_ordering_ordered.sort();

    let mut system_names_ordered = systems.keys().cloned().collect::<Vec<_>>();
    system_names_ordered.sort();

    let mut graph = Graph::new();

    for (src, dst) in systems_ordering_ordered.iter() {
        if systems.contains_key(src) && systems.contains_key(dst) {
            graph.insert_edge(src.clone(), dst.clone());
        }
    }

    for ty in system_names_ordered {
        graph.insert_node(ty);
    }
    
    let graph_vec = match graph.to_vec() {
        Ok(x) => x,
        Err(err) => panic!("The systems graph contains circular dependencies:\n{}", err.into_iter().collect::<Vec<_>>().join("\n")),
    };

    graph_vec
        .into_iter()
        .map(|t| systems.remove(&t).unwrap())
        .collect::<FfiVec<_>>()
}

pub fn flatten_ordering(
    ordering: &HashSet<(OrderEntry, OrderEntry)>,
    groups: &HashMap<FfiString, HashSet<OrderEntry>>,
) -> HashSet<(FfiString, FfiString)> {
    let flat_groups = flatten_groups(groups);

    let mut graph_orderer = HashSet::new();

    let mut single_value_min_buffer = HashSet::new();
    let mut single_value_max_buffer = HashSet::new();

    for (min, max) in ordering {
        let min_values = get_systems(min.clone(), &flat_groups, &mut single_value_min_buffer);
        let max_values = get_systems(max.clone(), &flat_groups, &mut single_value_max_buffer);

        for min_value in min_values {
            for max_value in max_values {
                graph_orderer.insert((min_value.clone(), max_value.clone()));
            }
        }
    }

    graph_orderer
}
fn get_systems<'a>(
    node: OrderEntry,
    flat_groups: &'a HashMap<FfiString, HashSet<FfiString>>,
    single_value_buffer: &'a mut HashSet<FfiString>,
) -> &'a HashSet<FfiString> {
    match node.entry_type {
        OrderEntryType::Group => {
            if let Some(group) = flat_groups.get(&node.id) {
                group
            } else {
                single_value_buffer.clear();
                single_value_buffer
            }
        },
        OrderEntryType::System => {
            single_value_buffer.clear();
            single_value_buffer.insert(node.id.clone());
            single_value_buffer
        }
    }
}
fn flatten_groups(groups: &HashMap<FfiString, HashSet<OrderEntry>>) -> HashMap<FfiString, HashSet<FfiString>> {
    let mut group_hierarchy = Graph::<FfiString>::new();

    for (parent_group, group_children) in groups.iter() {
        group_hierarchy.insert_node(parent_group.clone());

        for group_child in group_children {
            if let OrderEntryType::Group = group_child.entry_type {
                group_hierarchy.insert_edge(parent_group.clone(), group_child.id.clone());
            }
        }
    }

    let mut flat_groups = HashMap::<FfiString, HashSet<FfiString>>::new();

    let group_hierarchy_vec = match group_hierarchy.to_vec_rev() {
        Ok(x) => x,
        Err(err) => panic!("The systems graph contains circular dependencies:\n{}", err.into_iter().collect::<Vec<_>>().join("\n")),
    };

    for group in group_hierarchy_vec {
        let mut flat_group_children = HashSet::<FfiString>::new();

        for group_child in groups.get(&group).iter().copied().flatten() {
            match group_child.entry_type {
                OrderEntryType::System => _ = flat_group_children.insert(group_child.id.clone()),
                OrderEntryType::Group => {
                    for ele in &flat_groups[&group_child.id] {
                        flat_group_children.insert(ele.clone());
                    }
                }
            }
        }

        flat_groups.insert(group, flat_group_children);
    }

    flat_groups
}
