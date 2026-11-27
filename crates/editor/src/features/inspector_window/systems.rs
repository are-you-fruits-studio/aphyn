use std::ffi::OsStr;

use aphyn::tree::{TreeBuilder, TreeNode};

use crate::{
    features::{
        asset_serialization::get_asset_type,
        clipboard::{EditorClipboard, EditorClipboardResource},
        foldout::{FOLDOUT_WIDTH, FoldoutButtonComponent, FoldoutKey, FoldoutResource, FoldoutToggledEvent, spawn_foldout_arrow},
        game_window::GameViewFocusResource,
        generic_dropdown::{GenericDropdownResource, GenericDropdownStateTy},
        input_field::{
            InputFieldComponent, InputFieldSelectionChangedEvent, InputFieldTextChangedEvent, SelectedInputFieldResource,
            is_editor_ctrl_hotkey_just_pressed, is_editor_hotkey_just_pressed,
        },
        inspector_window::{
            FIELD_HEIGHT,
            data::*,
            utils::{
                assets::{clone_asset_for_inspection, reload_asset_from_inspected_copy},
                entries::{spawn_default_layout_ent, spawn_hierarchy_window_entry, spawn_inspector_value, spawn_text_ent},
                find_in_parents,
                reflection::*,
                serialization::save_asset_from_world_res,
                subsequence_match_ignore_case,
            },
        },
        number_drag::NumberInputFieldDraggedEvent,
        project_window_selection::{FileOpenedEvent, FileSelectedEvent, SelectedFileResource},
        world_preload::{SimulatedWorld, SimulatedWorldResource},
    },
    prefabs::WindowComponent,
    theme::{self, PAL},
    *,
};

pub fn select_entity_system(
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    file_selected_evt: Evt<FileSelectedEvent>,
    entry_q: WorldQuery<(&HierarchyWindowEntryComponent, &ButtonComponent)>,
    empty_area_q: WorldQuery<&ButtonComponent, WithFilter<HierarchyWindowEmptyAreaComponent>>,
    mut entity_selected_evt: EvtMut<EntitySelectedEvent>,
) {
    if !file_selected_evt.is_empty() {
        inspected_entity.selected_entity = EntityId::EMPTY;
    }

    if empty_area_q.iter().any(|b| b.was_clicked_this_frame) && inspected_entity.selected_entity != EntityId::EMPTY {
        inspected_entity.selected_entity = EntityId::EMPTY;
        entity_selected_evt.push(EntitySelectedEvent);
        return;
    }

    return_if_not!(Some((entry_c, _)) = entry_q.iter().find(|(_, b)| b.was_clicked_this_frame));

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
            image_c.color = PAL.surface_selected;
        } else {
            image_c.color = PAL.transparent;
        }
    }
}

fn get_inspected_component_id(ent: EntitiesHolderRef, ent_component: EntityId) -> Option<AnyTypeId> {
    Some(
        ent.get_component::<InspectorComponentComponent>(ent_component)?
            .component_type,
    )
}

// the type of a clipboard component in the current simulated world
fn clipboard_component_type(sim_res: ResourcesHolderRef, value: AnyRef) -> Option<AnyTypeId> {
    Some(sim_res.get::<ReflectionRegistryResource>()?.get_for(value)?.type_id())
}

fn remove_inspected_component(
    simulated_world: &mut SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    component_id: AnyTypeId,
) -> bool {
    let (_, mut sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();

    let mut has_component = false;
    sim_ent.get_all_components(inspected_entity.selected_entity, |c| {
        has_component |= c.type_id() == component_id
    });

    has_component
        && sim_ent
            .remove_component_any(inspected_entity.selected_entity, component_id)
            .is_some()
}

pub fn open_component_context_menu_system(
    ent: EntitiesHolderRef,
    input_res: Res<InputResource>,
    clipboard: Res<EditorClipboardResource>,
    simulated_world: Res<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
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
    return_if_not!(Some(simulated_world) = &simulated_world.0);

    let mut actions = vec![ComponentContextAction::Copy];

    if let Some(clipboard_component_id) = clipboard
        .component()
        .and_then(|value| clipboard_component_type(simulated_world.data.as_ref().resources(), value))
    {
        let mut has_clipboard_component = false;
        simulated_world
            .data
            .as_ref()
            .entities()
            .get_all_components(inspected_entity.selected_entity, |c| {
                has_clipboard_component |= c.type_id() == clipboard_component_id
            });

        match has_clipboard_component {
            true if clipboard_component_id == component_id => actions.push(ComponentContextAction::PasteValues),
            true => {}
            false => actions.push(ComponentContextAction::PasteAsNew),
        }
    }

    actions.push(ComponentContextAction::Remove);

    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id =
        generic_dropdown_res.request(actions.iter().map(|a| FfiString::from(a.label())).collect(), position);
    context_menu_res.component_id = component_id;
    context_menu_res.actions = actions;
}

pub fn apply_component_context_menu_system(
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut context_menu_res: ResMut<ComponentContextMenuResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut clipboard: ResMut<EditorClipboardResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    let context_menu = &mut *context_menu_res;

    return_if_not!(Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id));
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if!(!is_inspected_entity_shown(simulated_world, &inspected_entity));

    let is_edited = match context_menu.actions[idx as usize] {
        ComponentContextAction::Copy => {
            let sim_data = simulated_world.data.as_ref();
            return_if_not!(Some(registry) = sim_data.resources().get::<ReflectionRegistryResource>());

            let value = with_component(
                sim_data.entities(),
                inspected_entity.selected_entity,
                context_menu.component_id,
                |c| reflect_clone(registry, c),
            )
            .flatten();

            match value {
                Some(value) => clipboard.0 = Some(EditorClipboard::Component(EditorValue(value))),
                None => println!(
                    "component type #{} can't be cloned through reflection",
                    context_menu.component_id.id()
                ),
            }

            false
        }
        ComponentContextAction::PasteValues | ComponentContextAction::PasteAsNew => {
            let (sim_res, sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();
            paste_component(sim_res.as_ref(), sim_ent, inspected_entity.selected_entity, &clipboard)
        }
        ComponentContextAction::Remove => remove_inspected_component(simulated_world, &inspected_entity, context_menu.component_id),
    };

    if is_edited {
        asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
    }
}

pub fn open_field_value_context_menu_system(
    ent: EntitiesHolderRef,
    input_res: Res<InputResource>,
    clipboard: Res<EditorClipboardResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut context_menu_res: ResMut<FieldValueContextMenuResource>,
) {
    let value_ent = ent
        .query::<&InspectorFieldComponent>()
        .iter()
        .filter(|field_c| {
            ent.get_component::<ChildComponent>(field_c.key_text)
                .and_then(|child_c| ent.get_component::<ButtonComponent>(child_c.parent()))
                .is_some_and(|button_c| button_c.was_right_clicked_this_frame)
        })
        .filter_map(|field_c| {
            ent.get_component::<ParentComponent>(field_c.value_container)?
                .children()
                .first()
                .copied()
        })
        // an Euler angle's path is a raw quaternion component, so it has no value of its own to copy
        .find(|&value_ent| {
            ent.get_component::<InspectorValueComponent>(value_ent).is_some_and(|value_c| {
                !matches!(
                    value_c.ty,
                    InspectorValueComponentTy::Leaf {
                        kind: InspectorLeafKind::Euler(_),
                        ..
                    }
                )
            })
        });
    return_if_not!(Some(value_ent) = value_ent);

    let mut actions = vec![FieldValueContextAction::Copy];

    if clipboard.value().is_some() {
        actions.push(FieldValueContextAction::Paste);
    }

    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id =
        generic_dropdown_res.request(actions.iter().map(|a| FfiString::from(a.label())).collect(), position);
    context_menu_res.value_ent = value_ent;
    context_menu_res.actions = actions;
}

pub fn apply_field_value_context_menu_system(
    ent: EntitiesHolderRef,
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut context_menu_res: ResMut<FieldValueContextMenuResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut clipboard: ResMut<EditorClipboardResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    let context_menu = &mut *context_menu_res;

    return_if_not!(Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id));
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some(value_c) = ent.get_component::<InspectorValueComponent>(context_menu.value_ent));
    return_if_not!(
        Some(target) = get_inspected_target(
            ent,
            context_menu.value_ent,
            simulated_world,
            &inspected_entity,
            &inspected_asset
        )
    );

    let path = &value_c.path;

    match context_menu.actions[idx as usize] {
        FieldValueContextAction::Copy => {
            let value = with_inspected_value(
                &target,
                simulated_world,
                &inspected_entity,
                &mut inspected_asset,
                |registry, _, root| reflect_clone(registry, resolve(registry, root.as_any_ref(), path.segments())?),
            )
            .flatten();

            match value {
                Some(value) => clipboard.0 = Some(EditorClipboard::Value(EditorValue(value))),
                None => println!("the value at {path} can't be cloned through reflection"),
            }
        }
        FieldValueContextAction::Paste => {
            return_if_not!(Some(value) = clipboard.value());
            return_if_not!(
                Some(registry) = simulated_world
                    .data
                    .as_ref()
                    .resources()
                    .get::<ReflectionRegistryResource>()
                    .cloned()
            );
            return_if_not!(Some(value) = reflect_clone(&registry, value));

            let is_edited = edit_inspected_target(
                &target,
                path,
                InspectorEdit::Replace(value),
                simulated_world,
                &inspected_entity,
                &mut inspected_asset,
                &open_project,
            );

            if is_edited {
                asset_edited_evt.push(target.edited_event());
            }
        }
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

        let (sim_res, mut sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();
        let sim_res = sim_res.as_ref();
        continue_if_not!(Some(registry) = sim_res.get::<ReflectionRegistryResource>());

        let component_id = add_component_variant_c.component_id;
        continue_if!(with_component(sim_ent.as_ref(), inspected_entity.selected_entity, component_id, |_| ()).is_some());
        continue_if_not!(Some(component) = reflect_default(registry, component_id));

        if sim_ent.add_component_any(inspected_entity.selected_entity, component).is_ok() {
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
    for entity in ent
        .query::<&AddComponentInputComponent>()
        .iter()
        .map(|c| c.variants_container)
        .collect::<Vec<_>>()
    {
        destroy_entity_children(ent.as_mut(), entity);
    }

    let Some(simulated_world) = &simulated_world.0 else {
        return;
    };

    if !is_inspected_entity_shown(simulated_world, &inspected_entity) {
        return;
    }

    let sim_res = simulated_world.data.as_ref().resources();
    let Some(registry) = sim_res.get::<ReflectionRegistryResource>() else {
        return;
    };
    let serializers = sim_res.get::<SerializersResource>();

    let Some(add_component_input_c) = ent
        .get_component::<AddComponentInputComponent>(selected_input.selected)
        .copied()
    else {
        return;
    };

    let Some(input_field_c) = ent.get_component::<InputFieldComponent>(selected_input.selected) else {
        return;
    };

    let Some(text_c) = ent.get_component::<TextComponent>(input_field_c.text) else {
        return;
    };

    let searched_text = text_c.text.clone();

    // reflection doesn't know which types are components: every struct or enum that can be created is offered
    let mut keys = registry
        .iter()
        .filter(|r| r.has_default() && (r.as_struct().is_some() || r.as_enum().is_some()))
        .map(|r| (component_label(serializers, r.type_id(), r.type_name()), r.type_id()))
        .collect::<Vec<_>>();
    keys.sort_by(|a, b| a.0.cmp(&b.0));

    for (label, component_id) in keys {
        if !subsequence_match_ignore_case(label.as_str(), searched_text.as_str()) {
            continue;
        }

        let entry_ent = spawn_default_layout_ent(ent.as_mut(), add_component_input_c.variants_container, false);

        let (entry_ent, _) = spawn_text_ent(
            ent.as_mut(),
            entry_ent,
            label,
            assets.material_panel.clone(),
            assets.material_text.clone(),
            assets.font.clone(),
        );

        ent.set_components(
            entry_ent,
            (AddComponentVariantComponent { component_id }, ButtonComponent::default()),
        );
    }
}

pub fn update_add_component_input_visibility_system(
    mut ent: EntitiesHolderMut,
    mut selected_input: ResMut<SelectedInputFieldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    simulated_world: Res<SimulatedWorldResource>,
    mut input_field_selection_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
) {
    let is_shown = simulated_world
        .0
        .as_ref()
        .is_some_and(|w| is_inspected_entity_shown(w, &inspected_entity));

    let inputs = ent
        .query::<(EntityId, &AddComponentInputComponent, &InputFieldComponent)>()
        .iter()
        .map(|(e, add_c, input_c)| (e, add_c.variants_container, add_c.placeholder, input_c.text))
        .collect::<Vec<_>>();

    for (ent_input, ent_variants_container, ent_placeholder, ent_text) in inputs {
        let is_empty = ent.get_component::<TextComponent>(ent_text).is_none_or(|t| t.text.is_empty());
        if let Some(disableable_c) = ent.get_component_mut::<LocalDisableableComponent>(ent_placeholder) {
            disableable_c.is_disabled = !is_empty || selected_input.selected == ent_input;
        }

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
    simulated_world
        .data
        .as_ref()
        .entities()
        .contains_entity(inspected_entity.selected_entity)
}

// EntityId::EMPTY for a scene, where root entities have no parent
fn get_hierarchy_root_parent(simulated_world: &SimulatedWorld, opened_asset: &OpenedEntitiesAssetResource) -> Option<EntityId> {
    match opened_asset.asset_type {
        Some(AssetType::Prefab) => {
            Some(opened_asset.prefab_root).filter(|&e| simulated_world.data.as_ref().entities().contains_entity(e))
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
    clipboard: Res<EditorClipboardResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut context_menu_res: ResMut<HierarchyContextMenuResource>,
) {
    return_if_not!(Some(simulated_world) = &simulated_world.0);
    return_if_not!(Some(root_parent) = get_hierarchy_root_parent(simulated_world, &opened_asset));

    let (target, actions) = if let Some((entry_c, _)) = entry_q.iter().find(|(_, b)| b.was_right_clicked_this_frame) {
        let target = entry_c.simulated_entity;
        return_if!(!simulated_world.data.as_ref().entities().contains_entity(target));

        let mut actions = vec![HierarchyContextAction::CreateEntity, HierarchyContextAction::CopyEntity];

        if clipboard.entities().is_some() {
            actions.push(HierarchyContextAction::PasteEntity);
        }

        if clipboard.component().is_some() {
            actions.push(HierarchyContextAction::PasteComponent);
        }

        if target != root_parent {
            actions.push(HierarchyContextAction::DeleteEntity);
        }

        (target, actions)
    } else if empty_area_q.iter().any(|b| b.was_right_clicked_this_frame) {
        let mut actions = vec![HierarchyContextAction::CreateEntity];

        if clipboard.entities().is_some() {
            actions.push(HierarchyContextAction::PasteEntity);
        }

        (root_parent, actions)
    } else {
        return;
    };

    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id =
        generic_dropdown_res.request(actions.iter().map(|a| FfiString::from(a.label())).collect(), position);
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
    mut clipboard: ResMut<EditorClipboardResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    let context_menu = &mut *context_menu_res;

    return_if_not!(Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id));
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some(root_parent) = get_hierarchy_root_parent(simulated_world, &opened_asset));

    let target = context_menu.target;
    let (sim_res, mut sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();

    return_if!(target != EntityId::EMPTY && !sim_ent.contains_entity(target));

    match context_menu.actions[idx as usize] {
        HierarchyContextAction::CreateEntity => {
            let entity = sim_ent.create_entity();
            attach_to_parent(sim_ent.as_mut(), entity, target);

            inspected_entity.selected_entity = entity;
            *selected_file = SelectedFileResource::default();
            asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
        }
        HierarchyContextAction::CopyEntity => {
            if let Some(prefab) = record_into_prefab(sim_res.as_ref(), sim_ent.as_ref(), target) {
                clipboard.0 = Some(EditorClipboard::Entities(prefab.entities));
            }
        }
        HierarchyContextAction::PasteEntity => {
            return_if_not!(Some(entities) = clipboard.entities());
            return_if_not!(Some(entity) = paste_entities(sim_res.as_ref(), sim_ent.as_mut(), entities, target));

            inspected_entity.selected_entity = entity;
            *selected_file = SelectedFileResource::default();
            asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
        }
        HierarchyContextAction::PasteComponent => {
            return_if!(target == EntityId::EMPTY);

            if paste_component(sim_res.as_ref(), sim_ent.as_mut(), target, &clipboard) {
                asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
            }
        }
        HierarchyContextAction::DeleteEntity => {
            return_if!(target == EntityId::EMPTY || target == root_parent);

            delete_simulated_entity(sim_ent.as_mut(), target);

            inspected_entity.selected_entity = EntityId::EMPTY;
            asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
        }
    }
}

pub fn delete_selected_entity_hotkey_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    if !is_editor_hotkey_just_pressed(&input_res, &selected_input, &game_focus, KeyCode::Delete) {
        return;
    }
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some(root_parent) = get_hierarchy_root_parent(simulated_world, &opened_asset));

    let target = inspected_entity.selected_entity;
    return_if!(target == EntityId::EMPTY || target == root_parent);

    let (_, mut sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();
    return_if!(!sim_ent.contains_entity(target));

    delete_simulated_entity(sim_ent.as_mut(), target);

    inspected_entity.selected_entity = EntityId::EMPTY;
    asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
}

fn delete_simulated_entity(sim_ent: EntitiesHolderMut, target: EntityId) {
    destroy_entity_and_children(sim_ent, target);
}

// EntityId::EMPTY as the parent makes `entity` a scene root
fn attach_to_parent(sim_ent: EntitiesHolderMut, entity: EntityId, parent: EntityId) {
    if parent == EntityId::EMPTY {
        remove_parent(sim_ent, entity);
        return;
    }

    set_parent(sim_ent, entity, parent);
}

fn paste_entities(
    sim_res: ResourcesHolderRef,
    mut sim_ent: EntitiesHolderMut,
    entities: &SerializedEntities,
    parent: EntityId,
) -> Option<EntityId> {
    let root_id = *entities.0.keys().next()?;
    let root = *instantiate_serialized_entities(sim_res, sim_ent.as_mut(), entities).get(&root_id)?;

    attach_to_parent(sim_ent, root, parent);

    Some(root)
}

// pasted onto the same component type if the entity has one, otherwise added
fn paste_component(
    sim_res: ResourcesHolderRef,
    mut sim_ent: EntitiesHolderMut,
    entity: EntityId,
    clipboard: &EditorClipboardResource,
) -> bool {
    let Some(value) = clipboard.component() else {
        return false;
    };
    let Some(registry) = sim_res.get::<ReflectionRegistryResource>() else {
        return false;
    };
    let Some(value) = reflect_clone(registry, value) else {
        return false;
    };
    let component_id = value.type_id();

    if with_component(sim_ent.as_ref(), entity, component_id, |_| ()).is_none() {
        return sim_ent.add_component_any(entity, value).is_ok();
    }

    with_component_mut(sim_ent, entity, component_id, |mut c| c.replace(value).is_ok()).unwrap_or(false)
}

pub fn copy_selected_entity_hotkey_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    animation_window: Res<crate::features::animation_window::AnimationWindowResource>,
    inspected_entity: Res<InspectedEntityResource>,
    simulated_world: Res<SimulatedWorldResource>,
    mut clipboard: ResMut<EditorClipboardResource>,
) {
    return_if!(animation_window.is_focused);
    return_if!(!is_editor_ctrl_hotkey_just_pressed(
        &input_res,
        &selected_input,
        &game_focus,
        KeyCode::KeyC
    ));
    return_if_not!(Some(simulated_world) = &simulated_world.0);

    let sim_data = simulated_world.data.as_ref();

    if let Some(prefab) = record_into_prefab(sim_data.resources(), sim_data.entities(), inspected_entity.selected_entity) {
        clipboard.0 = Some(EditorClipboard::Entities(prefab.entities));
    }
}

pub fn paste_entities_hotkey_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    animation_window: Res<crate::features::animation_window::AnimationWindowResource>,
    clipboard: Res<EditorClipboardResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
) {
    return_if!(animation_window.is_focused);
    return_if!(!is_editor_ctrl_hotkey_just_pressed(
        &input_res,
        &selected_input,
        &game_focus,
        KeyCode::KeyV
    ));
    return_if_not!(Some(entities) = clipboard.entities());
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);
    return_if_not!(Some(root_parent) = get_hierarchy_root_parent(simulated_world, &opened_asset));

    let (sim_res, mut sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();

    let selected = inspected_entity.selected_entity;

    let parent = if sim_ent.contains_entity(selected) {
        selected
    } else if selected == EntityId::EMPTY && selected_file.path.is_empty() {
        root_parent
    } else {
        return;
    };

    return_if_not!(Some(entity) = paste_entities(sim_res.as_ref(), sim_ent.as_mut(), entities, parent));

    inspected_entity.selected_entity = entity;
    *selected_file = SelectedFileResource::default();
    asset_edited_evt.push(AssetEditedEvent::OpenedEntitiesAsset);
}

// todo: maybe merge all apply_inspector_xxx_to_simulated_world_system with a common event or smth.

pub fn apply_inspector_field_text_change_to_simulated_world_system(
    text_changed_evt: Evt<InputFieldTextChangedEvent>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
    mut field_edited_evt: EvtMut<EntityFieldEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    for text_changed_evt in text_changed_evt.iter() {
        let edited = apply_inspector_leaf_to_simulated_world(
            ent,
            text_changed_evt.input_field,
            simulated_world,
            &inspected_entity,
            &mut inspected_asset,
            &open_project,
        );

        push_inspector_edit(edited, &mut asset_edited_evt, &mut field_edited_evt);
    }
}

pub fn apply_inspector_number_drag_to_simulated_world_system(
    dragged_evt: Evt<NumberInputFieldDraggedEvent>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
    mut field_edited_evt: EvtMut<EntityFieldEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    for dragged_evt in dragged_evt.iter() {
        let edited = apply_inspector_leaf_to_simulated_world(
            ent,
            dragged_evt.input_field,
            simulated_world,
            &inspected_entity,
            &mut inspected_asset,
            &open_project,
        );

        push_inspector_edit(edited, &mut asset_edited_evt, &mut field_edited_evt);
    }
}

pub fn open_inspector_enum_dropdown_system(
    input_res: Res<InputResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    button_q: WorldQuery<&ButtonComponent>,
    mut enum_q: WorldQuery<&mut InspectorEnumComponent>,
) {
    for enum_c in enum_q.iter_mut() {
        continue_if_not!(Some(button_c) = button_q.get(enum_c.value_button));
        continue_if!(!button_c.was_clicked_this_frame);

        let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
        enum_c.dropdown_request_id = generic_dropdown_res.request(enum_c.variants.clone(), position);
    }
}

pub fn apply_inspector_dropdown_change_to_simulated_world_system(
    mut ent: EntitiesHolderMut,
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
    mut field_edited_evt: EvtMut<EntityFieldEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    let selections = ent
        .query::<(EntityId, &InspectorEnumComponent)>()
        .iter()
        .filter_map(|(ent_enum, enum_c)| {
            let state = generic_dropdown_res.get_state(enum_c.dropdown_request_id);
            let GenericDropdownStateTy::Selected { idx } = state.ty else {
                return None;
            };
            Some((ent_enum, state.variants.get(idx)?.clone()))
        })
        .collect::<Vec<_>>();

    for (ent_enum, variant) in selections {
        ent.get_component_mut::<InspectorEnumComponent>(ent_enum)
            .unwrap()
            .dropdown_request_id = 0;

        let ent = ent.as_ref();

        continue_if_not!(
            Some(ent_composite) = find_in_parents(ent.query::<&ChildComponent>(), ent_enum, |e| {
                ent.get_component::<InspectorValueComponent>(e).map(|_| e)
            })
        );

        let edited = apply_inspector_edit_to_simulated_world(
            ent,
            ent_composite,
            InspectorEdit::Variant(variant),
            simulated_world,
            &inspected_entity,
            &mut inspected_asset,
            &open_project,
        );

        push_inspector_edit(edited, &mut asset_edited_evt, &mut field_edited_evt);
    }
}

pub fn adjust_non_rigid_composite_system(
    click_evt: Evt<ButtonClickEvent>,
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
    mut field_edited_evt: EvtMut<EntityFieldEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    for click_evt in click_evt.iter() {
        let (ent_composite, edit) = if let Some(btn_add_c) = ent.get_component::<InspectorCompositeAddButton>(click_evt.entity) {
            (btn_add_c.composite, InspectorEdit::AddElement)
        } else if let Some(btn_remove_c) = ent.get_component::<InspectorCompositeRemoveButton>(click_evt.entity) {
            (btn_remove_c.composite, InspectorEdit::RemoveLastElement)
        } else {
            continue;
        };

        let edited = apply_inspector_edit_to_simulated_world(
            ent,
            ent_composite,
            edit,
            simulated_world,
            &inspected_entity,
            &mut inspected_asset,
            &open_project,
        );

        push_inspector_edit(edited, &mut asset_edited_evt, &mut field_edited_evt);
    }
}

pub fn toggle_inspector_checkbox_system(
    ent: EntitiesHolderRef,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    open_project: Res<OpenProjectResource>,
    mut asset_edited_evt: EvtMut<AssetEditedEvent>,
    mut field_edited_evt: EvtMut<EntityFieldEditedEvent>,
) {
    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    let toggled = ent
        .query::<(EntityId, &InspectorValueComponent)>()
        .iter()
        .filter_map(|(ent_value, value_c)| match value_c.ty {
            InspectorValueComponentTy::Checkbox { button, is_checked, .. }
                if ent
                    .get_component::<ButtonComponent>(button)
                    .is_some_and(|button_c| button_c.was_clicked_this_frame) =>
            {
                Some((ent_value, is_checked))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    for (ent_value, is_checked) in toggled {
        let edited = apply_inspector_edit_to_simulated_world(
            ent,
            ent_value,
            InspectorEdit::Leaf {
                kind: InspectorLeafKind::Bool,
                text: (!is_checked).to_string().into(),
            },
            simulated_world,
            &inspected_entity,
            &mut inspected_asset,
            &open_project,
        );

        push_inspector_edit(edited, &mut asset_edited_evt, &mut field_edited_evt);
    }
}

enum InspectedTarget {
    Asset(AssetType),
    PrefabComponent { component_id: AnyTypeId },
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
    ent_value: EntityId,
    simulated_world: &SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &InspectedAssetResource,
) -> Option<InspectedTarget> {
    if is_inspected_entity_shown(simulated_world, inspected_entity) {
        let component_c = find_in_parents(ent.query::<&ChildComponent>(), ent_value, |e| {
            ent.get_component::<InspectorComponentComponent>(e)
        })?;

        return Some(InspectedTarget::PrefabComponent {
            component_id: component_c.component_type,
        });
    }

    let asset_type = get_asset_type(simulated_world.data.as_ref().resources(), inspected_asset.asset_key.as_str())?;

    if is_entities_asset(asset_type) {
        return None;
    }

    Some(InspectedTarget::Asset(asset_type))
}

// the component on the selected simulated entity, or the inspected asset's reflected copy
fn with_inspected_value<R>(
    target: &InspectedTarget,
    simulated_world: &mut SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &mut InspectedAssetResource,
    f: impl FnOnce(&ReflectionRegistry, TransSerializerCtxState, AnyMut) -> R,
) -> Option<R> {
    match target {
        InspectedTarget::Asset(_) => {
            let res = simulated_world.data.as_ref().resources();
            let registry = res.get::<ReflectionRegistryResource>()?;
            let edited = inspected_asset.edited.as_mut()?;

            save_load_with_asset_serializers_from_world(res, EntityIdCtx::default(), |state| {
                f(registry, state, edited.0.as_any_mut())
            })
        }
        InspectedTarget::PrefabComponent { component_id } => {
            let (sim_res, sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();
            let sim_res = sim_res.as_ref();
            let registry = sim_res.get::<ReflectionRegistryResource>()?;
            let (id_to_ent, ent_to_id) = entity_ids_of(sim_ent.as_ref());
            let entity_ids = EntityIdCtx::new(
                EntityIdDecoder::Restricted(&id_to_ent),
                EntityIdEncoder::Restricted(&ent_to_id),
            );

            save_load_with_asset_serializers_from_world(sim_res, entity_ids, |state| {
                with_component_mut(sim_ent, inspected_entity.selected_entity, *component_id, |c| {
                    f(registry, state, c)
                })
            })
            .flatten()
        }
    }
}

fn edit_inspected_target(
    target: &InspectedTarget,
    path: &ReflectPath,
    edit: InspectorEdit,
    simulated_world: &mut SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &mut InspectedAssetResource,
    open_project: &OpenProjectResource,
) -> bool {
    let is_edited = with_inspected_value(
        target,
        simulated_world,
        inspected_entity,
        inspected_asset,
        |registry, state, root| apply_inspector_edit(registry, state, root, path, edit),
    )
    .unwrap_or(false);

    if let InspectedTarget::Asset(asset_type) = target
        && is_edited
        && let Some(edited) = &inspected_asset.edited
    {
        return reload_asset_from_inspected_copy(
            simulated_world.data.as_mut().resources_mut(),
            inspected_asset.asset_key.as_str(),
            *asset_type,
            edited.0.as_any_ref(),
            &(open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH),
        );
    }

    is_edited
}

fn apply_inspector_edit_to_simulated_world(
    ent: EntitiesHolderRef,
    ent_value: EntityId,
    edit: InspectorEdit,
    simulated_world: &mut SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &mut InspectedAssetResource,
    open_project: &OpenProjectResource,
) -> Option<(AssetEditedEvent, Option<EntityFieldEditedEvent>)> {
    let value_c = ent.get_component::<InspectorValueComponent>(ent_value)?;
    let target = get_inspected_target(ent, ent_value, simulated_world, inspected_entity, inspected_asset)?;

    let is_edited = edit_inspected_target(
        &target,
        &value_c.path,
        edit,
        simulated_world,
        inspected_entity,
        inspected_asset,
        open_project,
    );

    let field_edited = match &target {
        InspectedTarget::PrefabComponent { component_id } => Some(EntityFieldEditedEvent {
            entity: inspected_entity.selected_entity,
            component: *component_id,
            path: value_c.path.clone(),
        }),
        InspectedTarget::Asset(_) => None,
    };

    is_edited.then(|| (target.edited_event(), field_edited))
}

fn push_inspector_edit(
    edited: Option<(AssetEditedEvent, Option<EntityFieldEditedEvent>)>,
    asset_edited_evt: &mut EvtMut<AssetEditedEvent>,
    field_edited_evt: &mut EvtMut<EntityFieldEditedEvent>,
) {
    let Some((asset_edited, field_edited)) = edited else {
        return;
    };

    asset_edited_evt.push(asset_edited);
    if let Some(field_edited) = field_edited {
        field_edited_evt.push(field_edited);
    }
}

fn apply_inspector_leaf_to_simulated_world(
    ent: EntitiesHolderRef,
    ent_input: EntityId,
    simulated_world: &mut SimulatedWorld,
    inspected_entity: &InspectedEntityResource,
    inspected_asset: &mut InspectedAssetResource,
    open_project: &OpenProjectResource,
) -> Option<(AssetEditedEvent, Option<EntityFieldEditedEvent>)> {
    let InspectorValueComponentTy::Leaf { kind, .. } = ent.get_component::<InspectorValueComponent>(ent_input)?.ty else {
        return None;
    };
    let input_c = ent.get_component::<InputFieldComponent>(ent_input)?;
    let text = ent.get_component::<TextComponent>(input_c.text)?.text.clone();

    apply_inspector_edit_to_simulated_world(
        ent,
        ent_input,
        InspectorEdit::Leaf { kind, text },
        simulated_world,
        inspected_entity,
        inspected_asset,
        open_project,
    )
}

pub fn save_simulated_entities_to_asset_system(
    mut simulated_world: ResMut<SimulatedWorldResource>,
    opened_asset: Res<OpenedEntitiesAssetResource>,
) {
    // todo: support saving assets in play mode
    return_if!(simulated_world.is_playing());
    return_if!(opened_asset.is_saving_paused);

    let Some(simulated_world) = &mut simulated_world.0 else {
        return;
    };

    let asset_key = opened_asset.asset_key.as_str();
    let (sim_res, sim_ent, _) = simulated_world.data.as_mut().into_tuple_mut();

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

    let res = simulated_world.data.as_ref().resources();

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

    open_entities_asset(
        world,
        &mut opened_asset,
        &mut inspected_entity,
        &selected_file.potential_asset_key,
        &selected_file.path,
    );
}

pub fn open_entities_asset(
    world: &mut SimulatedWorld,
    opened_asset: &mut OpenedEntitiesAssetResource,
    inspected_entity: &mut InspectedEntityResource,
    asset_key: &str,
    path: &OsStr,
) {
    let (res, mut ent, _evt) = world.data.as_mut().into_tuple_mut();

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
            return_if_not!(
                Some(prefab_handle) = res
                    .as_ref()
                    .get::<AssetStorageResource<Prefab>>()
                    .and_then(|s| s.get_registered(asset_key))
                    .cloned()
            );

            prefab_root = instantiate_prefab(res.as_ref(), ent.as_mut(), prefab_handle).unwrap_or(EntityId::EMPTY);

            if !ent.contains_entity(prefab_root) {
                for entity in ent.query::<EntityId>().iter().collect::<Vec<_>>() {
                    ent.destroy_entity(entity);
                }
                return;
            }
        }
        Some(AssetType::Scene) => {
            return_if_not!(
                Some(scene_handle) = res
                    .as_ref()
                    .get::<AssetStorageResource<Scene>>()
                    .and_then(|s| s.get_registered(asset_key))
                    .cloned()
            );

            instantiate_scene(res.as_ref(), ent.as_mut(), scene_handle);
        }
        _ => return,
    }

    *opened_asset = OpenedEntitiesAssetResource {
        asset_key: asset_key.to_string(),
        path: path.to_owned(),
        asset_type,
        prefab_root,
        is_saving_paused: false,
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
    let is_playing = simulated_world.is_playing();
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

        let ent_scroll_content = entities
            .get_component::<WindowComponent>(ent_window)
            .unwrap()
            .content_container;
        continue_if_not!(
            Some(&HierarchyWindowContentComponent {
                entries_container: content,
                source_text,
                badge_text,
            }) = entities.get_component::<HierarchyWindowContentComponent>(ent_scroll_content)
        );

        let (source, badge, badge_color) = match (is_playing, opened_asset.asset_type) {
            (true, _) => ("running game".to_string(), "play", PAL.accent_fg),
            (false, Some(ty)) => (opened_asset.asset_key.clone(), ty.serialized_str(), PAL.text_3),
            (false, None) => ("no scene open".to_string(), "", PAL.text_3),
        };
        if let Some(text_c) = entities.get_component_mut::<TextComponent>(source_text) {
            set_text_if_changed(text_c, &source);
        }
        if let Some(text_c) = entities.get_component_mut::<TextComponent>(badge_text) {
            set_text_if_changed(text_c, badge);
            text_c.color = badge_color;
        }

        destroy_entity_children(entities.as_mut(), content);

        let mut hierarchy_tree = TreeBuilder::new();

        for (sim_ent, sim_child_c) in simulated_world
            .data
            .as_ref()
            .entities()
            .query::<(EntityId, Option<&ChildComponent>)>()
            .iter()
        {
            let sim_parent = sim_child_c
                .map(|c| c.parent())
                .filter(|p| simulated_world.data.as_ref().entities().contains_entity(*p));

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
                simulated_world.data.as_ref().entities(),
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

fn set_text_if_changed(text_c: &mut TextComponent, text: &str) {
    if text_c.text.as_str() != text {
        text_c.text.clear();
        text_c.text.push_str(text);
    }
}

fn spawn_hierarchy_window_entries_recursively(
    mut ent: EntitiesHolderMut,
    sim_ent: EntitiesHolderRef,
    material_text: &AssetHandle<Material>,
    material_panel: &AssetHandle<Material>,
    font: &AssetHandle<Font>,
    parent: EntityId,
    asset_key: &str,
    foldout_res: &FoldoutResource,
    node: &TreeNode<EntityId>,
    depth: usize,
) {
    let simulated_ent = node.value;

    let entity_id = sim_ent.encode_entity_as_id(simulated_ent);

    let name = sim_ent
        .get_component::<NameComponent>(simulated_ent)
        .map(|s| s.0.as_str().into())
        .unwrap_or_else(|| format!("Entity {}", entity_id));

    let name = name.into();

    let (ent_name_container, ent_name) = spawn_hierarchy_window_entry(
        ent.as_mut(),
        &material_text,
        &material_panel,
        &font,
        name,
        parent,
        simulated_ent,
    );

    let ent_id_text = ent.create_entity();
    ent.set_components(
        ent_id_text,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                parent_padding_max: Vec2::new(UiVal::px(theme::PAD_X), UiVal::px(0.0)),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: PAL.text_3,
                font: font.clone(),
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: format!("#{entity_id}").into(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Right,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_id_text, ent_name_container);

    let key = FoldoutKey::HierarchyEntity {
        asset_key: asset_key.to_string(),
        entity_id,
    };
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

pub fn change_inspected_asset_system(
    selected_file: Res<SelectedFileResource>,
    simulated_world: Res<SimulatedWorldResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
) {
    if inspected_asset.asset_key.as_str() != selected_file.potential_asset_key.as_str() {
        inspected_asset.asset_key = selected_file.potential_asset_key.clone();
        inspected_asset.path = selected_file.path.clone();
        inspected_asset.edited = None;
    }

    return_if!(inspected_asset.edited.is_some() || inspected_asset.asset_key.is_empty());
    return_if_not!(Some(simulated_world) = &simulated_world.0);

    inspected_asset.edited =
        clone_asset_for_inspection(simulated_world.data.as_ref().resources(), inspected_asset.asset_key.as_str()).map(EditorValue);
}

// what the inspector calls a component: its serialized name, or its type name when it has no serializer
fn component_label(serializers: Option<&SerializersResource>, id: AnyTypeId, type_name: &str) -> FfiString {
    serializers.and_then(|s| s.name_of(id)).unwrap_or(type_name).into()
}

fn display_component_label(label: &str) -> &str {
    label.strip_prefix("aphyn::").unwrap_or(label)
}

fn record_inspected_components(simulated_world: &SimulatedWorld, entity: EntityId) -> Vec<(AnyTypeId, FfiString, InspectorValue)> {
    let res = simulated_world.data.as_ref().resources();
    let ent = simulated_world.data.as_ref().entities();
    let registry = res.get::<ReflectionRegistryResource>().cloned().unwrap_or_default();
    let entity_ids = EntityIdCtx::new(EntityIdDecoder::Empty, EntityIdEncoder::World(ent));

    save_load_with_asset_serializers_from_world(res, entity_ids, |state| {
        let mut components = Vec::new();

        ent.get_all_components(entity, |c| {
            components.push((
                c.type_id(),
                component_label(res.get::<SerializersResource>(), c.type_id(), c.type_info().short().name()),
                build_inspector_value(&registry, state, c),
            ))
        });

        components
    })
    .unwrap_or_default()
}

pub fn update_inspector_window_system(
    mut ent: EntitiesHolderMut,
    selected_input_field: Res<SelectedInputFieldResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    inspected_asset: Res<InspectedAssetResource>,
    simulated_world: Res<SimulatedWorldResource>,
    assets: Res<StandardAssetsResource>,
    file_selected_evt: Evt<FileSelectedEvent>,
    asset_edited_evt: Evt<AssetEditedEvent>,
    entity_selected_evt: Evt<EntitySelectedEvent>,
    input_field_selection_changed_evt: Evt<InputFieldSelectionChangedEvent>,
    foldout_toggled_evt: Evt<FoldoutToggledEvent>,
    foldout_res: Res<FoldoutResource>,
) {
    return_if_not!(Some(simulated_world) = &simulated_world.0);

    let shown_components = is_inspected_entity_shown(simulated_world, &inspected_entity)
        .then(|| record_inspected_components(simulated_world, inspected_entity.selected_entity));

    let is_entity_changed = shown_components != inspected_entity.shown_components;
    inspected_entity.shown_components = shown_components;

    // todo
    return_if!(
        file_selected_evt.is_empty()
            && asset_edited_evt.is_empty()
            && input_field_selection_changed_evt.is_empty()
            && entity_selected_evt.is_empty()
            && foldout_toggled_evt.is_empty()
            && !is_entity_changed
    );

    let ent_selected_input = selected_input_field.selected;

    let container_q = ent
        .query::<&InspectorWindowContentComponent>()
        .iter()
        .copied()
        .collect::<Vec<_>>();

    for window_c in container_q {
        let asset_type_text = ent.get_component_mut::<TextComponent>(window_c.asset_type_text).unwrap();
        asset_type_text.text.clear();

        if let Some(inspected_components) = &inspected_entity.shown_components {
            update_prefab_components_ent(
                ent.as_mut(),
                window_c.content_container,
                ent_selected_input,
                inspected_components,
                &foldout_res,
                assets.material_panel.clone(),
                assets.material_text.clone(),
                assets.font.clone(),
            );

            continue;
        }

        let res = simulated_world.data.as_ref().resources();

        let Some(asset_type) = get_asset_type(res, inspected_asset.asset_key.as_str()) else {
            destroy_entity_children(ent.as_mut(), window_c.content_container);
            continue;
        };

        asset_type_text.text.push_str("asset type: ");
        asset_type_text.text.push_str(asset_type.serialized_str());

        if is_entities_asset(asset_type) {
            destroy_entity_children(ent.as_mut(), window_c.content_container);
            continue;
        }

        let value = match (&inspected_asset.edited, res.get::<ReflectionRegistryResource>()) {
            (Some(edited), Some(registry)) => save_load_with_asset_serializers_from_world(res, EntityIdCtx::default(), |state| {
                build_inspector_value(registry, state, edited.0.as_any_ref())
            }),
            _ => None,
        }
        .unwrap_or_else(|| InspectorValue::read_only("not inspectable through reflection"));

        let ent_last = ent
            .get_component::<ParentComponent>(window_c.content_container)
            .map(|p| p.children().first())
            .flatten()
            .copied()
            .unwrap_or(EntityId::EMPTY);

        if let Some(parent_c) = ent.get_component::<ParentComponent>(window_c.content_container) {
            let extra = parent_c.children().iter().skip(1).copied().collect::<Vec<_>>();
            for e in extra {
                destroy_entity_and_children(ent.as_mut(), e);
            }
        }

        spawn_inspector_value(
            ent.as_mut(),
            ent_last,
            window_c.content_container,
            ent_selected_input,
            ReflectPath::new(),
            &value,
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
    components: &[(AnyTypeId, FfiString, InspectorValue)],
    foldout_res: &FoldoutResource,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) {
    if let Some(parent_c) = ent.get_component::<ParentComponent>(ent_parent) {
        let foreign = parent_c
            .children()
            .iter()
            .copied()
            .filter(|&e| ent.get_component::<InspectorComponentComponent>(e).is_none())
            .collect::<Vec<_>>();
        for e in foreign {
            destroy_entity_and_children(ent.as_mut(), e);
        }

        let extra = ent
            .get_component::<ParentComponent>(ent_parent)
            .unwrap()
            .children()
            .get(components.len() as usize..)
            .map(|extra| extra.to_vec())
            .unwrap_or_default();
        for e in extra.into_iter().rev() {
            destroy_entity_and_children(ent.as_mut(), e);
        }

        for (i, (component_type, component_label, component_value)) in components.iter().enumerate() {
            let ent_last = ent
                .get_component::<ParentComponent>(ent_parent)
                .unwrap()
                .children()
                .get(i)
                .copied()
                .unwrap_or(EntityId::EMPTY);

            update_prefab_component_ent(
                ent.as_mut(),
                ent_last,
                ent_parent,
                ent_selected_input,
                *component_type,
                component_label,
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
    component_type: AnyTypeId,
    component_label: &str,
    component_value: &InspectorValue,
    foldout_res: &FoldoutResource,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) {
    let key = FoldoutKey::InspectorComponent {
        component_id: component_label.to_string(),
    };
    let is_expanded = foldout_res.is_expanded(&key);

    if let Some(&serialized_component_c) = ent.get_component::<InspectorComponentComponent>(ent_last)
        && serialized_component_c.component_type == component_type
        && let Some(container_parent_c) = ent.get_component::<ParentComponent>(serialized_component_c.component_data_container)
        && let data_ents = container_parent_c.children().to_vec()
        && let Some(text_c) = ent.get_component_mut::<TextComponent>(serialized_component_c.component_id_text)
    {
        text_c.text = component_label.into();

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

        spawn_inspector_value(
            ent.as_mut(),
            data_ent,
            serialized_component_c.component_data_container,
            ent_selected_input,
            ReflectPath::new(),
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
        display_component_label(&component_label).into(),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );
    ent.set_components(
        component_header_ent,
        (ComponentHeaderComponent { component: comp_ent }, ButtonComponent::default()),
    );
    if let Some(image_c) = ent.get_component_mut::<ImageComponent>(component_header_ent) {
        image_c.color = PAL.surface_raised;
    }
    if let Some(text_c) = ent.get_component_mut::<TextComponent>(componet_id_text_ent) {
        text_c.color = PAL.text_1;
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

    ent.set_component(
        comp_ent,
        InspectorComponentComponent {
            component_type,
            component_id_text: componet_id_text_ent,
            component_data_container: comp_data_container,
            foldout_arrow,
        },
    );

    return_if!(!is_expanded);

    spawn_inspector_value(
        ent.as_mut(),
        EntityId::EMPTY,
        comp_data_container,
        ent_selected_input,
        ReflectPath::new(),
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

fn spawn_component_separator(mut ent: EntitiesHolderMut, ent_parent: EntityId, material_panel: &AssetHandle<Material>) {
    let ent_separator = ent.create_entity();

    ent.set_components(
        ent_separator,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::pw(1.0).into(), UiVal::px(1.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.border_subtle,
                ..Default::default()
            },
        ),
    );

    set_parent(ent.as_mut(), ent_separator, ent_parent);
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
