//! # aphyn_reflection_ecs
//!
//! The bridge between `aphyn_reflection` and the ECS: [`ReflectionRegistryResource`], the world's
//! read-only reflection registry. It lives in its own
//! crate so that `aphyn_reflection` stays independent of the ECS.
//!
//! # How to use
//!
//! `aphyn_modules::add_defult_modules_to` inserts the resource, built from the engine's
//! reflectors. Systems read it and clone out the reflectors or fields they keep:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn find_position_field(res: &ReflectionRegistryResource) -> Option<FfiArc<ReflectField>> {
//!     let transform = res.get_of::<LocalTransform>()?.as_struct()?;
//!     transform.field("position").cloned()
//! }
//! ```
//!
//! # How to maintain
//!
//! [`ReflectionRegistryResource`] is a `#[repr(C)]` newtype over a `ReflectionRegistry` that derefs
//! to it. The registry is immutable once built, so the resource has no `DerefMut`; registering more
//! types means building a new registry and replacing the resource.

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
