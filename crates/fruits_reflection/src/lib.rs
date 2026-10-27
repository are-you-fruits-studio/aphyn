//! # fruits_reflection
//!
//! Runtime reflection for engine types. It lets code inspect a type's
//! structure, build and take apart values dynamically, and convert values to a
//! self-describing data tree without knowing the concrete type at compile time.
//! This is the groundwork for tooling — editors, inspectors, serializers — that
//! must operate over arbitrary user types.
//!
//! # How to use
//!
//! The crate exposes three independent facilities. Pick the one that matches the
//! task: a self-describing value tree ([`ReflRepr`]), dynamic construction and
//! field access ([`ReflMapStruct`]), or a static structural schema ([`ReflTy`]).
//!
//! #### Convert a value to a self-describing representation
//!
//! Turn a typed value into a [`ReflRepr`] tree that mirrors its fields. Register
//! the built-in representers for primitives, `Option`, and `Vec`, then add a
//! [`ReflRepresenter`] for your own type and call
//! [`into_repr`](ReflRepresenterRegistry::into_repr).
//!
//! ```
//! use fruits_reflection::*;
//!
//! struct Player {
//!     health: u32,
//!     name: String,
//! }
//!
//! struct PlayerRepresenter;
//! impl ReflRepresenter for PlayerRepresenter {
//!     type Item = Player;
//!
//!     fn item_name(&self, _ctx: &ReflRepresenterCtx) -> String {
//!         String::from("Player")
//!     }
//!
//!     fn into_repr(&self, ctx: &ReflRepresenterCtx, v: &Self::Item) -> Option<ReflRepr> {
//!         Some(ReflRepr::Struct(ReflReprStruct {
//!             name: self.item_name(ctx),
//!             fields: ReflReprFields::Named(vec![
//!                 (String::from("health"), ctx.into_repr(&v.health)?),
//!                 (String::from("name"), ctx.into_repr(&v.name)?),
//!             ]),
//!         }))
//!     }
//! }
//!
//! let mut reg = ReflRepresenterRegistry::default();
//! set_common_representers(&mut reg);
//! reg.set(Box::new(PlayerRepresenter));
//!
//! let player = Player { health: 100, name: String::from("Barry") };
//! let repr = reg.into_repr(&player).unwrap();
//!
//! assert!(matches!(repr, ReflRepr::Struct(_)));
//! ```
//!
//! #### Construct and destructure a value dynamically
//!
//! Build a value from type-erased parameters and read its fields back out
//! without naming the concrete type, using [`ReflMapStruct`] and
//! [`ReflMapStructField`].
//!
//! ```
//! use fruits_reflection::*;
//! use std::collections::HashMap;
//!
//! struct Player {
//!     health: u32,
//!     name: String,
//! }
//!
//! let map = ReflMapStruct::new(
//!     |(health, name)| Player { health, name },
//!     |v| (v.health, v.name),
//!     HashMap::from([
//!         ("health", ReflMapStructField::new(
//!             |p: &Player| &p.health,
//!             |p: &mut Player| &mut p.health,
//!         )),
//!         ("name", ReflMapStructField::new(
//!             |p: &Player| &p.name,
//!             |p: &mut Player| &mut p.name,
//!         )),
//!     ]),
//! );
//!
//! let value = map.create(vec![Box::new(100_u32), Box::new(String::from("Barry"))]).unwrap();
//! let health = map.fields["health"].get_ref(&*value).unwrap().downcast_ref::<u32>().unwrap();
//!
//! assert_eq!(*health, 100);
//! ```
//!
//! #### Derive a type's structural schema
//!
//! Generate a [`ReflTy`] describing a type's fields and their type ids with the
//! [`ReflectTy`](derive@ReflectTy) derive. The schema carries no values — only
//! the shape.
//!
//! ```
//! use fruits_reflection::*;
//!
//! #[derive(ReflectTy)]
//! struct Player {
//!     health: u32,
//!     name: String,
//! }
//!
//! let ty = Player::refl_ty();
//!
//! assert!(matches!(ty, ReflTy::Struct(ReflTyStruct::Named(_))));
//! ```
//!
//! # How to maintain
//!
//! The crate is early and experimental — the source is dotted with `// todo`
//! markers for enums, collections, and generics, and several `example` /
//! `*_use_case` functions (in `refl_ty.rs`, `registry.rs`, and the
//! `fruits_reflection_example` binary) are scratch demonstrations rather than
//! API. It currently offers three parallel, independent reflection facilities;
//! the private `Reflect` trait in this file is an unfinished attempt to unify
//! them and is not used yet.
//!
//! #### Structural schema: `ReflTy`
//!
//! [`ReflTy`] is a value-free description of a type's shape (struct, enum,
//! array, slice). Types are identified by [`ReflTyId`], whose identity is
//! currently the [`std::any::type_name`] string — a `// todo` flags this as not
//! collision-proof or guaranteed stable across compilations. [`ReflTyRegistry`]
//! maps ids to schemas. The [`ReflectTy`](derive@ReflectTy) derive macro (in the
//! sibling `fruits_reflection_macros` crate) builds the `refl_ty()` body by
//! emitting source text as a string and parsing it; it panics on lifetimes and
//! unions, and references `ReflTy`/`ReflTyStruct`/`ReflTyId` unqualified, so the
//! derive only works where those names are in scope.
//!
//! #### Dynamic construction: `ReflMapStruct`
//!
//! [`ReflMapStruct`] stores the user's `fn(P) -> T` factory and `fn(T) -> P`
//! deconstructor as type-erased `*const ()` pointers, alongside monomorphized
//! wrapper functions that capture the concrete `T` and `P` at construction and
//! transmute the raw pointer back to the real signature before calling it. The
//! transmute is sound only because each raw pointer is ever invoked through its
//! own matching wrapper. [`TupleFromVecOfAny`] bridges `Vec<Box<dyn Any>>` and
//! tuples — a macro implements it for arities 0 through 16 — which is what lets
//! the factory take a variadic parameter list. [`create`](ReflMapStruct::create)
//! and [`deconstruct`](ReflMapStruct::deconstruct) return `None` on an
//! arity or type mismatch, surfaced as a failed `try_into`/`downcast`. Field
//! access works the same way through [`ReflMapStructField`]. Enums and
//! collections are not yet supported.
//!
//! #### Value tree: `ReflRepr` and representers
//!
//! [`ReflRepr`] is a runtime value tree of structs, enums, and primitives with a
//! hand-written [`Debug`] impl that prints Rust-like syntax. A
//! [`ReflRepresenter`] converts a concrete type to and from [`ReflRepr`];
//! [`ReflRepresenterRegistry`] keys them by [`std::any::TypeId`]. Because
//! `dyn ReflRepresenter` is not [`Any`](std::any::Any), the registry stores each
//! representer double-boxed as `Box<Box<dyn ReflRepresenter<Item = T>>>` erased
//! to `Box<dyn Any>` and recovers it by downcasting to the inner box type.
//! [`ReflRepresenterCtx`] lends the registry to a representer so it can recurse
//! into field types via [`ctx.into_repr`](ReflRepresenterCtx::into_repr).
//! [`from_repr`](ReflRepresenter::from_repr) defaults to `None`: the value→repr
//! direction works, but repr→value round-tripping is unimplemented for most
//! types.
//!
//! `refl_repr_reg_entries` seeds the built-ins via [`set_common_representers`]:
//! a macro generates primitive representers (integers → `Int(i128)`, floats →
//! `Float(f64)`, plus `char` and `String`), and for each item type
//! `set_common_generic_*` also registers `Option<T>` and `Vec<T>` wrappers. Only
//! one level of generic nesting is pre-registered, so e.g. `Vec<Vec<u8>>` is not
//! representable out of the box.

mod refl_map;
mod refl_repr;
mod refl_repr_reg_entries;
mod refl_ty;
mod registry;
mod tuple_from_vec_of_any;

pub use refl_map::*;
pub use refl_repr::*;
pub use refl_repr_reg_entries::*;
pub use refl_ty::*;
pub use registry::*;
pub use tuple_from_vec_of_any::*;

pub use fruits_reflection_macros::*;

pub trait ReflectTy: 'static {
    fn refl_ty() -> ReflTy;
}

// todo
trait Reflect: 'static {
    fn refl_ty() -> ReflTy;
    fn to_refl_repr(&self) -> ReflRepr;
    fn from_refl_repr(repr: &ReflRepr) -> Self;
}
