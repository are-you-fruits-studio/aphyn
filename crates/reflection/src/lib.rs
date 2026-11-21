//! # aphyn_reflection
//!
//! Runtime reflection for engine types: reach into a type-erased value's fields, enum variants and
//! collection elements by name or index, without knowing the concrete type at compile time. It is
//! the groundwork for animations (which resolve a field once and then read/write it every frame)
//! and for other tooling that operates over arbitrary types.
//!
//! # How to use
//!
//! A type describes itself through [`Reflectable`], whose single method builds its [`Reflector`].
//! Derive it with [`Reflectable`](derive@Reflectable) or write it by hand with the builders.
//! Reflectors are collected in a [`ReflectionRegistryBuilder`] and built into a read-only
//! [`ReflectionRegistry`], which looks them up by [`std::any::type_name`].
//!
//! #### Register and access a field
//!
//! ```
//! use aphyn_ffi::{FfiAnyMut, FfiAnyRef};
//! use aphyn_reflection::*;
//!
//! #[derive(Reflectable)]
//! struct Player {
//!     health: u32,
//!     name: String,
//! }
//!
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Player>().register::<u32>().register::<String>();
//! let registry = builder.build();
//!
//! let reflector = registry.get_of::<Player>().unwrap();
//! let health = reflector.as_struct().unwrap().field("health").unwrap().clone();
//!
//! let mut player = Player { health: 100, name: String::from("Barry") };
//! *health.get_mut(FfiAnyMut::new(&mut player)).unwrap().downcast_mut::<u32>().unwrap() = 50;
//! assert_eq!(player.health, 50);
//!
//! // a field only names its type; the registry resolves it
//! let health_reflector = registry.dst_of(&health).unwrap();
//! assert!(health_reflector.as_value().is_some());
//!
//! // a field reference is shared and outlives the registry
//! drop(registry);
//! assert_eq!(health.get(FfiAnyRef::new(&player)).unwrap().downcast_ref::<u32>(), Some(&50));
//! ```
//!
//! #### Enums
//!
//! An [`EnumReflector`] lists every variant; [`active_variant`](EnumReflector::active_variant)
//! tells which one a value holds. Variant fields return `None` while another variant is active.
//! Tuple fields (of structs and variants) are named by index: `"0"`, `"1"`, …
//!
//! ```
//! use aphyn_ffi::FfiAnyRef;
//! use aphyn_reflection::*;
//!
//! #[derive(Reflectable)]
//! enum PersonId {
//!     Username(String),
//!     Token { value: u64 },
//! }
//!
//! let Reflector::Enum(reflector) = PersonId::reflector() else { unreachable!() };
//!
//! let id = PersonId::Token { value: 7 };
//! let active = reflector.active_variant(FfiAnyRef::new(&id)).unwrap();
//! assert_eq!(active.name(), "Token");
//!
//! let value = active.field("value").unwrap();
//! assert_eq!(value.get(FfiAnyRef::new(&id)).unwrap().downcast_ref::<u64>(), Some(&7));
//!
//! let username = reflector.variant("Username").unwrap().field("0").unwrap();
//! assert!(username.get(FfiAnyRef::new(&id)).is_none());
//! ```
//!
//! #### Collections
//!
//! A [`CollectionReflector`] reads and writes elements of an indexed list. `Vec<T>` and
//! [`FfiVec<T>`](aphyn_ffi::FfiVec) also insert and remove; fixed-size `[T; N]` and
//! [`Mat<N, T>`](aphyn_math::Mat) (a list of columns) do not
//! ([`is_resizable`](CollectionReflector::is_resizable) is `false`).
//!
//! ```
//! use aphyn_ffi::{FfiAny, FfiAnyMut, FfiAnyRef};
//! use aphyn_reflection::*;
//!
//! let Reflector::Collection(reflector) = Vec::<u8>::reflector() else { unreachable!() };
//!
//! let mut list = vec![1_u8, 3];
//! assert!(reflector.insert(FfiAnyMut::new(&mut list), 1, FfiAny::new(2_u8)).is_ok());
//! assert!(reflector.insert(FfiAnyMut::new(&mut list), 0, FfiAny::new(2_u32)).is_err());
//! assert_eq!(list, [1, 2, 3]);
//!
//! assert_eq!(reflector.len(FfiAnyRef::new(&list)), Some(3));
//! let removed = reflector.remove(FfiAnyMut::new(&mut list), 0).unwrap();
//! assert_eq!(removed.downcast_ref::<u8>(), Some(&1));
//! ```
//!
//! #### Hand-written reflectors
//!
//! The builders take non-capturing closures, which coerce to plain `fn` pointers.
//! [`ValueReflector`] marks a leaf type with no reflected inner structure.
//!
//! ```
//! use aphyn_reflection::*;
//!
//! struct Health(u32);
//!
//! impl Reflectable for Health {
//!     fn reflector() -> Reflector {
//!         StructReflectorBuilder::<Self>::new()
//!             .field("0", |v| Some(&v.0), |v| Some(&mut v.0))
//!             .build()
//!     }
//! }
//!
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Health>();
//! builder.register_reflector(ValueReflector::of::<Health>()); // ignored: the first one is kept
//! let registry = builder.build();
//!
//! assert!(registry.get_of::<Health>().unwrap().as_struct().is_some());
//! assert_eq!(registry.missing_dst_types(), [std::any::type_name::<u32>()]);
//! ```
//!
//! # How to maintain
//!
//! #### Identity and lookup
//!
//! Types are identified by [`std::any::type_name`], as everywhere else across the dynamic-library
//! boundary (`FfiShortTypeInfo::does_match` compares size, align and that name). Every reflector
//! and field stores [`FfiShortTypeInfo`](aphyn_ffi::FfiShortTypeInfo)s, not the names themselves.
//! A field knows only its target type's name, never its reflector: resolving goes through
//! [`ReflectionRegistry::dst_of`]. This keeps registration order free and recursive types free of
//! reference cycles. [`missing_dst_types`](ReflectionRegistry::missing_dst_types) lists target and
//! element types with no registered reflector.
//!
//! #### Sharing
//!
//! [`ReflectionRegistry`] is an [`FfiArc`](aphyn_ffi::FfiArc) of an
//! [`FfiIndexMap`](aphyn_ffi::FfiIndexMap) of `FfiArc<Reflector>`; fields and enum variants are
//! `FfiArc`s too. Users clone exactly the part they need (an animation keeps the field chain of its
//! property path) and use it without borrowing the registry. The registry never changes after
//! [`ReflectionRegistryBuilder::build`]. Registering a type that is already registered keeps the
//! first reflector; [`register`](ReflectionRegistryBuilder::register) does not even build the
//! second one.
//!
//! #### Erased accessors
//!
//! Getters, `is_active`, and collection functions are stored as the user's typed `fn` pointer cast
//! to `*const ()` plus a monomorphized `extern "C-unwind"` trampoline that casts it back. This is
//! sound only because each erased pointer is called solely through the trampoline instantiated with
//! the same type parameters at the same construction site. Trampolines check the input value's type
//! info before reinterpreting it, so a value of a wrong type yields `None` / `false` / `Err`, never
//! UB. Rust-side `Option`/`Result` cross the trampolines as `FfiOption`; a rejected collection
//! insert hands the element back.
//!
//! #### Built-in impls and the derive
//!
//! `impls.rs` implements [`Reflectable`] for primitives, `String`, `FfiString`, `FfiSmallString`
//! (values), `Vec`, `FfiVec`, arrays, `Mat` (collections), `Option`, `FfiOption` (enums) and
//! `Vec2`/`Vec3`/`Vec4`/`Quat` (structs). Generic impls only require `T: 'static`: a field's target
//! is referenced by name, so its reflectability is checked by the registry, not by the type system.
//! The derive lives in `aphyn_reflection_macros` and generates the same builder calls as the
//! hand-written example above; its generated code names this crate's items unqualified.

mod field;
mod impls;
mod reflector;
mod registry;

pub use field::*;
pub use reflector::*;
pub use registry::*;

pub use aphyn_reflection_macros::*;

pub trait Reflectable: 'static {
    fn reflector() -> Reflector;
}
