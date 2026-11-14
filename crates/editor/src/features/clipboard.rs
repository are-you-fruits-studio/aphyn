use std::path::PathBuf;

use crate::*;

pub fn register_feature(mut data: WorldDataMut, _behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(EditorClipboardResource::default());
}

// in-editor only; text copy-paste and "Copy asset key" keep using the OS clipboard
#[derive(Resource, Default)]
pub struct EditorClipboardResource(pub Option<EditorClipboard>);

pub enum EditorClipboard {
    Files(Vec<PathBuf>),
    Entities(SerializedEntities),
    Component {
        component_id: FfiString,
        value: SerializedValue,
    },
    Value(SerializedValue),
}

impl EditorClipboardResource {
    pub fn files(&self) -> Option<&[PathBuf]> {
        match &self.0 {
            Some(EditorClipboard::Files(paths)) => Some(paths),
            _ => None,
        }
    }

    pub fn entities(&self) -> Option<&SerializedEntities> {
        match &self.0 {
            Some(EditorClipboard::Entities(entities)) => Some(entities),
            _ => None,
        }
    }

    pub fn component(&self) -> Option<(&FfiString, &SerializedValue)> {
        match &self.0 {
            Some(EditorClipboard::Component { component_id, value }) => Some((component_id, value)),
            _ => None,
        }
    }

    pub fn value(&self) -> Option<&SerializedValue> {
        match &self.0 {
            Some(EditorClipboard::Value(value)) => Some(value),
            _ => None,
        }
    }
}
