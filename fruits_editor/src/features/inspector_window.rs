use std::ffi::OsString;

use fruits_engine::*;

use crate::{
    features::{
        dropdown::select_dropdown_variant_system, input_field::{select_input_field_system, update_selected_input_field_text_system}, inspector_window::{
            data::{InspectedAssetResource, InspectedEntityResource}, systems::{
                add_component_system, adjust_hierarchy_entries_system, adjust_non_rigid_composite_system, apply_inspector_dropdown_change_to_simulated_world_system, apply_inspector_field_text_change_to_simulated_world_system, change_inspected_asset_system, destroy_non_inspected_entity_system, remove_component_system, save_inspected_asset_from_simulated_world_to_file_system, save_simulated_entities_to_prefab_system, select_entity_system, spawn_inspected_prefab_system, update_add_component_variants_system, update_hierarchy_entries_selection, update_hierarchy_window_system, update_inspector_window_system,
            },
        }, project_window_selection::select_file_system,
    }, *,
};

pub mod data;
pub mod systems;
pub mod utils;

// (+/2) todo: asset references as strings
// todo: support is_rigid=false (add element, remove element, edit key)

// todo: asset references as asset picker
// todo: improve scroll (scroll handle scale, mouse scroll, scroll handle auto-hide)
// todo: bool as a checkbox
// todo: float short form
// todo: better text editing
// todo: fancier visuals
// todo: better font
// todo: refactor
// todo: drag for numbers
// todo: optional slider for numbers
// todo: color picker
// todo: copy-paste
// todo: hint for int-float differentiation
// todo: asset drag-and-drop for referencing

pub const FONT_SIZE: UiVal = UiVal::px(16.0);
pub const FIELD_HEIGHT: UiVal = UiVal::px(18.0);
pub const INDENT_WIDTH: UiVal = UiVal::px(18.0);
pub const FIELD_GAP: UiVal = UiVal::px(2.0);

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(InspectedAssetResource {
        asset_key: String::new(),
        path: OsString::new(),
        spawned_prefab: EntityId::EMPTY,
    });
    world.data_mut().resources_mut().insert(InspectedEntityResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(update_inspector_window_system)
        .insert_child_system(apply_inspector_field_text_change_to_simulated_world_system)
        .insert_child_system(apply_inspector_dropdown_change_to_simulated_world_system)
        .insert_child_system(save_simulated_entities_to_prefab_system)
        .insert_child_system(select_entity_system)
        .insert_child_system(update_hierarchy_entries_selection)
        .insert_child_system(adjust_non_rigid_composite_system)
        .insert_child_system(remove_component_system)
        .insert_child_system(add_component_system)
        .insert_child_system(update_add_component_variants_system)
        .insert_child_system(adjust_hierarchy_entries_system)
        .insert_child_system(update_hierarchy_window_system)
        .insert_child_system(change_inspected_asset_system)
        .insert_child_system(save_inspected_asset_from_simulated_world_to_file_system)
        .insert_child_system(destroy_non_inspected_entity_system)
        .insert_child_system(spawn_inspected_prefab_system);

    update
        .order_system(select_file_system)
        .before_system(adjust_non_rigid_composite_system)
        .before_system(remove_component_system)
        .before_system(add_component_system)
        .before_system(update_add_component_variants_system)
        .before_system(select_dropdown_variant_system)
        .before_system(adjust_hierarchy_entries_system)
        .before_system(apply_inspector_field_text_change_to_simulated_world_system)
        .before_system(save_simulated_entities_to_prefab_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system)
        .before_system(select_entity_system)
        .before_system(destroy_non_inspected_entity_system)
        .before_system(spawn_inspected_prefab_system)
        .before_system(update_hierarchy_window_system)
        .before_system(update_hierarchy_entries_selection)
        .before_system(change_inspected_asset_system)
        .before_system(update_inspector_window_system);

    update.order_system(check_ui_interaction_system)
        .before_system(select_entity_system);

    update
        .order_system(select_input_field_system)
        .before_system(update_inspector_window_system);

    update
        .order_system(update_selected_input_field_text_system)
        .before_system(apply_inspector_field_text_change_to_simulated_world_system);

    update
        .order_system(select_dropdown_variant_system)
        .before_system(apply_inspector_dropdown_change_to_simulated_world_system);

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

    //
}
