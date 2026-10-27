use aphyn_ecs::Event;
use aphyn_ffi::FfiString;

#[repr(C)]
#[derive(Event)]
pub struct TextInputEvent(pub FfiString);