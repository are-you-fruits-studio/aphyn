use std::ffi::OsString;

use crate::{
    features::inspector_window::utils::reflection::{EditorValue, InspectorValue},
    *,
};

#[derive(Resource, Clone)]
pub struct OpenProjectResource {
    pub dir_path: String,
}

// `edited` is the reflected copy the inspector edits; the asset is rebuilt from it after every edit
#[derive(Resource, Default)]
pub struct InspectedAssetResource {
    pub asset_key: String,
    pub path: OsString,
    pub edited: Option<EditorValue>,
}

#[derive(Resource, Default)]
pub struct OpenedEntitiesAssetResource {
    pub asset_key: String,
    pub path: OsString,
    pub asset_type: Option<AssetType>,
    pub prefab_root: EntityId,
    // set while the Animation window previews, so animated values aren't recorded into the asset
    pub is_saving_paused: bool,
}

// todo: maybe merge InspectedAssetResource and InspectedEntityResource or smth.

#[derive(Resource, Default)]
pub struct InspectedEntityResource {
    pub selected_entity: EntityId,
    pub shown_components: Option<Vec<(AnyTypeId, FfiString, InspectorValue)>>,
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

#[derive(Clone, Copy)]
pub enum ComponentContextAction {
    Copy,
    PasteValues,
    PasteAsNew,
    Remove,
}

impl ComponentContextAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy component",
            Self::PasteValues => "Paste component values",
            Self::PasteAsNew => "Paste component as new",
            Self::Remove => "Remove component",
        }
    }
}

#[derive(Resource, Default)]
pub struct ComponentContextMenuResource {
    pub request_id: u64,
    pub component_id: AnyTypeId,
    pub actions: Vec<ComponentContextAction>,
}

#[derive(Clone, Copy)]
pub enum FieldValueContextAction {
    Copy,
    Paste,
}

impl FieldValueContextAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy value",
            Self::Paste => "Paste value",
        }
    }
}

#[derive(Resource, Default)]
pub struct FieldValueContextMenuResource {
    pub request_id: u64,
    pub value_ent: EntityId,
    pub actions: Vec<FieldValueContextAction>,
}

#[derive(Clone, Copy)]
pub enum HierarchyContextAction {
    CreateEntity,
    CopyEntity,
    PasteEntity,
    PasteComponent,
    DeleteEntity,
}

impl HierarchyContextAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::CreateEntity => "Create entity",
            Self::CopyEntity => "Copy entity",
            Self::PasteEntity => "Paste entity",
            Self::PasteComponent => "Paste component",
            Self::DeleteEntity => "Delete entity",
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
pub struct InspectorCompositeAddButton {
    pub composite: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct InspectorCompositeRemoveButton {
    pub composite: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct InspectorComponentComponent {
    pub component_type: AnyTypeId,
    pub component_id_text: EntityId,
    pub component_data_container: EntityId,
    pub foldout_arrow: EntityId,
}

#[derive(Component, Default, Copy, Clone)]
pub struct AddComponentInputComponent {
    pub variants_container: EntityId,
    pub placeholder: EntityId,
}

#[derive(Component, Default, Clone)]
pub struct AddComponentVariantComponent {
    pub component_id: AnyTypeId,
}

#[derive(Event, Copy, Clone, PartialEq, Eq)]
pub enum AssetEditedEvent {
    InspectedAsset,
    OpenedEntitiesAsset,
}

#[derive(Event)]
pub struct EntitySelectedEvent;

// a field of a simulated entity's component changed through an editor tool (inspector, gizmo)
#[derive(Event, Clone)]
pub struct EntityFieldEditedEvent {
    pub entity: EntityId,
    pub component: AnyTypeId,
    pub path: ReflectPath,
}
