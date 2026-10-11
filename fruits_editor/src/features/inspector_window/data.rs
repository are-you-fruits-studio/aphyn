
use std::{collections::HashMap, ffi::OsString};

use crate::*;

#[derive(Resource, Clone)]
pub struct OpenProjectResource {
    pub dir_path: String,
}

#[derive(Resource)]
pub struct InspectedAssetResource {
    pub asset_key: String,
    pub path: OsString,
    pub spawned_prefab: EntityId,
}

#[derive(Resource, Default)]
pub struct InspectedEntityResource {
    pub selected_entity: EntityId,
    pub ent_to_id: HashMap<EntityId, u64>,
    pub id_to_ent: HashMap<u64, EntityId>,
}

#[derive(Component, Default, Copy, Clone)]
pub struct HierarchyWindowEntryComponent {
    pub simulated_entity: EntityId,
}

#[derive(Component, Default, Debug)]
pub struct InspectorWindowComponent {
    pub is_init: bool,
}

#[derive(Component, Default, Copy, Clone)]
pub struct InspectorWindowContentComponent {
    pub asset_type_text: EntityId,
    pub content_container: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct ComponentRemoveButton {
    pub component: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct SerializedCompositeAddButton {
    pub composite: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct SerializedCompositeRemoveButton {
    pub composite: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct SerializedComponentComponent {
    pub component_id_text: EntityId,
    pub component_data_container: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct AddComponentInputComponent {
    pub variants_container: EntityId,
}

#[derive(Component, Default, Clone)]
pub struct AddComponentVariantComponent {
    pub component_id: FfiString,
}

#[derive(Event)]
pub struct InspectedAssetEditedEvent;

#[derive(Event)]
pub struct PrefabEntitySelectedEvent;
