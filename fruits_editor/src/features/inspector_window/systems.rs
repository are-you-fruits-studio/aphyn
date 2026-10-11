use std::collections::HashMap;

use fruits_engine::tree::{TreeBuilder, TreeNode};

use crate::{
    features::{
        asset_serialization::{InspectedAsset, get_asset_type}, dropdown::DropdownEntryComponent, input_field::{InputFieldComponent, InputFieldSelectionChangedEvent, SelectedInputFieldResource}, inspector_window::{
            data::*, utils::{
                entries::{parse_serialized, spawn_default_layout_ent, spawn_hierarchy_window_entry, spawn_serialized, spawn_text_ent}, find_in_parents, serialization::{are_components_slices_similar, deserialize_asset_to_world_res, enrich_serialized_with_asset_type, load_asset_to_world_res, save_asset_from_world_res}, subsequence_match_ignore_case,
            },
        }, project_window_selection::{FileSelectedEvent, SelectedFileResource}, world_preload::{SimulatedWorld, SimulatedWorldResource},
    }, prefabs::WindowComponent, *,
};

pub fn select_entity_system(
    mut inspected_entity: ResMut<InspectedEntityResource>,
    button_click_evt: Evt<ButtonClickEvent>,
    entry_q: WorldQuery<&HierarchyWindowEntryComponent>,
    mut prefab_entity_selected_evt: EvtMut<PrefabEntitySelectedEvent>,
) {
    return_if_not!(Some(button_click_evt) = button_click_evt.last());

    if let Some(entry_c) = entry_q.get(button_click_evt.entity) {
        inspected_entity.selected_entity = entry_c.simulated_entity;
        prefab_entity_selected_evt.push(PrefabEntitySelectedEvent);
    }
}

pub fn update_hierarchy_entries_selection(
    inspected_entity: Res<InspectedEntityResource>,
    mut entry_q: WorldQuery<(&HierarchyWindowEntryComponent, &mut ImageComponent)>,
) {
    for (entry_c, image_c) in entry_q.iter_mut() {
        if entry_c.simulated_entity == inspected_entity.selected_entity {
            image_c.color = Vec4::from_array(const { parse_color_rgba_f32("#7d90e6ff").unwrap() });
        } else {
            image_c.color = Vec4::from_array(const { parse_color_rgba_f32("#00000000").unwrap() });
        }
    }
}

pub fn remove_component_system(
    ent: EntitiesHolderRef,
    button_click_evt: Evt<ButtonClickEvent>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    mut inspected_asset_edited_evt: EvtMut<InspectedAssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    return_if!(
        get_asset_type(simulated_world.world.data().resources(), inspected_asset.asset_key.as_str()) != Some(AssetType::Prefab)
    );

    for button_click_evt in button_click_evt.iter() {
        continue_if_not!(Some(component_remove_button_c) = ent.get_component::<ComponentRemoveButton>(button_click_evt.entity));
        continue_if_not!(Some(serialized_component_c) = ent.get_component::<SerializedComponentComponent>(component_remove_button_c.component));
        continue_if_not!(Some(component_id_text_c) = ent.get_component::<TextComponent>(serialized_component_c.component_id_text));

        let component_id = component_id_text_c.text.as_str();
        let (_, mut sim_ent, _) = simulated_world.world.data_mut().into_tuple_mut();

        let mut has_component = false;
        sim_ent.get_all_components(inspected_entity.selected_entity, |c| has_component |= c.type_info().short().name() == component_id);
        continue_if!(!has_component);

        if sim_ent.remove_component_any(inspected_entity.selected_entity, component_id).is_some() {
            inspected_asset_edited_evt.push(InspectedAssetEditedEvent);
        }
    }
}

pub fn add_component_system(
    ent: EntitiesHolderRef,
    button_click_evt: Evt<ButtonClickEvent>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    mut inspected_asset_edited_evt: EvtMut<InspectedAssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    return_if!(
        get_asset_type(simulated_world.world.data().resources(), inspected_asset.asset_key.as_str()) != Some(AssetType::Prefab)
    );

    for button_click_evt in button_click_evt.iter() {
        continue_if_not!(Some(add_component_variant_c) = ent.get_component::<AddComponentVariantComponent>(button_click_evt.entity));

        let (sim_res, sim_ent, _) = simulated_world.world.data_mut().into_tuple_mut();

        let did_add = deserialize_entity_component_from_prefab(
            sim_res.as_ref(),
            sim_ent,
            inspected_entity.selected_entity,
            &add_component_variant_c.component_id,
            "",
            &SerializedValue::Null,
            &inspected_entity.id_to_ent,
        );

        if did_add {
            inspected_asset_edited_evt.push(InspectedAssetEditedEvent);
        }
    }
}

pub fn update_add_component_variants_system(
    mut ent: EntitiesHolderMut,
    selected_input: Res<SelectedInputFieldResource>,
    assets: Res<StandardAssetsResource>,
    simulated_world: Res<SimulatedWorldResource>,
) {
    for entity in ent.query::<&AddComponentInputComponent>().iter().map(|c| c.variants_container).collect::<Vec<_>>() {
        destroy_entity_children(ent.as_mut(), entity);
    }

    let Some(simulated_world) = &simulated_world.0 else {
        return;
    };

    let Some(serializers) = simulated_world.world.data().resources().get::<SerializersResource>() else {
        return;
    };

    let Some(add_component_input_c) = ent.get_component::<AddComponentInputComponent>(selected_input.selected).copied() else {
        return;
    };

    let Some(input_field_c) = ent.get_component::<InputFieldComponent>(selected_input.selected) else {
        return;
    };

    let Some(text_c) = ent.get_component::<TextComponent>(input_field_c.text) else {
        return;
    };

    let searched_text = text_c.text.clone();

    let mut keys = serializers.0.keys().map(FfiString::from).collect::<Vec<_>>();
    keys.sort();
    
    for component_id in keys {
        if !subsequence_match_ignore_case(component_id.as_str(), searched_text.as_str()) {
            continue;
        }

        let entry_ent = spawn_default_layout_ent(
            ent.as_mut(),
            add_component_input_c.variants_container,
            false,
        );

        let (entry_ent, _) = spawn_text_ent(
            ent.as_mut(),
            entry_ent,
            component_id.clone(),
            assets.material_panel.clone(),
            assets.material_text.clone(),
            assets.font.clone(),
        );

        ent.set_components(entry_ent, (
            AddComponentVariantComponent { component_id: component_id },
            ButtonComponent::default(),
        ));
    }
}

pub fn adjust_hierarchy_entries_system(
    button_click_evt: Evt<ButtonClickEvent>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    button_add_q: WorldQuery<&HierarchyButtonAddComponent>,
    button_remove_q: WorldQuery<&HierarchyButtonRemoveComponent>,
) {
    let Some(simulated_world) = &mut simulated_world.0 else {
        return;
    };

    for click_evt in button_click_evt.iter() {
        if button_remove_q.get(click_evt.entity).is_some() {
            let removed_entity = inspected_entity.selected_entity;
            simulated_world.world.data_mut().entities_mut().destroy_entity(removed_entity);
            
            if let Some(removed_id) = inspected_entity.ent_to_id.remove(&removed_entity) {
                inspected_entity.id_to_ent.remove(&removed_id);
                inspected_entity.selected_entity = EntityId::EMPTY;
            }
        }
        if button_add_q.get(click_evt.entity).is_some() {
            let entity = simulated_world.world.data_mut().entities_mut().create_entity();
            
            let new_id = (1..=(inspected_entity.id_to_ent.len() as u64 + 1)).filter(|i| !inspected_entity.id_to_ent.contains_key(i)).next().unwrap();

            inspected_entity.ent_to_id.insert(entity, new_id);
            inspected_entity.id_to_ent.insert(new_id, entity);

            inspected_entity.selected_entity = entity;
        }
    }
}

pub fn apply_inspector_field_text_change_to_simulated_world_system(
    text_input_evt: Evt<TextInputEvent>,
    selected_input_res: Res<SelectedInputFieldResource>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut inspected_asset_edited_evt: EvtMut<InspectedAssetEditedEvent>,
) {
    // todo:
    // - save assets to files when they are changed even a bit
    // - restore the "add-component" functionality (with the new serialization api)

    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    if text_input_evt.is_empty() {
        return;
    }

    let did_asset_load = apply_inspector_serialized_value_to_simulated_world(
        ent,
        selected_input_res.selected,
        simulated_world,
        &inspected_entity,
        &inspected_asset,
        &open_project,
    );

    if did_asset_load {
        inspected_asset_edited_evt.push(InspectedAssetEditedEvent);
    };
}

pub fn apply_inspector_dropdown_change_to_simulated_world_system(
    click_evt: Evt<ButtonClickEvent>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut inspected_asset_edited_evt: EvtMut<InspectedAssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    for click_evt in click_evt.iter() {
        continue_if_not!(Some(dropdown_entry_c) = ent.get_component::<DropdownEntryComponent>(click_evt.entity));

        continue_if_not!(Some(ent_composite) = find_in_parents(ent.query::<&ChildComponent>(), dropdown_entry_c.dropdown, |e| {
            ent.get_component::<SerializedValueComponent>(e).map(|_| e)
        }));

        let did_asset_load = apply_inspector_serialized_value_to_simulated_world(
            ent,
            ent_composite,
            simulated_world,
            &inspected_entity,
            &inspected_asset,
            &open_project,
        );

        if did_asset_load {
            inspected_asset_edited_evt.push(InspectedAssetEditedEvent);
        };
    }
}

pub fn adjust_non_rigid_composite_system(
    click_evt: Evt<ButtonClickEvent>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut inspected_asset_edited_evt: EvtMut<InspectedAssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    for click_evt in click_evt.iter() {
        let (ent_composite, is_add) = if let Some(btn_add_c) = ent.get_component::<SerializedCompositeAddButton>(click_evt.entity) {
            (btn_add_c.composite, true)
        } else if let Some(btn_remove_c) = ent.get_component::<SerializedCompositeRemoveButton>(click_evt.entity) {
            (btn_remove_c.composite, false)
        } else {
            continue;
        };

        continue_if_not!(Some(serialized_value_c) = ent.get_component::<SerializedValueComponent>(ent_composite));
        continue_if_not!(Some(target) = get_inspected_target(ent, ent_composite, simulated_world, &inspected_asset));

        let path = serialized_value_c.path.as_str();

        continue_if_not!(Some(SerializedValue::Composite(mut composite)) = serialize_from_simulated_world(
            &target,
            path,
            simulated_world,
            &inspected_entity,
            &inspected_asset,
        ));

        let did_asset_load = if is_add {
            let new_key = match &composite.values {
                SerializedCompositeValues::List(list) => list.len().to_string(),
                SerializedCompositeValues::Map(map) => (0u64..)
                    .map(|i| i.to_string())
                    .find(|key| map.values.get(key.as_str()).is_none())
                    .unwrap(),
            };

            deserialize_to_simulated_world(
                &target,
                &format!("{path}/{new_key}"),
                &SerializedValue::Null,
                simulated_world,
                &inspected_entity,
                &inspected_asset,
                &open_project,
            )
        } else {
            // todo: think about changing the serializing api:
            // path writes can't remove elements, so the whole collection is rewritten without its last one
            let did_remove = match &mut composite.values {
                SerializedCompositeValues::List(list) => list.pop().is_some(),
                SerializedCompositeValues::Map(map) => match map.values.len().checked_sub(1) {
                    Some(idx_last) => {
                        let key_last = map.values.get_by_idx(idx_last).unwrap().0.clone();
                        map.values.remove_shift(&key_last).is_some()
                    }
                    None => false,
                },
            };

            did_remove && deserialize_to_simulated_world(
                &target,
                path,
                &SerializedValue::Composite(composite),
                simulated_world,
                &inspected_entity,
                &inspected_asset,
                &open_project,
            )
        };

        if did_asset_load {
            inspected_asset_edited_evt.push(InspectedAssetEditedEvent);
        };
    }
}

enum InspectedTarget {
    Asset(AssetType),
    PrefabComponent { component_id: FfiString },
}

fn get_inspected_target(
    ent: EntitiesHolderRef,
    ent_serialized: EntityId,
    simulated_world: &SimulatedWorld,
    inspected_asset: &InspectedAssetResource,
) -> Option<InspectedTarget> {
    let asset_type = get_asset_type(simulated_world.world.data().resources(), inspected_asset.asset_key.as_str())?;

    if asset_type != AssetType::Prefab {
        return Some(InspectedTarget::Asset(asset_type));
    }

    let serialized_component_c = find_in_parents(ent.query::<&ChildComponent>(), ent_serialized, |e| {
        ent.get_component::<SerializedComponentComponent>(e)
    })?;

    let component_id_text_c = ent.get_component::<TextComponent>(serialized_component_c.component_id_text)?;

    Some(InspectedTarget::PrefabComponent { component_id: component_id_text_c.text.clone() })
}

fn serialize_from_simulated_world(
    target: &InspectedTarget,
    path: &str,
    simulated_world: &SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &InspectedAssetResource,
) -> Option<SerializedValue> {
    let res = simulated_world.world.data().resources();

    match target {
        InspectedTarget::Asset(_) => save_asset_from_world_res(res, inspected_asset.asset_key.as_str())?
            .get_by_path(path)
            .cloned(),
        InspectedTarget::PrefabComponent { component_id } => record_into_prefab_components(
            res,
            simulated_world.world.data().entities(),
            inspected_entity.selected_entity,
            &inspected_entity.ent_to_id,
        )
        .get(component_id)?
        .get_by_path(path)
        .cloned(),
    }
}

fn deserialize_to_simulated_world(
    target: &InspectedTarget,
    path: &str,
    serialized: &SerializedValue,
    simulated_world: &mut SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &InspectedAssetResource,
    open_project: &OpenProjectResource,
) -> bool {
    match target {
        InspectedTarget::Asset(asset_type) => deserialize_asset_to_world_res(
            simulated_world.world.data_mut().resources_mut(),
            inspected_asset.asset_key.as_str(),
            path,
            serialized,
            *asset_type,
            &(open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH),
        ),
        InspectedTarget::PrefabComponent { component_id } => {
            let (sim_res, sim_ent, _) = simulated_world.world.data_mut().into_tuple_mut();

            // todo: react to result?
            deserialize_entity_component_from_prefab(
                sim_res.as_ref(),
                sim_ent,
                inspected_entity.selected_entity,
                component_id,
                path,
                serialized,
                &inspected_entity.id_to_ent,
            )
        }
    }
}

fn apply_inspector_serialized_value_to_simulated_world(
    ent: EntitiesHolderRef,
    ent_serialized: EntityId,
    simulated_world: &mut SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &InspectedAssetResource,
    open_project: &OpenProjectResource,
) -> bool {
    let Some(serialized_value_c) = ent.get_component::<SerializedValueComponent>(ent_serialized) else {
        return false;
    };

    let Some(target) = get_inspected_target(ent, ent_serialized, simulated_world, inspected_asset) else {
        return false;
    };

    deserialize_to_simulated_world(
        &target,
        &serialized_value_c.path,
        &parse_serialized(ent, ent_serialized),
        simulated_world,
        inspected_entity,
        inspected_asset,
        open_project,
    )
}

// todo: rewrite to new serialization api
pub fn apply_inspector_to_simulated_world_system(
    ent: EntitiesHolderRef,
    inspector_window_q: WorldQuery<&InspectorWindowContentComponent>,
    parent_q: WorldQuery<&ParentComponent>,
    text_q: WorldQuery<&TextComponent>,
    serialized_component_q: WorldQuery<&SerializedComponentComponent>,
    inspected_asset: Res<InspectedAssetResource>,
    inspected_entity: Res<InspectedEntityResource>,
    open_project: Res<OpenProjectResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut inspected_asset_edited_evt: EvtMut<InspectedAssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some(window_c) = inspector_window_q.iter().next().copied());
    let content_container = window_c.content_container;
    return_if_not!(Some(parent_c) = parent_q.get(content_container));
    return_if_not!(Some(&content_ent) = parent_c.children.first());

    return_if_not!(Some(asset_type) = get_asset_type(simulated_world.world.data().resources(), inspected_asset.asset_key.as_str()));

    if asset_type == AssetType::Prefab {
        let stored_components = record_into_prefab_components(
            simulated_world.world.data().resources(),
            simulated_world.world.data().entities(),
            inspected_entity.selected_entity,
            &inspected_entity.ent_to_id,
        );

        let mut parsed_components = FfiIndexMap::<FfiString, SerializedValue>::new();

        for &child in &parent_c.children {
            continue_if_not!(Some(serialized_component_c) = serialized_component_q.get(child));
            continue_if_not!(Some(text_c) = text_q.get(serialized_component_c.component_id_text));
            continue_if_not!(Some(component_data_container_parent_c) = parent_q.get(serialized_component_c.component_data_container));
            continue_if_not!(Some(&serialized_ent) = component_data_container_parent_c.children.get(0));

            let serialized_component = parse_serialized(ent, serialized_ent);

            parsed_components.insert(
                text_c.text.clone(),
                serialized_component,
            );
        }

        return_if!(are_components_slices_similar(&stored_components, &parsed_components));

        let (sim_res, sim_ent, sim_evt) = simulated_world.world.data_mut().into_tuple_mut();
        override_entity_components_from_prefab(
            sim_res.as_ref(),
            sim_ent,
            inspected_entity.selected_entity,
            &parsed_components,
            &inspected_entity.id_to_ent,
        );

        inspected_asset_edited_evt.push(InspectedAssetEditedEvent);

        return;
    }

    let serialized_existing = save_asset_from_world_res(
        simulated_world.world.data().resources(),
        inspected_asset.asset_key.as_str(),
    );

    let mut serialized_parsed = parse_serialized(ent, content_ent);

    enrich_serialized_with_asset_type(&mut serialized_parsed, asset_type);

    return_if!(SerializedValue::similar_option(
        serialized_existing.as_ref(),
        Some(&serialized_parsed)
    ));

    // todo
    let did_asset_load = load_asset_to_world_res(
        simulated_world.world.data_mut().resources_mut(),
        inspected_asset.asset_key.as_str(),
        &serialized_parsed,
        asset_type,
        &(open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH),
    );

    if did_asset_load {
        inspected_asset_edited_evt.push(InspectedAssetEditedEvent);
    };
}

pub fn save_simulated_entities_to_prefab_system(
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_asset: Res<InspectedAssetResource>,
) {
    let Some(simulated_world) = &mut simulated_world.0 else {
        return;
    };

    let Some(prefab) = record_into_prefab(
        simulated_world.world.data().resources(),
        simulated_world.world.data().entities(),
        inspected_asset.spawned_prefab
    ) else {
        return;
    };

    let Some(AssetType::Prefab) = get_asset_type(simulated_world.world.data().resources(), inspected_asset.asset_key.as_str()) else {
        return;
    };

    let prefabs = simulated_world.world.data_mut().into_resources_mut().into_get_mut::<AssetStorageResource<Prefab>>().unwrap();
    let prefab_handle = prefabs.get_registered(inspected_asset.asset_key.as_str()).unwrap().clone();

    *prefabs.get_mut(&prefab_handle).unwrap() = prefab;
}

pub fn save_inspected_asset_from_simulated_world_to_file_system(
    evt_inspected_asset_edited: Evt<InspectedAssetEditedEvent>,
    simulated_world: Res<SimulatedWorldResource>,
    selected_file: Res<SelectedFileResource>,
    inspected_asset: Res<InspectedAssetResource>,
) {
    return_if!(evt_inspected_asset_edited.is_empty() && inspected_asset.asset_key == selected_file.potential_asset_key);

    return_if_not!(Some(simulated_world) = &simulated_world.0);

    let serialized = save_asset_from_world_res(
        simulated_world.world.data().resources(),
        inspected_asset.asset_key.as_str(),
    );

    return_if_not!(Some(serialized) = serialized);

    let json_str = serde_json::to_string_pretty(&serialized.to_json()).unwrap();

    let json_bytes = json_str.into_bytes();

    if let Err(err) = std::fs::write(&inspected_asset.path, &json_bytes) {
        eprintln!("failed to write to {:?}. {}", &inspected_asset.path, err);
        return;
    }
}

pub fn destroy_non_inspected_entity_system(
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    selected_file: Res<SelectedFileResource>,
) {
    return_if!(selected_file.potential_asset_key == inspected_asset.asset_key);

    return_if_not!(Some(world) = &mut simulated_world.0);

    for entity in world.world.data().entities().query::<EntityId>().iter().collect::<Vec<_>>() {
        world.world.data_mut().entities_mut().destroy_entity(entity);
    }

    inspected_asset.spawned_prefab = EntityId::EMPTY;
}

pub fn spawn_inspected_prefab_system(
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    selected_file: Res<SelectedFileResource>,
) {
    return_if!(selected_file.potential_asset_key == inspected_asset.asset_key);

    return_if_not!(Some(world) = &mut simulated_world.0);

    let (res, mut ent, _evt) = world.world.data_mut().into_tuple_mut();

    return_if!(get_asset_type(res.as_ref(), selected_file.potential_asset_key.as_str()) != Some(AssetType::Prefab));

    return_if_not!(
        Some(prefab_handle) = res
            .as_ref()
            .get::<AssetStorageResource<Prefab>>()
            .unwrap()
            .get_registered(selected_file.potential_asset_key.as_str())
            .cloned()
    );

    let instantiated = instantiate_prefab(res.as_ref(), ent.as_mut(), prefab_handle).unwrap_or(EntityId::EMPTY);

    if !ent.contains_entity(instantiated) {
        for entity in ent.query::<EntityId>().iter().collect::<Vec<_>>() {
            ent.destroy_entity(entity);
        }
        return;
    }

    inspected_entity.ent_to_id.clear();
    inspected_entity.id_to_ent.clear();

    for entity in ent.query::<EntityId>().iter() {
        let id = entity.version_index().index + 1;

        inspected_entity.ent_to_id.insert(entity, id);
        inspected_entity.id_to_ent.insert(id, entity);
    }

    inspected_asset.spawned_prefab = instantiated;
}

pub fn update_hierarchy_window_system(
    simulated_world: Res<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut entities: EntitiesHolderMut,
    assets: Res<StandardAssetsResource>,
    file_selected_evt: Evt<FileSelectedEvent>,
) {
    return_if_not!(Some(simulated_world) = simulated_world.0.as_ref());

    // todo
    // if file_selected_evt.is_empty() {
    //     return;
    // }

    let windows = entities
        .query_filtered::<EntityId, WithFilter<HierarchyWindowComponent>>()
        .iter()
        .collect::<Vec<_>>();

    for ent_window in windows {
        let hierarchy_win_c = entities.get_component_mut::<HierarchyWindowComponent>(ent_window).unwrap();

        // todo
        // if !is_cache_old && hierarchy_win_c.is_init {
        //     continue;
        // }

        hierarchy_win_c.is_init = true;

        let content = entities.get_component::<WindowComponent>(ent_window).unwrap().content_container;

        destroy_entity_children(entities.as_mut(), content);

        let mut hierarchy_tree = TreeBuilder::new();

        for (sim_ent, sim_child_c) in simulated_world.world.data().entities().query::<(EntityId, Option<&ChildComponent>)>().iter() {
            let sim_parent = sim_child_c.map(|c| c.parent).filter(|p| simulated_world.world.data().entities().contains_entity(*p));

            match sim_parent {
                Some(sim_parent) => hierarchy_tree.insert_pair(sim_parent, sim_ent),
                None => hierarchy_tree.insert_single(sim_ent),
            };
        }

        let mut hierarchy_tree = hierarchy_tree.build();

        hierarchy_tree.sort_by_key(|n| n.value.version_index().index);
        for node in &mut hierarchy_tree {
            node.iter_nodes_recursively(|n| n.children.sort_by_key(|n| n.value.version_index().index));
        }

        for node in &hierarchy_tree {
            spawn_hierarchy_window_entries_recursively(
                entities.as_mut(),
                simulated_world.world.data().entities(),
                &assets.material_text,
                &assets.material_panel,
                &assets.font,
                content,
                &inspected_entity.ent_to_id,
                node,
                0,
            )
        }
    }
}

fn spawn_hierarchy_window_entries_recursively(
    mut ent: EntitiesHolderMut,
    sim_ent: EntitiesHolderRef,
    material_text: &AssetHandle<StandardMaterial>,
    material_panel: &AssetHandle<StandardMaterial>,
    font: &AssetHandle<Font>,
    parent: EntityId,
    ent_to_id: &HashMap<EntityId, u64>,
    node: &TreeNode<EntityId>,
    depth: usize,
) {
    let simulated_ent = node.value;

    let entity_id = ent_to_id.get(&simulated_ent).map(|n| n.to_string()).unwrap_or_else(|| "_".into());

    let name = sim_ent.get_component::<DebugNameComponent>(simulated_ent)
        .map(|s| s.0.as_str().into())
        .unwrap_or_else(|| format!("e{{{}}}", simulated_ent.version_index().index));

    // todo: text indent to layout indent
    let indent = std::iter::repeat_n(' ', depth).collect::<String>();
    
    let name = format!("{}[{}] {}", indent, entity_id, name).into();

    spawn_hierarchy_window_entry(
        ent.as_mut(),
        &material_text,
        &material_panel,
        &font,
        name,
        parent,
        simulated_ent,
    );

    for child in &node.children {
        spawn_hierarchy_window_entries_recursively(
            ent.as_mut(),
            sim_ent,
            material_text,
            material_panel,
            font,
            parent,
            ent_to_id,
            child,
            depth + 1,
        );
    }
}

pub fn change_inspected_asset_system(selected_file: Res<SelectedFileResource>, mut inspected_asset: ResMut<InspectedAssetResource>) {
    if inspected_asset.asset_key.as_str() != selected_file.potential_asset_key.as_str() {
        inspected_asset.asset_key = selected_file.potential_asset_key.clone();
        inspected_asset.path = selected_file.path.clone();
    }
}

pub fn update_inspector_window_system(
    mut ent: EntitiesHolderMut,
    selected_input_field: Res<SelectedInputFieldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    selected_file: Res<SelectedFileResource>,
    simulated_world: Res<SimulatedWorldResource>,
    assets: Res<StandardAssetsResource>,
    file_selected_evt: Evt<FileSelectedEvent>,
    inspected_asset_edited_evt: Evt<InspectedAssetEditedEvent>,
    prefab_entity_selected_evt: Evt<PrefabEntitySelectedEvent>,
    input_field_selection_changed_evt: Evt<InputFieldSelectionChangedEvent>,
) {
    // todo
    return_if!(
        file_selected_evt.is_empty()
            && inspected_asset_edited_evt.is_empty()
            && input_field_selection_changed_evt.is_empty()
            && prefab_entity_selected_evt.is_empty()
    );

    let ent_selected_input = selected_input_field.selected;

    let container_q = ent.query::<&InspectorWindowContentComponent>().iter().copied().collect::<Vec<_>>();

    return_if_not!(Some(simulated_world) = &simulated_world.0);

    for window_c in container_q {
        let asset_type_text = ent.get_component_mut::<TextComponent>(window_c.asset_type_text).unwrap();
        asset_type_text.text.clear();

        let Some(asset_type) = get_asset_type(simulated_world.world.data().resources(), selected_file.potential_asset_key.as_str()) else {
            destroy_entity_children(ent.as_mut(), window_c.content_container);
            continue;
        };

        asset_type_text.text.push_str("asset type: ");
        asset_type_text.text.push_str(asset_type.serialized_str());

        let inspected_asset = InspectedAsset {
            asset_key: selected_file.potential_asset_key.clone().into(),
            asset_type,
        };

        // todo
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        // let mut err_handler = |_| ();

        // todo
        if asset_type == AssetType::Prefab {
            let inspected_components = record_into_prefab_components(
                simulated_world.world.data().resources(),
                simulated_world.world.data().entities(),
                inspected_entity.selected_entity,
                &inspected_entity.ent_to_id,
            );

            update_prefab_components_ent(
                ent.as_mut(),
                window_c.content_container,
                ent_selected_input,
                &inspected_components,
                assets.material_panel.clone(),
                assets.material_text.clone(),
                assets.font.clone(),
            );

            continue;
        }

        let serialized = save_with_asset_serializers_from_world(simulated_world.world.data().resources(), None, |serializers_local| {
            let serializers_global = simulated_world.world.data().resources().get::<SerializersResource>().unwrap();
            
            let serializer_ctx_state = serializers_global.to_ctx_state();
            let mut serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local)
                .into_ctx(&mut err_handler);

            inspected_asset.to_serialized(serializer_ctx)
        })
        .unwrap();

        let ent_last = ent
            .get_component::<ParentComponent>(window_c.content_container)
            .map(|p| p.children.first())
            .flatten()
            .copied()
            .unwrap_or(EntityId::EMPTY);

        spawn_serialized(
            ent.as_mut(),
            ent_last,
            window_c.content_container,
            ent_selected_input,
            "".into(),
            &serialized,
            assets.material_panel.clone(),
            assets.material_text.clone(),
            assets.font.clone(),
        );
    }
}

fn update_prefab_components_ent(
    mut ent: EntitiesHolderMut,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    components: &FfiIndexMap<FfiString, SerializedValue>,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) {
    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_parent) {
        while ent.get_component_mut::<ParentComponent>(ent_parent).unwrap().children.len() > components.len() as u64 {
            let popped_ent = ent.get_component_mut::<ParentComponent>(ent_parent).unwrap().children.pop().unwrap();
            destroy_entity_and_children(ent.as_mut(), popped_ent);
        }

        for (i, (component_id, component_value)) in components.iter().enumerate() {
            let ent_last = ent.get_component_mut::<ParentComponent>(ent_parent).unwrap().children.get(i as u64).copied().unwrap_or(EntityId::EMPTY);

            update_prefab_component_ent(
                ent.as_mut(),
                ent_last,
                ent_parent,
                ent_selected_input,
                component_id,
                component_value,
                material_panel.clone(),
                material_text.clone(),
                font.clone(),
            )
        }
    }
}

fn update_prefab_component_ent(
    mut ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    component_id: &str,
    component_value: &SerializedValue,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) {
    if let Some(&serialized_component_c) = ent.get_component::<SerializedComponentComponent>(ent_last)
    && let Some(container_parent_c) = ent.get_component::<ParentComponent>(serialized_component_c.component_data_container)
    && container_parent_c.children.len() == 1
    && let data_ent = container_parent_c.children[0]
    && let Some(text_c) = ent.get_component_mut::<TextComponent>(serialized_component_c.component_id_text) {
        text_c.text = component_id.into();
        
        spawn_serialized(
            ent.as_mut(),
            data_ent,
            serialized_component_c.component_data_container,
            ent_selected_input,
            "".into(),
            component_value,
            material_panel.clone(),
            material_text.clone(),
            font.clone(),
        );

        return;
    }

    let comp_ent = spawn_default_layout_ent(ent.as_mut(), ent_parent, false);
    let (_, componet_id_text_ent) = spawn_text_ent(
        ent.as_mut(),
        comp_ent,
        component_id.into(),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );
    let (btn_remove, _) = spawn_text_ent(
        ent.as_mut(),
        comp_ent,
        "X".into(),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );
    ent.set_components(btn_remove, (
        ComponentRemoveButton { component: comp_ent },
        ButtonComponent::default(),
    ));
    let comp_data_container = spawn_default_layout_ent(ent.as_mut(), comp_ent, false);

    ent.set_component(comp_ent, SerializedComponentComponent {
        component_id_text: componet_id_text_ent,
        component_data_container: comp_data_container,
    });
    
    spawn_serialized(
        ent.as_mut(),
        EntityId::EMPTY,
        comp_data_container,
        ent_selected_input,
        "".into(),
        component_value,
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );
}

// todo
fn tree_to_debug_str(tree: &[TreeNode<EntityId>]) -> String {
    fn log_tree_internal(s: &mut String, tree: &[TreeNode<EntityId>]) {
        if tree.is_empty() {
            return;
        }

        s.push_str(" { ");

        for node in tree {
            s.push_str(&node.value.version_index().index.to_string());

            log_tree_internal(s, &node.children);
            
            s.push_str(", ");
        }

        s.push_str("}");
    }

    let mut s = String::new();
    log_tree_internal(&mut s, tree);

    s
}