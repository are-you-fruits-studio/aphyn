use aphyn_ecs::Event;
use aphyn_ffi::{FfiOption, FfiString};

use crate::KeyCode;

#[repr(C)]
#[derive(Event)]
pub struct TextInputEvent(pub FfiString);

#[repr(C)]
#[derive(Event)]
pub struct KeyPressedEvent {
    pub key: FfiOption<KeyCode>,
    // todo: maybe use FfiSmallString
    pub text: FfiString,
    pub is_repeat: bool,
}

#[repr(C)]
#[derive(Event, Copy, Clone, PartialEq, Eq, Debug)]
pub struct WindowFocusChangedEvent(pub bool);
