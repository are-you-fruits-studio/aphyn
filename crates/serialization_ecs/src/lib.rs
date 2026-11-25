//! # aphyn_serialization_ecs
//!
//! The bridge between `aphyn_serialization` and the ECS: [`SerializersResource`], the world's
//! shared, read-only runtime serializer registry. It lives in its own crate so that
//! `aphyn_serialization` stays independent of the ECS (`aphyn_ecs` depends on it, not the other
//! way around).
//!
//! # How to use
//!
//! Serializers are registered during app init in the `TransSerializerRegistryBuilder` of the init
//! context; `aphyn_modules::add_default_modules_to` pre-fills it with the engine's common
//! serializers. Register your own types there before the app runs:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn setup(ctx: AppInitCtx) {
//!     ctx.serializers.register(ctx.types, "my_game::Health", StandardSerializer::<Health>::default());
//! }
//! ```
//!
//! When the app starts running it builds the registry and inserts it as [`SerializersResource`],
//! which systems and render features read:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn save_health(res: Res<SerializersResource>, types: &TypesRegistryCache, health: &Health) -> SerializedValue {
//!     res.serialize(types, health, "", &mut |err| println!("{err}"))
//! }
//! ```
//!
//! # How to maintain
//!
//! [`SerializersResource`] is a `#[repr(C)]` newtype over a `TransSerializerRegistry` that derefs
//! to it, so every registry method is callable on the resource directly. The registry is
//! immutable once built, so there is no `DerefMut`. The resource holds no logic of its own;
//! consumers (`aphyn_asset_loading`, `aphyn_prefab` users, the editor) fetch it from the world and
//! pair it with a per-operation `ContextData` to build a context.

use std::ops::Deref;

use aphyn_ecs::Resource;
use aphyn_serialization::TransSerializerRegistry;

#[repr(C)]
#[derive(Default, Clone)]
pub struct SerializersResource(pub TransSerializerRegistry);

impl Resource for SerializersResource {}

impl Deref for SerializersResource {
    type Target = TransSerializerRegistry;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
