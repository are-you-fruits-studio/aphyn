use std::ops::{Deref, DerefMut};

use fruits_ecs::Resource;

use crate::TransSerializerRegistry;

#[repr(C)]
#[derive(Default)]
pub struct SerializersResource(pub TransSerializerRegistry<'static>);

impl Resource for SerializersResource { }

impl Deref for SerializersResource {
    type Target = TransSerializerRegistry<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SerializersResource {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}