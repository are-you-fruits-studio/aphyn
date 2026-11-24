use std::ffi::OsString;

use aphyn::*;

use crate::features::inspector_window::utils::reflection::{InspectorContainerKind, InspectorLeafKind};

#[derive(Component, Default, Debug)]
pub struct ProjectWindowComponent {
    pub is_init: bool,
}

#[derive(Component, Default, Debug)]
pub struct ProjectWindowEmptyAreaComponent;

#[derive(Component, Debug, Clone)]
pub struct ProjectWindowEntryComponent {
    pub path: OsString,
}

#[derive(Component, Default, Debug)]
pub struct HierarchyWindowComponent {
    pub is_init: bool,
}

#[derive(Component, Default, Copy, Clone)]
pub struct HierarchyWindowContentComponent {
    pub entries_container: EntityId,
}

#[derive(Component, Default, Debug)]
pub struct HierarchyWindowEmptyAreaComponent;

#[derive(Debug, Copy, Clone)]
pub enum InspectorValueComponentTy {
    Container {
        kind: InspectorContainerKind,
        is_inline: bool,
        container_enum_metadata: EntityId,
        container_fields: EntityId,
        container_buttons: EntityId,
    },
    Leaf {
        text: EntityId,
        kind: InspectorLeafKind,
    },
    Checkbox {
        button: EntityId,
        mark: EntityId,
        is_checked: bool,
    },
}

#[derive(Component, Clone)]
pub struct InspectorValueComponent {
    pub ty: InspectorValueComponentTy,
    pub path: ReflectPath,
}

#[derive(Component, Debug, Copy, Clone)]
pub struct InspectorFieldComponent {
    pub key_text: EntityId,
    pub value_container: EntityId,
}

#[derive(Component, Clone)]
pub struct InspectorEnumComponent {
    pub value_button: EntityId,
    pub value_text: EntityId,
    pub variants: FfiVec<FfiString>,
    pub dropdown_request_id: u64,
}
