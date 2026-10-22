
use std::{collections::HashMap, ffi::OsString};

use crate::*;

#[derive(Resource, Clone)]
pub struct OpenProjectResource {
    pub dir_path: String,
}

#[derive(Resource, Default)]
pub struct InspectedAssetResource {
    pub asset_key: String,
    pub path: OsString,
}

#[derive(Resource, Default)]
pub struct OpenedEntitiesAssetResource {
    pub asset_key: String,
    pub path: OsString,
    pub asset_type: Option<AssetType>,
    pub prefab_root: EntityId,
}

// todo: maybe merge InspectedAssetResource and InspectedEntityResource or smth.

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
pub struct ComponentHeaderComponent {
    pub component: EntityId,
}

#[derive(Resource, Default)]
pub struct ComponentContextMenuResource {
    pub request_id: u64,
    pub component_id: FfiString,
}

#[derive(Clone, Copy)]
pub enum HierarchyContextAction {
    CreateEntity,
    DeleteEntity,
}

impl HierarchyContextAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::CreateEntity => "Create Entity",
            Self::DeleteEntity => "Delete Entity",
        }
    }
}

#[derive(Resource, Default)]
pub struct HierarchyContextMenuResource {
    pub request_id: u64,
    pub target: EntityId,
    pub actions: Vec<HierarchyContextAction>,
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

#[derive(Event, Copy, Clone, PartialEq, Eq)]
pub enum AssetEditedEvent {
    InspectedAsset,
    OpenedEntitiesAsset,
}

#[derive(Event)]
pub struct EntitySelectedEvent;
