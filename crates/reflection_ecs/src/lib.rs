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
//! #### Registering your own types
//!
//! Game setup code adds its reflectors with [`extend_reflection_registry`] after the default
//! modules are added (the editor's inspector shows only components it finds there):
//!
//! ```ignore
//! use aphyn::*;
//!
//! #[derive(Component, Default, Clone, Reflectable)]
//! struct Health(u32);
//!
//! fn setup(ctx: AppInitCtx) {
//!     extend_reflection_registry(ctx.data.resources_mut(), |reflectors| {
//!         reflectors.register::<Health>();
//!     });
//! }
//! ```
//!
//! # How to maintain
//!
//! [`ReflectionRegistryResource`] is a `#[repr(C)]` newtype over a `ReflectionRegistry` that derefs
//! to it. The registry is immutable once built, so the resource has no `DerefMut`;
//! [`extend_reflection_registry`] copies the reflectors into a builder (`ReflectionRegistry::to_builder`),
//! registers more types and replaces the resource with the rebuilt registry. Fields and reflectors
//! cloned out of the old registry stay valid, since they are shared `FfiArc`s.

use std::ops::Deref;

use aphyn_ecs::{Resource, ResourcesHolderMut};
use aphyn_reflection::{ReflectionRegistry, ReflectionRegistryBuilder};

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

// rebuilds the world's registry (an empty one when missing) with more types and replaces the resource
pub fn extend_reflection_registry(mut res: ResourcesHolderMut, register: impl FnOnce(&mut ReflectionRegistryBuilder)) {
    let mut builder = res
        .get::<ReflectionRegistryResource>()
        .map(|r| r.to_builder())
        .unwrap_or_default();

    register(&mut builder);

    res.insert(ReflectionRegistryResource(builder.build()));
}
