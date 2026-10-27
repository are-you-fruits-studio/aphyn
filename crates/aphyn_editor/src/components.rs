use std::ffi::OsString;

use aphyn::*;

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
pub enum SerializedValueComponentTy {
    Container { ty: SerializedValueContainerType, container_enum_metadata: EntityId, container_fields: EntityId, container_buttons: EntityId },
    Primitive { text: EntityId, ty: SerializedValuePrimitiveType },
}

#[derive(Component, Clone)]
pub struct SerializedValueComponent {
    pub ty: SerializedValueComponentTy,
    pub path: FfiString,
}

#[derive(Debug, Copy, Clone)]
pub struct SerializedFieldComponent {
    pub key_text: EntityId,
    pub value_container: EntityId
}

#[derive(Component, Clone)]
pub struct SerializedEnumMetadataComponent {
    pub value_button: EntityId,
    pub value_text: EntityId,
    pub variants: FfiVec<FfiString>,
    pub dropdown_request_id: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SerializedValueContainerType {
    List,
    Map,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SerializedValuePrimitiveType {
    Null,
    Bool,
    Int,
    Float,
    String,
}