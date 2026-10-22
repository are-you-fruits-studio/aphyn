use std::ffi::OsStr;

use fruits_engine::tree::{TreeBuilder, TreeNode};

use crate::{
    features::{
        asset_serialization::{InspectedAsset, get_asset_type}, foldout::{FOLDOUT_WIDTH, FoldoutButtonComponent, FoldoutKey, FoldoutResource, FoldoutToggledEvent, spawn_foldout_arrow}, generic_dropdown::{GenericDropdownResource, GenericDropdownStateTy}, input_field::{InputFieldComponent, InputFieldSelectionChangedEvent, InputFieldTextChangedEvent, SelectedInputFieldResource}, inspector_window::{
            FIELD_HEIGHT, data::*, utils::{
                entries::{parse_serialized, spawn_default_layout_ent, spawn_hierarchy_window_entry, spawn_serialized, spawn_text_ent}, find_in_parents, serialization::{deserialize_asset_to_world_res, save_asset_from_world_res}, subsequence_match_ignore_case,
            },
        }, number_drag::NumberInputFieldDraggedEvent, project_window_selection::{FileOpenedEvent, FileSelectedEvent, SelectedFileResource}, world_preload::{SimulatedWorld, SimulatedWorldResource},
    }, prefabs::WindowComponent, *,
};

pub fn select_entity_system(
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    file_selected_evt: Evt<FileSelectedEvent>,
    entry_q: WorldQuery<(&HierarchyWindowEntryComponent, &ButtonComponent)>,
    mut entity_selected_evt: EvtMut<EntitySelectedEvent>,
) {
    if !file_selected_evt.is_empty() {
        inspected_entity.selected_entity = EntityId::EMPTY;
    }

    return_if_not!(Some((entry_c, _)) = entry_q.iter().find(|(_, b)| b.was_clicked_this_frame || b.was_right_clicked_this_frame));

    inspected_entity.selected_entity = entry_c.simulated_entity;
    *selected_file = SelectedFileResource::default();
    entity_selected_evt.push(EntitySelectedEvent);
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

fn get_inspected_component_id(ent: EntitiesHolderRef, ent_component: EntityId) -> Option<FfiString> {
    let serialized_component_c = ent.get_component::<SerializedComponentComponent>(ent_component)?;
    let component_id_text_c = ent.get_component::<TextComponent>(serialized_component_c.component_id_text)?;
    Some(component_id_text_c.text.clone())
}

fn remove_inspected_component(simulated_world: &mut SimulatedWorld, inspected_entity: &InspectedEntityResource, component_id: &str) -> bool {
    let (_, mut sim_ent, _) = simulated_world.world.data_mut().into_tuple_mut();

    let mut has_component = false;
    sim_ent.get_all_components(inspected_entity.selected_entity, |c| has_component |= c.type_info().short().name() == component_id);

    has_component && sim_ent.remove_component_any(inspected_entity.selected_entity, component_id).is_some()
}

pub fn open_component_context_menu_system(
    ent: EntitiesHolderRef,
    input_res: Res<InputResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut context_menu_res: ResMut<ComponentContextMenuResource>,
) {
    let Some(ent_component) = ent
        .query::<(&ButtonComponent, &ComponentHeaderComponent)>()
        .iter()
        .find(|(b, _)| b.was_right_clicked_this_frame)
        .map(|(_, header_c)| header_c.component)
    else {
        return;
    };

    return_if_not!(Some(component_id) = get_inspected_component_id(ent, ent_component));

    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id = generic_dropdown_res.request(vec![FfiString::from("Remove component")].into(), position);
    context_menu_res.component_id = component_id;
}

pub fn apply_component_context_menu_system(
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut context_menu_res: ResMut<ComponentContextMenuResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    let context_menu = &mut *context_menu_res;

    return_if_not!(Some(_) = generic_dropdown_res.take_selected(&mut context_menu.request_id));
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if!(!is_inspected_entity_shown(simulated_world, &inspected_entity));

    if remove_inspected_component(simulated_world, &inspected_entity, context_menu.component_id.as_str()) {
        asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
    }
}

pub fn add_component_system(
    ent: EntitiesHolderRef,
    button_click_evt: Evt<ButtonClickEvent>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    return_if!(!is_inspected_entity_shown(simulated_world, &inspected_entity));

    for button_click_evt in button_click_evt.iter() {
        continue_if_not!(Some(add_component_variant_c) = ent.get_component::<AddComponentVariantComponent>(button_click_evt.entity));

        let (sim_res, sim_ent, _) = simulated_world.world.data_mut().into_tuple_mut();

        let did_add = deserialize_entity_component_at_path(
            sim_res.as_ref(),
            sim_ent,
            inspected_entity.selected_entity,
            &add_component_variant_c.component_id,
            "",
            &SerializedValue::Null,
        );

        if did_add {
            asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
        }
    }
}

pub fn update_add_component_variants_system(
    mut ent: EntitiesHolderMut,
    selected_input: Res<SelectedInputFieldResource>,
    assets: Res<StandardAssetsResource>,
    simulated_world: Res<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
) {
    for entity in ent.query::<&AddComponentInputComponent>().iter().map(|c| c.variants_container).collect::<Vec<_>>() {
        destroy_entity_children(ent.as_mut(), entity);
    }

    let Some(simulated_world) = &simulated_world.0 else {
        return;
    };

    if !is_inspected_entity_shown(simulated_world, &inspected_entity) {
        return;
    }

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

pub fn update_add_component_input_visibility_system(
    mut ent: EntitiesHolderMut,
    mut selected_input: ResMut<SelectedInputFieldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    simulated_world: Res<SimulatedWorldResource>,
    mut input_field_selection_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
) {
    let is_shown = simulated_world.0.as_ref().is_some_and(|w| is_inspected_entity_shown(w, &inspected_entity));

    let inputs = ent
        .query::<(EntityId, &AddComponentInputComponent, &InputFieldComponent)>()
        .iter()
        .map(|(e, add_c, input_c)| (e, add_c.variants_container, input_c.text))
        .collect::<Vec<_>>();

    for (ent_input, ent_variants_container, ent_text) in inputs {
        if let Some(disableable_c) = ent.get_component_mut::<LocalDisableableComponent>(ent_input) {
            disableable_c.is_disabled = !is_shown;
        }

        // disabled rects still take layout space, so collapse it too
        if let Some(local_rect_c) = ent.get_component_mut::<LocalRectComponent>(ent_input) {
            local_rect_c.scale.y = if is_shown { FIELD_HEIGHT } else { UiVal::px(0.0) }.into();
        }

        continue_if!(is_shown);

        if selected_input.selected == ent_input {
            selected_input.selected = EntityId::EMPTY;
            input_field_selection_changed_evt.push(InputFieldSelectionChangedEvent);
            destroy_entity_children(ent.as_mut(), ent_variants_container);
        }

        if let Some(text_c) = ent.get_component_mut::<TextComponent>(ent_text) {
            text_c.text.clear();
        }
    }
}

fn is_inspected_entity_shown(simulated_world: &SimulatedWorld, inspected_entity: &InspectedEntityResource) -> bool {
    simulated_world.world.data().entities().contains_entity(inspected_entity.selected_entity)
}

// EntityId::EMPTY for a scene, where root entities have no parent
fn get_hierarchy_root_parent(simulated_world: &SimulatedWorld, opened_asset: &OpenedEntitiesAssetResource) -> Option<EntityId> {
    match opened_asset.asset_type {
        Some(AssetType::Prefab) => {
            Some(opened_asset.prefab_root).filter(|&e| simulated_world.world.data().entities().contains_entity(e))
        }
        Some(AssetType::Scene) => Some(EntityId::EMPTY),
        _ => None,
    }
}

pub fn open_hierarchy_context_menu_system(
    input_res: Res<InputResource>,
    entry_q: WorldQuery<(&HierarchyWindowEntryComponent, &ButtonComponent)>,
    empty_area_q: WorldQuery<&ButtonComponent, WithFilter<HierarchyWindowEmptyAreaComponent>>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
    simulated_world: Res<SimulatedWorldResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut context_menu_res: ResMut<HierarchyContextMenuResource>,
) {
    return_if_not!(Some(simulated_world) = &simulated_world.0);
    return_if_not!(Some(root_parent) = get_hierarchy_root_parent(simulated_world, &opened_asset));

    let (target, actions) = if let Some((entry_c, _)) = entry_q.iter().find(|(_, b)| b.was_right_clicked_this_frame) {
        let target = entry_c.simulated_entity;
        return_if!(!simulated_world.world.data().entities().contains_entity(target));

        match target == root_parent {
            true => (target, vec![HierarchyContextAction::CreateEntity]),
            false => (target, vec![HierarchyContextAction::CreateEntity, HierarchyContextAction::DeleteEntity]),
        }
    } else if empty_area_q.iter().any(|b| b.was_right_clicked_this_frame) {
        (root_parent, vec![HierarchyContextAction::CreateEntity])
    } else {
        return;
    };

    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id = generic_dropdown_res.request(actions.iter().map(|a| FfiString::from(a.label())).collect(), position);
    context_menu_res.target = target;
    context_menu_res.actions = actions;
}

pub fn apply_hierarchy_context_menu_system(
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut context_menu_res: ResMut<HierarchyContextMenuResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    let context_menu = &mut *context_menu_res;

    return_if_not!(Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id));
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some(root_parent) = get_hierarchy_root_parent(simulated_world, &opened_asset));

    let target = context_menu.target;
    let (_, mut sim_ent, _) = simulated_world.world.data_mut().into_tuple_mut();

    return_if!(target != EntityId::EMPTY && !sim_ent.contains_entity(target));

    match context_menu.actions[idx as usize] {
        HierarchyContextAction::CreateEntity => {
            let entity = sim_ent.create_entity();

            if target != EntityId::EMPTY {
                sim_ent.set_component(entity, ChildComponent { parent: target });

                match sim_ent.get_component_mut::<ParentComponent>(target) {
                    Some(parent_c) => parent_c.children.push(entity),
                    None => _ = sim_ent.set_component(target, ParentComponent { children: vec![entity].into() }),
                }
            }

            inspected_entity.selected_entity = entity;
            *selected_file = SelectedFileResource::default();
            asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
        }
        HierarchyContextAction::DeleteEntity => {
            return_if!(target == EntityId::EMPTY || target == root_parent);

            if let Some(&ChildComponent { parent }) = sim_ent.get_component::<ChildComponent>(target)
                && let Some(parent_c) = sim_ent.get_component_mut::<ParentComponent>(parent)
            {
                parent_c.children = parent_c.children.iter().copied().filter(|&e| e != target).collect();
            }

            let mut subtree = vec![target];
            let mut i = 0;
            while let Some(&e) = subtree.get(i) {
                if let Some(parent_c) = sim_ent.get_component::<ParentComponent>(e) {
                    subtree.extend(parent_c.children.iter().copied());
                }
                i += 1;
            }

            for e in subtree {
                sim_ent.destroy_entity(e);
            }

            inspected_entity.selected_entity = EntityId::EMPTY;
            asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
        }
    }
}

// todo: maybe merge all apply_inspector_xxx_to_simulated_world_system with a common event or smth.

pub fn apply_inspector_field_text_change_to_simulated_world_system(
    text_changed_evt: Evt<InputFieldTextChangedEvent>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    for text_changed_evt in text_changed_evt.iter() {
        let edited = apply_inspector_serialized_value_to_simulated_world(
            ent,
            text_changed_evt.input_field,
            simulated_world,
            &inspected_entity,
            &inspected_asset,
            &open_project,
        );

        if let Some(edited) = edited {
            asset_edited_evt.push(edited);
        };
    }
}

pub fn apply_inspector_number_drag_to_simulated_world_system(
    dragged_evt: Evt<NumberInputFieldDraggedEvent>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    for dragged_evt in dragged_evt.iter() {
        let edited = apply_inspector_serialized_value_to_simulated_world(
            ent,
            dragged_evt.input_field,
            simulated_world,
            &inspected_entity,
            &inspected_asset,
            &open_project,
        );

        if let Some(edited) = edited {
            asset_edited_evt.push(edited);
        };
    }
}

pub fn open_inspector_enum_dropdown_system(
    input_res: Res<InputResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    button_q: WorldQuery<&ButtonComponent>,
    mut enum_metadata_q: WorldQuery<&mut SerializedEnumMetadataComponent>,
) {
    for enum_metadata_c in enum_metadata_q.iter_mut() {
        continue_if_not!(Some(button_c) = button_q.get(enum_metadata_c.value_button));
        continue_if!(!button_c.was_clicked_this_frame);

        let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
        enum_metadata_c.dropdown_request_id = generic_dropdown_res.request(enum_metadata_c.variants.clone(), position);
    }
}

pub fn apply_inspector_dropdown_change_to_simulated_world_system(
    mut ent: EntitiesHolderMut,
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    let selections = ent
        .query::<(EntityId, &SerializedEnumMetadataComponent)>()
        .iter()
        .filter_map(|(ent_enum_meta, enum_metadata_c)| {
            let state = generic_dropdown_res.get_state(enum_metadata_c.dropdown_request_id);
            let GenericDropdownStateTy::Selected { idx } = state.ty else {
                return None;
            };
            Some((ent_enum_meta, state.variants.get(idx)?.clone()))
        })
        .collect::<Vec<_>>();

    for (ent_enum_meta, variant) in selections {
        let enum_metadata_c = ent.get_component_mut::<SerializedEnumMetadataComponent>(ent_enum_meta).unwrap();
        enum_metadata_c.dropdown_request_id = 0;
        let ent_value_text = enum_metadata_c.value_text;

        continue_if_not!(Some(text_c) = ent.get_component_mut::<TextComponent>(ent_value_text));
        text_c.text = variant;

        let ent = ent.as_ref();

        continue_if_not!(Some(ent_composite) = find_in_parents(ent.query::<&ChildComponent>(), ent_enum_meta, |e| {
            ent.get_component::<SerializedValueComponent>(e).map(|_| e)
        }));

        let edited = apply_inspector_serialized_value_to_simulated_world(
            ent,
            ent_composite,
            simulated_world,
            &inspected_entity,
            &inspected_asset,
            &open_project,
        );

        if let Some(edited) = edited {
            asset_edited_evt.push(edited);
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
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
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
        continue_if_not!(Some(target) = get_inspected_target(ent, ent_composite, simulated_world, &inspected_entity, &inspected_asset));

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
            asset_edited_evt.push(target.edited_event());
        };
    }
}

enum InspectedTarget {
    Asset(AssetType),
    PrefabComponent { component_id: FfiString },
}

impl InspectedTarget {
    fn edited_event(&self) -> AssetEditedEvent {
        match self {
            InspectedTarget::Asset(_) => AssetEditedEvent::InspectedAsset,
            InspectedTarget::PrefabComponent { .. } => AssetEditedEvent::OpenedEntitiesAsset,
        }
    }
}

fn get_inspected_target(
    ent: EntitiesHolderRef,
    ent_serialized: EntityId,
    simulated_world: &SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &InspectedAssetResource,
) -> Option<InspectedTarget> {
    if is_inspected_entity_shown(simulated_world, inspected_entity) {
        let serialized_component_c = find_in_parents(ent.query::<&ChildComponent>(), ent_serialized, |e| {
            ent.get_component::<SerializedComponentComponent>(e)
        })?;

        let component_id_text_c = ent.get_component::<TextComponent>(serialized_component_c.component_id_text)?;

        return Some(InspectedTarget::PrefabComponent { component_id: component_id_text_c.text.clone() });
    }

    let asset_type = get_asset_type(simulated_world.world.data().resources(), inspected_asset.asset_key.as_str())?;

    if is_entities_asset(asset_type) {
        return None;
    }

    Some(InspectedTarget::Asset(asset_type))
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
        InspectedTarget::PrefabComponent { component_id } => record_entity_components(
            res,
            simulated_world.world.data().entities(),
            inspected_entity.selected_entity,
            EntityIdEncoder::World(simulated_world.world.data().entities()),
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
            deserialize_entity_component_at_path(
                sim_res.as_ref(),
                sim_ent,
                inspected_entity.selected_entity,
                component_id,
                path,
                serialized,
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
) -> Option<AssetEditedEvent> {
    let serialized_value_c = ent.get_component::<SerializedValueComponent>(ent_serialized)?;

    let target = get_inspected_target(ent, ent_serialized, simulated_world, inspected_entity, inspected_asset)?;

    let did_asset_load = deserialize_to_simulated_world(
        &target,
        &serialized_value_c.path,
        &parse_serialized(ent, ent_serialized),
        simulated_world,
        inspected_entity,
        inspected_asset,
        open_project,
    );

    did_asset_load.then(|| target.edited_event())
}

pub fn save_simulated_entities_to_asset_system(
    mut simulated_world: ResMut<SimulatedWorldResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
) {
    // todo: support saving assets in play mode
    return_if!(simulated_world.is_playing());

    let Some(simulated_world) = &mut simulated_world.0 else {
        return;
    };

    let asset_key = opened_asset.asset_key.as_str();
    let (sim_res, sim_ent, _) = simulated_world.world.data_mut().into_tuple_mut();

    return_if!(get_asset_type(sim_res.as_ref(), asset_key) != opened_asset.asset_type);

    match opened_asset.asset_type {
        Some(AssetType::Prefab) => {
            return_if_not!(Some(prefab) = record_into_prefab(sim_res.as_ref(), sim_ent.as_ref(), opened_asset.prefab_root));
            replace_registered_asset(sim_res, asset_key, prefab);
        }
        Some(AssetType::Scene) => {
            return_if_not!(Some(scene) = record_into_scene(sim_res.as_ref(), sim_ent.as_ref()));
            replace_registered_asset(sim_res, asset_key, scene);
        }
        _ => {}
    }
}

fn replace_registered_asset<T: 'static + Send + Sync>(res: ResourcesHolderMut, asset_key: &str, asset: T) {
    return_if_not!(Some(storage) = res.into_get_mut::<AssetStorageResource<T>>());
    return_if_not!(Some(handle) = storage.get_registered(asset_key).cloned());

    if let Some(stored) = storage.get_mut(&handle) {
        *stored = asset;
    }
}

pub fn save_inspected_asset_from_simulated_world_to_file_system(
    asset_edited_evt: Evt<AssetEditedEvent>,
    simulated_world: Res<SimulatedWorldResource>,
    selected_file: Res<SelectedFileResource>,
    inspected_asset: Res<InspectedAssetResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
) {
    // todo: support saving assets in play mode
    return_if!(simulated_world.is_playing());
    return_if_not!(Some(simulated_world) = &simulated_world.0);

    let res = simulated_world.world.data().resources();

    let is_inspected_asset_edited = asset_edited_evt.iter().any(|e| *e == AssetEditedEvent::InspectedAsset);
    let is_opened_asset_edited = asset_edited_evt.iter().any(|e| *e == AssetEditedEvent::OpenedEntitiesAsset);

    if is_inspected_asset_edited || inspected_asset.asset_key != selected_file.potential_asset_key {
        save_asset_from_world_res_to_file(res, inspected_asset.asset_key.as_str(), &inspected_asset.path);
    }

    if is_opened_asset_edited {
        save_asset_from_world_res_to_file(res, opened_asset.asset_key.as_str(), &opened_asset.path);
    }
}

fn save_asset_from_world_res_to_file(res: ResourcesHolderRef, asset_key: &str, path: &OsStr) {
    return_if_not!(Some(serialized) = save_asset_from_world_res(res, asset_key));

    let json_str = serde_json::to_string_pretty(&serialized.to_json()).unwrap();

    let json_bytes = json_str.into_bytes();

    if let Err(err) = std::fs::write(path, &json_bytes) {
        eprintln!("failed to write to {:?}. {}", path, err);
    }
}

pub fn open_entities_asset_system(
    file_opened_evt: Evt<FileOpenedEvent>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    selected_file: Res<SelectedFileResource>,
) {
    return_if!(file_opened_evt.is_empty());
    return_if!(simulated_world.is_playing());

    return_if_not!(Some(world) = &mut simulated_world.0);

    open_entities_asset(world, &mut opened_asset, &mut inspected_entity, &selected_file.potential_asset_key, &selected_file.path);
}

pub fn open_entities_asset(
    world: &mut SimulatedWorld,
    opened_asset: &mut OpenedEntitiesAssetResource,
    inspected_entity: &mut InspectedEntityResource,
    asset_key: &str,
    path: &OsStr,
) {
    let (res, mut ent, _evt) = world.world.data_mut().into_tuple_mut();

    let asset_type = get_asset_type(res.as_ref(), asset_key);

    return_if!(!asset_type.is_some_and(is_entities_asset));

    *inspected_entity = InspectedEntityResource::default();
    *opened_asset = OpenedEntitiesAssetResource::default();

    for entity in ent.query::<EntityId>().iter().collect::<Vec<_>>() {
        ent.destroy_entity(entity);
    }

    let mut prefab_root = EntityId::EMPTY;

    match asset_type {
        Some(AssetType::Prefab) => {
            return_if_not!(Some(prefab_handle) = res.as_ref().get::<AssetStorageResource<Prefab>>().and_then(|s| s.get_registered(asset_key)).cloned());

            prefab_root = instantiate_prefab(res.as_ref(), ent.as_mut(), prefab_handle).unwrap_or(EntityId::EMPTY);

            if !ent.contains_entity(prefab_root) {
                for entity in ent.query::<EntityId>().iter().collect::<Vec<_>>() {
                    ent.destroy_entity(entity);
                }
                return;
            }
        }
        Some(AssetType::Scene) => {
            return_if_not!(Some(scene_handle) = res.as_ref().get::<AssetStorageResource<Scene>>().and_then(|s| s.get_registered(asset_key)).cloned());

            instantiate_scene(res.as_ref(), ent.as_mut(), scene_handle);
        }
        _ => return,
    }

    *opened_asset = OpenedEntitiesAssetResource {
        asset_key: asset_key.to_string(),
        path: path.to_owned(),
        asset_type,
        prefab_root,
    };
}

fn is_entities_asset(asset_type: AssetType) -> bool {
    matches!(asset_type, AssetType::Prefab | AssetType::Scene)
}

pub fn update_hierarchy_window_system(
    simulated_world: Res<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
    foldout_res: Res<FoldoutResource>,
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

        let ent_scroll_content = entities.get_component::<WindowComponent>(ent_window).unwrap().content_container;
        continue_if_not!(Some(&HierarchyWindowContentComponent { entries_container: content }) = entities.get_component::<HierarchyWindowContentComponent>(ent_scroll_content));

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
                &opened_asset.asset_key,
                &foldout_res,
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
    asset_key: &str,
    foldout_res: &FoldoutResource,
    node: &TreeNode<EntityId>,
    depth: usize,
) {
    let simulated_ent = node.value;

    let entity_id = sim_ent.encode_entity_as_id(simulated_ent);

    let name = sim_ent.get_component::<DebugNameComponent>(simulated_ent)
        .map(|s| s.0.as_str().into())
        .unwrap_or_else(|| format!("e{{{}}}", entity_id));

    let name = format!("[{}] {}", entity_id, name).into();

    let (ent_name_container, ent_name) = spawn_hierarchy_window_entry(
        ent.as_mut(),
        &material_text,
        &material_panel,
        &font,
        name,
        parent,
        simulated_ent,
    );

    let key = FoldoutKey::HierarchyEntity { asset_key: asset_key.to_string(), entity_id };
    let is_expanded = foldout_res.is_expanded(&key);
    let foldout = if node.children.is_empty() {
        None
    } else {
        Some((key, is_expanded))
    };

    spawn_foldout_arrow(
        ent.as_mut(),
        ent_name_container,
        ent_name,
        depth as f32 * FOLDOUT_WIDTH,
        foldout,
        material_panel,
        material_text,
        font,
    );

    return_if!(!is_expanded);

    for child in &node.children {
        spawn_hierarchy_window_entries_recursively(
            ent.as_mut(),
            sim_ent,
            material_text,
            material_panel,
            font,
            parent,
            asset_key,
            foldout_res,
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
    asset_edited_evt: Evt<AssetEditedEvent>,
    entity_selected_evt: Evt<EntitySelectedEvent>,
    input_field_selection_changed_evt: Evt<InputFieldSelectionChangedEvent>,
    foldout_toggled_evt: Evt<FoldoutToggledEvent>,
    foldout_res: Res<FoldoutResource>,
) {
    // todo
    return_if!(
        file_selected_evt.is_empty()
            && asset_edited_evt.is_empty()
            && input_field_selection_changed_evt.is_empty()
            && entity_selected_evt.is_empty()
            && foldout_toggled_evt.is_empty()
    );

    let ent_selected_input = selected_input_field.selected;

    let container_q = ent.query::<&InspectorWindowContentComponent>().iter().copied().collect::<Vec<_>>();

    return_if_not!(Some(simulated_world) = &simulated_world.0);

    for window_c in container_q {
        let asset_type_text = ent.get_component_mut::<TextComponent>(window_c.asset_type_text).unwrap();
        asset_type_text.text.clear();

        if is_inspected_entity_shown(simulated_world, &inspected_entity) {
            let inspected_components = record_entity_components(
                simulated_world.world.data().resources(),
                simulated_world.world.data().entities(),
                inspected_entity.selected_entity,
                EntityIdEncoder::World(simulated_world.world.data().entities()),
            );

            update_prefab_components_ent(
                ent.as_mut(),
                window_c.content_container,
                ent_selected_input,
                &inspected_components,
                &foldout_res,
                assets.material_panel.clone(),
                assets.material_text.clone(),
                assets.font.clone(),
            );

            continue;
        }

        let Some(asset_type) = get_asset_type(simulated_world.world.data().resources(), selected_file.potential_asset_key.as_str()) else {
            destroy_entity_children(ent.as_mut(), window_c.content_container);
            continue;
        };

        asset_type_text.text.push_str("asset type: ");
        asset_type_text.text.push_str(asset_type.serialized_str());

        if is_entities_asset(asset_type) {
            destroy_entity_children(ent.as_mut(), window_c.content_container);
            continue;
        }

        let inspected_asset = InspectedAsset {
            asset_key: selected_file.potential_asset_key.clone().into(),
            asset_type,
        };

        // todo
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        // let mut err_handler = |_| ();

        let serialized = save_load_with_asset_serializers_from_world(simulated_world.world.data().resources(), |serializers_local| {
            let serializers_global = simulated_world.world.data().resources().get::<SerializersResource>().unwrap();
            
            let serializer_ctx_state = serializers_global.to_ctx_state();
            let serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local)
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

        if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(window_c.content_container) {
            let extra = parent_c.children.iter().skip(1).copied().collect::<Vec<_>>();
            parent_c.children = parent_c.children.iter().take(1).copied().collect();
            for e in extra {
                destroy_entity_and_children(ent.as_mut(), e);
            }
        }

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
    foldout_res: &FoldoutResource,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) {
    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_parent) {
        let children = std::mem::take(&mut parent_c.children);
        let (kept, foreign): (Vec<_>, Vec<_>) = children.iter().copied().partition(|&e| ent.get_component::<SerializedComponentComponent>(e).is_some());
        ent.get_component_mut::<ParentComponent>(ent_parent).unwrap().children = kept.into();
        for e in foreign {
            destroy_entity_and_children(ent.as_mut(), e);
        }

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
                foldout_res,
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
    foldout_res: &FoldoutResource,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) {
    let key = FoldoutKey::InspectorComponent { component_id: component_id.to_string() };
    let is_expanded = foldout_res.is_expanded(&key);

    if let Some(&serialized_component_c) = ent.get_component::<SerializedComponentComponent>(ent_last)
    && let Some(container_parent_c) = ent.get_component::<ParentComponent>(serialized_component_c.component_data_container)
    && let data_ents = container_parent_c.children.iter().copied().collect::<Vec<_>>()
    && let Some(text_c) = ent.get_component_mut::<TextComponent>(serialized_component_c.component_id_text) {
        text_c.text = component_id.into();

        if let Some(foldout_c) = ent.get_component_mut::<FoldoutButtonComponent>(serialized_component_c.foldout_arrow) {
            foldout_c.key = key;
        }

        let data_ent = match data_ents.as_slice() {
            &[data_ent] if is_expanded => data_ent,
            _ => {
                destroy_entity_children(ent.as_mut(), serialized_component_c.component_data_container);
                EntityId::EMPTY
            }
        };

        set_component_body_padding(ent.as_mut(), serialized_component_c.component_data_container, is_expanded);

        return_if!(!is_expanded);

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
    if let Some(align_c) = ent.get_component_mut::<RectChildAlignComponent>(comp_ent) {
        align_c.anchor = Vec2::new(0.5, 0.0);
        align_c.min_gap = UiVal::px(0.0);
    }

    let (component_header_ent, componet_id_text_ent) = spawn_text_ent(
        ent.as_mut(),
        comp_ent,
        component_id.into(),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );
    ent.set_components(component_header_ent, (
        ComponentHeaderComponent { component: comp_ent },
        ButtonComponent::default(),
    ));
    if let Some(image_c) = ent.get_component_mut::<ImageComponent>(component_header_ent) {
        image_c.color = Vec4::from_array(rgba_f32_array!("#8f8f8fff"));
    }

    let foldout_arrow = spawn_foldout_arrow(
        ent.as_mut(),
        component_header_ent,
        componet_id_text_ent,
        0.0,
        Some((key, is_expanded)),
        &material_panel,
        &material_text,
        &font,
    );

    let comp_data_container = spawn_default_layout_ent(ent.as_mut(), comp_ent, false);
    if let Some(local_rect_c) = ent.get_component_mut::<LocalRectComponent>(comp_data_container) {
        local_rect_c.parent_padding_min.x = COMPONENT_BODY_PADDING;
        local_rect_c.parent_padding_max.x = COMPONENT_BODY_PADDING;
    }
    set_component_body_padding(ent.as_mut(), comp_data_container, is_expanded);

    spawn_component_separator(ent.as_mut(), comp_ent, &material_panel);

    ent.set_component(comp_ent, SerializedComponentComponent {
        component_id_text: componet_id_text_ent,
        component_data_container: comp_data_container,
        foldout_arrow,
    });

    return_if!(!is_expanded);

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

const COMPONENT_BODY_PADDING: UiVal = UiVal::px(4.0);

// SpaceEvenly with a single child pads it by min_gap above and below; collapsed bodies stay zero-height
fn set_component_body_padding(mut ent: EntitiesHolderMut, ent_body: EntityId, is_expanded: bool) {
    if let Some(align_c) = ent.get_component_mut::<RectChildAlignComponent>(ent_body) {
        align_c.spacing = UiSpacing::SpaceEvenly;
        align_c.min_gap = if is_expanded { COMPONENT_BODY_PADDING } else { UiVal::px(0.0) };
    }
}

fn spawn_component_separator(mut ent: EntitiesHolderMut, ent_parent: EntityId, material_panel: &AssetHandle<StandardMaterial>) {
    let ent_separator = ent.create_entity();

    ent.set_components(ent_separator, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(1.0).into()),
            ..Default::default()
        },
        ChildComponent { parent: ent_parent },
        BatchedMeshComponent::default(),
        StandardMaterialComponent { material: material_panel.clone() },
        ImageComponent {
            color: Vec4::from_array(rgba_f32_array!("#3a3a3aff")),
            ..Default::default()
        },
    ));

    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_parent) {
        parent_c.children.push(ent_separator);
    }
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