use std::path::PathBuf;

use crate::{features::inspector_window::utils::reflection::EditorValue, *};

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx { mut data, .. } = ctx;
    data.resources_mut().insert(EditorClipboardResource::default());
}

// in-editor only; text copy-paste and "Copy asset key" keep using the OS clipboard
#[derive(Resource, Default)]
pub struct EditorClipboardResource(pub Option<EditorClipboard>);

// component and field values are reflected clones; pasting clones them again
pub enum EditorClipboard {
    Files(Vec<PathBuf>),
    Entities(SerializedEntities),
    // the component type is the value's; it is resolved against the current simulated world when pasting
    Component(EditorValue),
    Value(EditorValue),
    AnimationKeys(Vec<CopiedAnimationKey>),
}

// `time` is relative to the earliest copied key
pub struct CopiedAnimationKey {
    pub path: AnimationPath,
    pub value_type: AnyTypeId,
    pub time: f32,
    pub value: EditorValue,
    pub interpolation: KeyInterpolation,
    pub in_tangent: f32,
    pub out_tangent: f32,
    pub auto_tangent: bool,
}

impl EditorClipboardResource {
    pub fn animation_keys(&self) -> Option<&[CopiedAnimationKey]> {
        match &self.0 {
            Some(EditorClipboard::AnimationKeys(keys)) => Some(keys),
            _ => None,
        }
    }

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

    pub fn component(&self) -> Option<AnyRef<'_>> {
        match &self.0 {
            Some(EditorClipboard::Component(value)) => Some(value.0.as_any_ref()),
            _ => None,
        }
    }

    pub fn value(&self) -> Option<AnyRef<'_>> {
        match &self.0 {
            Some(EditorClipboard::Value(value)) => Some(value.0.as_any_ref()),
            _ => None,
        }
    }
}
