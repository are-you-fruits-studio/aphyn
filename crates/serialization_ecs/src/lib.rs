//! # aphyn_serialization_ecs
//!
//! The bridge between `aphyn_serialization` and the ECS: [`SerializersResource`], the world's
//! shared runtime serializer registry. It lives in its own crate so that `aphyn_serialization`
//! stays independent of the ECS (`aphyn_ecs` depends on it, not the other way around).
//!
//! # How to use
//!
//! `aphyn_modules::add_defult_modules_to` inserts the resource, pre-filled with the engine's
//! common serializers. Register your own type through it before (de)serializing world data that
//! contains it:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn register_serializers(mut data: WorldDataMut) {
//!     let mut res = data.resources_mut();
//!     let serializers = res.get_mut::<SerializersResource>().unwrap();
//!
//!     serializers.register(StandardSerializer::<Health>::default());
//! }
//! ```
//!
//! # How to maintain
//!
//! [`SerializersResource`] is a `#[repr(C)]` newtype over a `'static` `TransSerializerRegistry`
//! that derefs to it, so every registry method is callable on the resource directly. It holds no
//! logic of its own; consumers (`aphyn_asset_loading`, `aphyn_prefab` users, the editor) fetch it
//! from the world and wrap it with their local registries.

use std::ops::{Deref, DerefMut};

use aphyn_ecs::Resource;
use aphyn_serialization::TransSerializerRegistry;

#[repr(C)]
#[derive(Default)]
pub struct SerializersResource(pub TransSerializerRegistry<'static>);

impl Resource for SerializersResource {}

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
