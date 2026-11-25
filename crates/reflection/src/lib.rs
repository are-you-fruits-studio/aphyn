//! # aphyn_reflection
//!
//! Runtime reflection for engine types: reach into a type-erased value's fields, enum variants and
//! collection elements by name or index, create default values and variants, and clone values,
//! without knowing the concrete type at compile time. The editor's inspector edits through it, and
//! animations can resolve a field once and then read/write it every frame.
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
//! #### Lists
//!
//! A [`ListReflector`] reads and writes elements of an indexed list. `Vec<T>` and
//! [`FfiVec<T>`](aphyn_ffi::FfiVec) also insert and remove; fixed-size `[T; N]` and
//! [`Mat<N, T>`](aphyn_math::Mat) (a list of columns) are rigid
//! ([`is_rigid`](ListReflector::is_rigid) is `true`): the builder got no insert/remove pair.
//!
//! ```
//! use aphyn_ffi::{FfiAny, FfiAnyMut, FfiAnyRef};
//! use aphyn_reflection::*;
//!
//! let Reflector::List(reflector) = Vec::<u8>::reflector() else { unreachable!() };
//! assert!(!reflector.is_rigid());
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
//! #### Maps
//!
//! A [`MapReflector`] reads and writes values by key, passed as type-erased references.
//! [`keys`](MapReflector::keys) returns a snapshot of the keys to enumerate entries.
//! [`FfiIndexMap`](aphyn_ffi::FfiIndexMap), `HashMap` and `BTreeMap` are reflected; inserting a
//! key that is already present fails and hands both key and value back.
//!
//! ```
//! use aphyn_ffi::{FfiAny, FfiAnyMut, FfiAnyRef, FfiIndexMap, FfiString};
//! use aphyn_reflection::*;
//!
//! let Reflector::Map(reflector) = FfiIndexMap::<FfiString, u32>::reflector() else { unreachable!() };
//!
//! let mut map = FfiIndexMap::<FfiString, u32>::new();
//! map.insert("health".into(), 100);
//!
//! let health = FfiString::from("health");
//! let keys = reflector.keys(FfiAnyRef::new(&map)).unwrap();
//! assert_eq!(keys[0].downcast_ref::<FfiString>(), Some(&health));
//! drop(keys);
//!
//! let mut value = reflector.get_mut(FfiAnyMut::new(&mut map), FfiAnyRef::new(&health)).unwrap();
//! *value.downcast_mut::<u32>().unwrap() = 50;
//! assert_eq!(map.get("health"), Some(&50));
//!
//! let rejected = reflector.insert(FfiAnyMut::new(&mut map), FfiAny::new(health.clone()), FfiAny::new(1_u32));
//! assert!(rejected.is_err());
//!
//! let removed = reflector.remove(FfiAnyMut::new(&mut map), FfiAnyRef::new(&health)).unwrap();
//! assert_eq!(removed.downcast_ref::<u32>(), Some(&50));
//! assert_eq!(reflector.len(FfiAnyRef::new(&map)), Some(0));
//! ```
//!
//! #### Creating and cloning values
//!
//! A reflector may carry a default factory and a clone factory. The derive adds them when the type
//! implements `Default` / `Clone`. [`reflect_clone`] uses the clone factory, or else fills the
//! type's default with clones of its reflected parts; with neither it returns `None`.
//! [`reflect_construct_variant`] builds an enum variant from its fields' defaults.
//!
//! ```
//! use aphyn_ffi::FfiAnyRef;
//! use aphyn_reflection::*;
//!
//! #[derive(Default, Reflectable)] // no Clone: cloned as default + fill
//! struct Stats {
//!     health: u32,
//!     shield: Option<u32>,
//! }
//!
//! #[derive(Reflectable)]
//! enum Target {
//!     Nobody,
//!     Point { x: f32, y: f32 },
//! }
//!
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Stats>().register::<Target>().register::<u32>().register::<f32>().register::<Option<u32>>();
//! let registry = builder.build();
//!
//! let stats = Stats { health: 30, shield: Some(5) };
//! let cloned = reflect_clone(&registry, FfiAnyRef::new(&stats)).unwrap();
//! let cloned = cloned.downcast_ref::<Stats>().unwrap();
//! assert_eq!((cloned.health, cloned.shield), (30, Some(5)));
//!
//! let target = registry.get_of::<Target>().unwrap().as_enum().unwrap();
//! assert!(!registry.get_of::<Target>().unwrap().has_default());
//! let point = reflect_construct_variant(&registry, target.variant("Point").unwrap()).unwrap();
//! assert!(matches!(point.downcast_ref::<Target>(), Some(Target::Point { x: 0.0, y: 0.0 })));
//! ```
//!
//! Generic impls can't name `Default`/`Clone` of their parameters, so concrete instantiations
//! get their factories at registration:
//! [`register_with_factories`](ReflectionRegistryBuilder::register_with_factories) (or
//! `register_with_default` / `register_with_clone`).
//!
//! #### Paths
//!
//! A [`ReflectPath`] addresses a nested value: struct (or active variant) fields by name, list
//! elements and map entries by index. [`resolve`] / [`resolve_mut`] follow it through the registry.
//!
//! ```
//! use aphyn_ffi::{FfiAny, FfiAnyMut};
//! use aphyn_reflection::*;
//!
//! #[derive(Reflectable)]
//! struct Inventory {
//!     items: Vec<u32>,
//! }
//!
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Inventory>().register::<Vec<u32>>().register::<u32>();
//! let registry = builder.build();
//!
//! let mut inventory = Inventory { items: vec![1, 2] };
//! let path = ReflectPath::new()
//!     .join(ReflectPathSegment::Field("items".into()))
//!     .join(ReflectPathSegment::Index(1));
//!
//! let mut item = resolve_mut(&registry, FfiAnyMut::new(&mut inventory), path.segments()).unwrap();
//! item.replace(FfiAny::new(7_u32)).ok().unwrap();
//! assert_eq!(inventory.items, [1, 7]);
//! ```
//!
//! A path prints as text (`/items/[1]`) and [`ReflectPath::parse`] reads that text back, so
//! assets can store paths as strings:
//!
//! ```
//! use aphyn_reflection::*;
//!
//! let path = ReflectPath::parse("/items/[1]").unwrap();
//! assert_eq!(path.to_string(), "/items/[1]");
//! assert_eq!(ReflectPath::parse("").unwrap(), ReflectPath::new());
//! assert!(ReflectPath::parse("items").is_none());
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
//! reference cycles. [`missing_dst_types`](ReflectionRegistry::missing_dst_types) lists field
//! target, list element, map key and map value types with no registered reflector.
//!
//! #### Sharing
//!
//! [`ReflectionRegistry`] is an [`FfiArc`](aphyn_ffi::FfiArc) of an
//! [`FfiIndexMap`](aphyn_ffi::FfiIndexMap) of `FfiArc<Reflector>`; fields and enum variants are
//! `FfiArc`s too. Users clone exactly the part they need (an animation keeps the field chain of its
//! property path) and use it without borrowing the registry. The registry never changes after
//! [`ReflectionRegistryBuilder::build`]. Registering a type that is already registered keeps the
//! first reflector; [`register`](ReflectionRegistryBuilder::register) does not even build the
//! second one. [`to_builder`](ReflectionRegistry::to_builder) copies every reflector (they are
//! `Clone`) into a new builder, which is how a registry gets extended; the old registry and
//! everything cloned out of it stay valid.
//!
//! #### Erased accessors
//!
//! Getters, `is_active`, and list/map functions are stored as the user's typed `fn` pointer cast
//! to `*const ()` plus a monomorphized `extern "C-unwind"` trampoline that casts it back. This is
//! sound only because each erased pointer is called solely through the trampoline instantiated with
//! the same type parameters at the same construction site. Trampolines check the input value's type
//! info before reinterpreting it, so a value of a wrong type yields `None` / `false` / `Err`, never
//! UB. Rust-side `Option`/`Result` cross the trampolines as `FfiOption`; a rejected list insert
//! hands the element back, a rejected map insert the key and value. A list's or map's insert and
//! remove are stored together in one `FfiOption`: either both exist or the reflector is rigid.
//! Map keys are returned as a freshly collected `FfiVec` snapshot on every call. The default and
//! clone factories (`factories.rs`, one optional pair per reflector) and enum variant constructors
//! are erased the same way; a constructor takes the variant's fields as an `FfiVec<FfiAny>` in
//! field order and fails on a count or type mismatch. [`FfiAnyMut::replace`](aphyn_ffi::FfiAnyMut::replace)
//! is how [`reflect_clone`] and the users of [`resolve_mut`] write a whole value into a place.
//!
//! #### Clone, default and paths
//!
//! [`reflect_clone`] prefers the clone factory. Otherwise it starts from the default factory and
//! fills it: struct fields are replaced by clones, a rigid list must have the source's length and
//! gets its elements replaced, a resizable list or a map gets clones appended/inserted (a key the
//! default already holds gets its value replaced). An enum is rebuilt from clones of its active
//! variant's fields, so it needs no default. Hidden state (a field the reflector does not list)
//! keeps its default. [`resolve_mut`] clones map keys before looking an entry up mutably, because
//! the key snapshot borrows the map; a map entry index is a position in that snapshot.
//!
//! #### Built-in impls and the derive
//!
//! `impls.rs` implements [`Reflectable`] for primitives, `String`, `FfiString`, `FfiSmallString`
//! (values), `Vec`, `FfiVec`, arrays, `Mat` (lists), `FfiIndexMap`, `HashMap`, `BTreeMap` (maps),
//! `Option`, `FfiOption` (enums) and `Vec2`/`Vec3`/`Vec4`/`Quat` (structs). Generic impls only
//! require `T: 'static` (plus what the map itself needs of its key): a field's target is referenced
//! by name, so its reflectability is checked by the registry, not by the type system.
//! Values get both factories; the collections and options get a default factory (it needs no bound
//! on `T`); `Option`/`FfiOption` get variant constructors. The rest comes from
//! `register_with_factories` on concrete instantiations.
//! The derive lives in `aphyn_reflection_macros` and generates the same builder calls as the
//! hand-written example above; its generated code names this crate's items unqualified, including
//! the hidden `__reflect_default_fn!` / `__reflect_clone_fn!` macros (autoref specialization over
//! [`ReflectProbe`]) and [`__reflect_take`].

mod factories;
mod field;
mod impls;
mod ops;
mod reflector;
mod registry;

pub use factories::{
    ReflectProbe, ReflectProbeClone, ReflectProbeCloneFallback, ReflectProbeDefault, ReflectProbeDefaultFallback, __reflect_take,
};
pub use field::*;
pub use ops::*;
pub use reflector::*;
pub use registry::*;

pub use aphyn_reflection_macros::*;

pub trait Reflectable: 'static {
    fn reflector() -> Reflector;
}
