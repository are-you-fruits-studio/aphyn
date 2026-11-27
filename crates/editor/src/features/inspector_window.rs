use aphyn::*;

use crate::{
    features::{
        generic_dropdown::select_generic_dropdown_system,
        input_field::{apply_input_field_context_menu_system, select_input_field_system, update_selected_input_field_text_system},
        inspector_window::{
            data::{
                ComponentContextMenuResource, FieldValueContextMenuResource, HierarchyContextMenuResource, InspectedAssetResource,
                InspectedEntityResource, OpenedEntitiesAssetResource,
            },
            systems::{
                add_component_system, adjust_non_rigid_composite_system, apply_component_context_menu_system,
                apply_field_value_context_menu_system, apply_hierarchy_context_menu_system,
                apply_inspector_dropdown_change_to_simulated_world_system,
                apply_inspector_field_text_change_to_simulated_world_system, apply_inspector_number_drag_to_simulated_world_system,
                change_inspected_asset_system, copy_selected_entity_hotkey_system, delete_selected_entity_hotkey_system,
                open_component_context_menu_system, open_entities_asset_system, open_field_value_context_menu_system,
                open_hierarchy_context_menu_system, open_inspector_enum_dropdown_system, paste_entities_hotkey_system,
                save_inspected_asset_from_simulated_world_to_file_system, save_simulated_entities_to_asset_system,
                select_entity_system, toggle_inspector_checkbox_system, update_add_component_input_visibility_system,
                update_add_component_variants_system, update_hierarchy_entries_selection, update_hierarchy_window_system,
                update_inspector_window_system,
            },
        },
        number_drag::drag_number_input_field_system,
        project_window_selection::{
            apply_project_entry_context_menu_system, select_file_system, update_project_entry_selection_system,
        },
        world_preload::tick_simulated_world_system,
    },
    *,
};

pub mod data;
pub mod systems;
pub mod utils;

// (+/2) todo: asset references as strings
// todo: support is_rigid=false (add element, remove element, edit key)

// todo: asset references as asset picker
// todo: improve scroll (scroll handle auto-hide)
// todo: float short form
// todo: better text editing
// todo: fancier visuals
// todo: better font
// todo: refactor
// todo: optional slider for numbers
// todo: color picker
// todo: copy-paste
// todo: hint for int-float differentiation
// todo: asset drag-and-drop for referencing

pub const FONT_SIZE: UiVal = UiVal::px(crate::theme::FONT);
pub const FIELD_HEIGHT: UiVal = UiVal::px(crate::theme::CONTROL_H);
pub const INDENT_WIDTH: UiVal = UiVal::px(crate::theme::INDENT);
pub const FIELD_GAP: UiVal = UiVal::px(2.0);

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(InspectedAssetResource::default());
    data.resources_mut().insert(OpenedEntitiesAssetResource::default());
    data.resources_mut().insert(InspectedEntityResource::default());
    data.resources_mut().insert(ComponentContextMenuResource::default());
    data.resources_mut().insert(HierarchyContextMenuResource::default());
    data.resources_mut().insert(FieldValueContextMenuResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(update_inspector_window_system)
        .insert_child_system(apply_inspector_field_text_change_to_simulated_world_system)
        .insert_child_system(apply_inspector_number_drag_to_simulated_world_system)
        .insert_child_system(apply_inspector_dropdown_change_to_simulated_world_system)
        .insert_child_system(open_inspector_enum_dropdown_system)
        .insert_child_system(open_component_context_menu_system)
        .insert_child_system(apply_component_context_menu_system)
        .insert_child_system(save_simulated_entities_to_asset_system)
        .insert_child_system(select_entity_system)
        .insert_child_system(update_hierarchy_entries_selection)
        .insert_child_system(adjust_non_rigid_composite_system)
        .insert_child_system(toggle_inspector_checkbox_system)
        .insert_child_system(add_component_system)
        .insert_child_system(update_add_component_variants_system)
        .insert_child_system(update_add_component_input_visibility_system)
        .insert_child_system(open_hierarchy_context_menu_system)
        .insert_child_system(apply_hierarchy_context_menu_system)
        .insert_child_system(delete_selected_entity_hotkey_system)
        .insert_child_system(copy_selected_entity_hotkey_system)
        .insert_child_system(paste_entities_hotkey_system)
        .insert_child_system(open_field_value_context_menu_system)
        .insert_child_system(apply_field_value_context_menu_system)
        .insert_child_system(update_hierarchy_window_system)
        .insert_child_system(change_inspected_asset_system)
        .insert_child_system(save_inspected_asset_from_simulated_world_to_file_system)
        .insert_child_system(open_entities_asset_system);

    update
        .order_system(select_file_system)
        .before_system(adjust_non_rigid_composite_system)
        .before_system(add_component_system)
        .before_system(update_add_component_variants_system)
        .before_system(apply_hierarchy_context_menu_system)
        .before_system(apply_inspector_field_text_change_to_simulated_world_system)
        .before_system(save_simulated_entities_to_asset_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system)
        .before_system(select_entity_system)
        .before_system(open_entities_asset_system)
        .before_system(update_hierarchy_window_system)
        .before_system(update_hierarchy_entries_selection)
        .before_system(change_inspected_asset_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(select_entity_system);

    update
        .order_system(select_entity_system)
        .before_system(update_project_entry_selection_system);

    update
        .order_system(apply_project_entry_context_menu_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(open_entities_asset_system)
        .before_system(update_add_component_input_visibility_system);

    update
        .order_system(update_add_component_input_visibility_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(select_input_field_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(tick_simulated_world_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(update_selected_input_field_text_system)
        .before_system(apply_inspector_field_text_change_to_simulated_world_system);

    update
        .order_system(select_file_system)
        .before_system(apply_inspector_number_drag_to_simulated_world_system);

    update
        .order_system(drag_number_input_field_system)
        .before_system(apply_inspector_number_drag_to_simulated_world_system);

    update
        .order_system(drag_number_input_field_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(apply_inspector_number_drag_to_simulated_world_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(apply_inspector_number_drag_to_simulated_world_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(apply_inspector_dropdown_change_to_simulated_world_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(open_inspector_enum_dropdown_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(open_component_context_menu_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(apply_component_context_menu_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(apply_component_context_menu_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(open_hierarchy_context_menu_system);

    update
        .order_system(open_hierarchy_context_menu_system)
        .before_system(update_hierarchy_window_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(apply_hierarchy_context_menu_system);

    update
        .order_system(delete_selected_entity_hotkey_system)
        .before_system(save_simulated_entities_to_asset_system);

    // a field deselected by Enter/Escape this frame still swallows the key
    update
        .order_system(delete_selected_entity_hotkey_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(paste_entities_hotkey_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(paste_entities_hotkey_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(copy_selected_entity_hotkey_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(open_field_value_context_menu_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(apply_field_value_context_menu_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(apply_field_value_context_menu_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(apply_inspector_field_text_change_to_simulated_world_system);

    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(update_add_component_variants_system);

    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(apply_inspector_dropdown_change_to_simulated_world_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(adjust_non_rigid_composite_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(select_input_field_system)
        .before_system(update_add_component_variants_system);

    update
        .order_system(update_selected_input_field_text_system)
        .before_system(update_add_component_variants_system);

    // todo
    // update
    //     .order_system(parse_selected_file_system)
    //     .before_system(update_inspector_window_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(adjust_non_rigid_composite_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(toggle_inspector_checkbox_system);

    update
        .order_system(select_file_system)
        .before_system(toggle_inspector_checkbox_system);

    update
        .order_system(toggle_inspector_checkbox_system)
        .before_system(save_simulated_entities_to_asset_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system)
        .before_system(update_inspector_window_system);

    //
}
