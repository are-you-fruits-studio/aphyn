use fruits_ecs::Event;
use fruits_ffi::FfiString;

#[repr(C)]
#[derive(Event)]
pub struct TextInputEvent(pub FfiString);