//! # aphyn_reflection_ecs
//!
//! The bridge between `aphyn_reflection` and the ECS: [`ReflectionRegistryResource`], the world's
//! read-only reflection registry. It lives in its own
//! crate so that `aphyn_reflection` stays independent of the ECS.
//!
//! # How to use
//!
//! When the app starts running it builds the resource from the `ReflectionRegistryBuilder` of its
//! init context (`aphyn_modules::add_default_modules_to` fills that builder with the engine's
//! reflectors). Systems and render features read it and clone out the reflectors or fields they keep:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn find_position_field(res: &ReflectionRegistryResource, types: &TypesRegistryCache) -> Option<FfiArc<ReflectField>> {
//!     let transform = res.get_of::<LocalTransform>(types)?.as_struct()?;
//!     transform.field("position").cloned()
//! }
//! ```
//!
//! #### Registering your own types
//!
//! Game setup code registers its reflectors in the init context's builder (the editor's inspector
//! shows only components it finds in the registry):
//!
//! ```ignore
//! use aphyn::*;
//!
//! #[derive(Component, Default, Clone, Reflectable)]
//! struct Health(u32);
//!
//! fn setup(ctx: AppInitCtx) {
//!     ctx.reflection.register::<Health>(ctx.types);
//! }
//! ```
//!
//! # How to maintain
//!
//! [`ReflectionRegistryResource`] is a `#[repr(C)]` newtype over a `ReflectionRegistry` that derefs
//! to it. The registry is immutable once built, so the resource has no `DerefMut`, and nothing
//! extends it after init.

use std::ops::Deref;

use aphyn_ecs::Resource;
use aphyn_reflection::ReflectionRegistry;

#[repr(C)]
#[derive(Default, Clone, Debug)]
pub struct ReflectionRegistryResource(pub ReflectionRegistry);

impl Resource for ReflectionRegistryResource {}

impl Deref for ReflectionRegistryResource {
    type Target = ReflectionRegistry;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
