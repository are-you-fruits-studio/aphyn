use std::path::PathBuf;

use crate::{features::inspector_window::utils::reflection::EditorValue, *};

pub fn register_feature(mut data: WorldDataMut, _behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(EditorClipboardResource::default());
}

// in-editor only; text copy-paste and "Copy asset key" keep using the OS clipboard
#[derive(Resource, Default)]
pub struct EditorClipboardResource(pub Option<EditorClipboard>);

// component and field values are reflected clones; pasting clones them again
pub enum EditorClipboard {
    Files(Vec<PathBuf>),
    Entities(SerializedEntities),
    Component { component_id: FfiString, value: EditorValue },
    Value(EditorValue),
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

    pub fn component(&self) -> Option<(&FfiString, FfiAnyRef<'_>)> {
        match &self.0 {
            Some(EditorClipboard::Component { component_id, value }) => Some((component_id, value.0.as_any_ref())),
            _ => None,
        }
    }

    pub fn value(&self) -> Option<FfiAnyRef<'_>> {
        match &self.0 {
            Some(EditorClipboard::Value(value)) => Some(value.0.as_any_ref()),
            _ => None,
        }
    }
}
