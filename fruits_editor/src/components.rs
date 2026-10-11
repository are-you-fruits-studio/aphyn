use std::ffi::OsString;

use fruits_engine::*;

#[derive(Component, Default, Debug)]
pub struct ProjectWindowComponent {
    pub is_init: bool,
}

#[derive(Component, Debug, Clone)]
pub struct ProjectWindowEntryComponent {
    pub path: OsString,
}

#[derive(Component, Default, Debug)]
pub struct HierarchyWindowComponent {
    pub is_init: bool,
}

#[derive(Component)]
pub struct HierarchyButtonAddComponent;

#[derive(Component)]
pub struct HierarchyButtonRemoveComponent;

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
    pub value_text: EntityId,
    pub variants: FfiVec<FfiString>,
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