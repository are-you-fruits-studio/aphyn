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
//! [`ReflectionRegistry`], which looks them up by the reflected type's `AnyTypeId`. Values travel as
//! `aphyn_any` anys (`AnyRef`, `AnyMut`, `AnyBox`). Every registration takes the
//! `TypesRegistryCache` of the world the values live in and resolves every type the reflector
//! touches to its id there, so type checks compare ids.
//!
//! #### Register and access a field
//!
//! ```
//! use aphyn_any::*;
//! use aphyn_reflection::*;
//!
//! #[derive(Reflectable)]
//! struct Player {
//!     health: u32,
//!     name: String,
//! }
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Player>(&types).register::<u32>(&types).register::<String>(&types);
//! let registry = builder.build();
//!
//! let reflector = registry.get_of::<Player>(&types).unwrap();
//! let health = reflector.as_struct().unwrap().field("health").unwrap().clone();
//!
//! let mut player = Player { health: 100, name: String::from("Barry") };
//! let mut value = health.get_mut(AnyMut::new(&mut player, types.type_id_of())).unwrap();
//! *value.downcast_mut(types.type_id_of::<u32>()).unwrap() = 50;
//! assert_eq!(player.health, 50);
//!
//! // a field knows its type's id; the registry resolves its reflector by that id
//! assert_eq!(health.dst_type_id(), types.type_id_of::<u32>().any_type_id());
//! assert!(registry.dst_of(&health).unwrap().as_value().is_some());
//!
//! // a field reference is shared and outlives the registry
//! drop(registry);
//! let player_ref = AnyRef::new(&player, types.type_id_of());
//! assert_eq!(health.get(player_ref).unwrap().downcast_ref(types.type_id_of::<u32>()), Some(&50));
//! ```
//!
//! #### Enums
//!
//! An [`EnumReflector`] lists every variant; [`active_variant`](EnumReflector::active_variant)
//! tells which one a value holds. Variant fields return `None` while another variant is active.
//! Tuple fields (of structs and variants) are named by index: `"0"`, `"1"`, …
//!
//! ```
//! use aphyn_any::*;
//! use aphyn_reflection::*;
//!
//! #[derive(Reflectable)]
//! enum PersonId {
//!     Username(String),
//!     Token { value: u64 },
//! }
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<PersonId>(&types);
//! let registry = builder.build();
//! let reflector = registry.get_of::<PersonId>(&types).unwrap().as_enum().unwrap();
//!
//! let id = PersonId::Token { value: 7 };
//! let id_ref = AnyRef::new(&id, types.type_id_of());
//! let active = reflector.active_variant(id_ref).unwrap();
//! assert_eq!(active.name(), "Token");
//!
//! let value = active.field("value").unwrap();
//! assert_eq!(value.get(id_ref).unwrap().downcast_ref(types.type_id_of::<u64>()), Some(&7));
//!
//! let username = reflector.variant("Username").unwrap().field("0").unwrap();
//! assert!(username.get(id_ref).is_none());
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
//! use aphyn_any::*;
//! use aphyn_reflection::*;
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Vec<u8>>(&types);
//! let registry = builder.build();
//! let reflector = registry.get_of::<Vec<u8>>(&types).unwrap().as_list().unwrap();
//! assert!(!reflector.is_rigid());
//!
//! let mut list = vec![1_u8, 3];
//! let list_id = types.type_id_of::<Vec<u8>>();
//! assert!(reflector.insert(AnyMut::new(&mut list, list_id), 1, AnyBox::new(2_u8, &types)).is_ok());
//! assert!(reflector.insert(AnyMut::new(&mut list, list_id), 0, AnyBox::new(2_u32, &types)).is_err());
//! assert_eq!(list, [1, 2, 3]);
//!
//! assert_eq!(reflector.len(AnyRef::new(&list, list_id)), Some(3));
//! let removed = reflector.remove(AnyMut::new(&mut list, list_id), 0).unwrap();
//! assert_eq!(removed.downcast_ref(types.type_id_of::<u8>()), Some(&1));
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
//! use aphyn_any::*;
//! use aphyn_ffi::{FfiIndexMap, FfiString};
//! use aphyn_reflection::*;
//!
//! type Stats = FfiIndexMap<FfiString, u32>;
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Stats>(&types);
//! let registry = builder.build();
//! let reflector = registry.get_of::<Stats>(&types).unwrap().as_map().unwrap();
//!
//! let (map_id, key_id, value_id) = (types.type_id_of::<Stats>(), types.type_id_of::<FfiString>(), types.type_id_of::<u32>());
//! let mut map = Stats::new();
//! map.insert("health".into(), 100);
//!
//! let health = FfiString::from("health");
//! let keys = reflector.keys(AnyRef::new(&map, map_id)).unwrap();
//! assert_eq!(keys[0].downcast_ref(key_id), Some(&health));
//! drop(keys);
//!
//! let mut value = reflector.get_mut(AnyMut::new(&mut map, map_id), AnyRef::new(&health, key_id)).unwrap();
//! *value.downcast_mut(value_id).unwrap() = 50;
//! assert_eq!(map.get("health"), Some(&50));
//!
//! let rejected = reflector.insert(AnyMut::new(&mut map, map_id), AnyBox::new(health.clone(), &types), AnyBox::new(1_u32, &types));
//! assert!(rejected.is_err());
//!
//! let removed = reflector.remove(AnyMut::new(&mut map, map_id), AnyRef::new(&health, key_id)).unwrap();
//! assert_eq!(removed.downcast_ref(value_id), Some(&50));
//! assert_eq!(reflector.len(AnyRef::new(&map, map_id)), Some(0));
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
//! use aphyn_any::*;
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
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Stats>(&types).register::<Target>(&types).register::<u32>(&types).register::<f32>(&types).register::<Option<u32>>(&types);
//! let registry = builder.build();
//!
//! let stats = Stats { health: 30, shield: Some(5) };
//! let cloned = reflect_clone(&registry, AnyRef::new(&stats, types.type_id_of())).unwrap();
//! let cloned = cloned.downcast_ref(types.type_id_of::<Stats>()).unwrap();
//! assert_eq!((cloned.health, cloned.shield), (30, Some(5)));
//!
//! let target = registry.get_of::<Target>(&types).unwrap().as_enum().unwrap();
//! assert!(!registry.get_of::<Target>(&types).unwrap().has_default());
//! let point = reflect_construct_variant(&registry, target.variant("Point").unwrap()).unwrap();
//! assert!(matches!(point.downcast_ref(types.type_id_of::<Target>()), Some(Target::Point { x: 0.0, y: 0.0 })));
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
//! use aphyn_any::*;
//! use aphyn_reflection::*;
//!
//! #[derive(Reflectable)]
//! struct Inventory {
//!     items: Vec<u32>,
//! }
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Inventory>(&types).register::<Vec<u32>>(&types).register::<u32>(&types);
//! let registry = builder.build();
//!
//! let mut inventory = Inventory { items: vec![1, 2] };
//! let path = ReflectPath::new()
//!     .join(ReflectPathSegment::Field("items".into()))
//!     .join(ReflectPathSegment::Index(1));
//!
//! let mut item = resolve_mut(&registry, AnyMut::new(&mut inventory, types.type_id_of()), path.segments()).unwrap();
//! item.replace(AnyBox::new(7_u32, &types)).ok().unwrap();
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
//! [`ValueReflector`] marks a leaf type with no reflected inner structure. An enum variant's
//! constructor takes its field values from a [`ReflectVariantFields`] in the order the variant
//! declares them; `take` is `unsafe` because it trusts that order and those types.
//!
//! ```
//! use aphyn_any::*;
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
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let mut builder = ReflectionRegistryBuilder::new();
//! builder.register::<Health>(&types);
//! builder.register_reflector(&types, ValueReflector::of::<Health>()); // ignored: the first one is kept
//! let registry = builder.build();
//!
//! assert!(registry.get_of::<Health>(&types).unwrap().as_struct().is_some());
//! assert_eq!(registry.missing_dst_types(), [std::any::type_name::<u32>()]);
//! ```
//!
//! # How to maintain
//!
//! #### Identity and lookup
//!
//! Reflectors are stored in a dense vec indexed by the reflected type's id in the builder's types
//! registry ([`ReflectionRegistry::get`] returns `None` for an id of another registry;
//! [`get_for`](ReflectionRegistry::get_for) resolves such a value's type through its type info first). Every reflector, field, list element, map key and map value stores its type's
//! `&'static` [`FfiExtendedTypeInfo`](aphyn_ffi::FfiExtendedTypeInfo) plus an `AnyTypeId` that
//! starts unresolved. Registering ([`ReflectionRegistryBuilder::register`] and its variants)
//! resolves each one against the given `TypesRegistryCache` (by type info, so types need no prior
//! registration there) and stores a refcounted registry handle in every reflector, which boxes
//! created by the reflector keep alive; fields and variants shared as `FfiArc`s are rebuilt with
//! their ids at that point. The builder remembers the registry of its first registration and
//! panics when a later one passes a cache of another registry. [`build`](ReflectionRegistryBuilder::build)
//! only wraps the reflectors. A field
//! knows its target type's id but not its reflector: resolving goes through
//! [`ReflectionRegistry::dst_of`] (by the field's target id). This keeps registration order free and recursive types
//! free of reference cycles. [`missing_dst_types`](ReflectionRegistry::missing_dst_types) lists
//! field target, list element, map key and map value types with no registered reflector.
//!
//! Register against the types registry of the world whose values the registry will see (the init
//! contexts' `types` is that registry). Values from another registry still work: ids are only
//! compared when the registry tags match, otherwise `aphyn_any` falls back to comparing type info.
//!
//! #### Sharing
//!
//! [`ReflectionRegistry`] is an [`FfiArc`](aphyn_ffi::FfiArc) of an
//! [`FfiIndexMap`](aphyn_ffi::FfiIndexMap) of `FfiArc<Reflector>`; fields and enum variants are
//! `FfiArc`s too. Users clone exactly the part they need (an animation keeps the field chain of its
//! property path) and use it without borrowing the registry. The registry never changes after
//! [`ReflectionRegistryBuilder::build`]. Registering a type that is already registered keeps the
//! first reflector; [`register`](ReflectionRegistryBuilder::register) does not even build the
//! second one. A built registry cannot be extended: inside the engine all types are registered
//! during app init and the registry is built once, when the app starts running.
//!
//! #### Erased accessors
//!
//! Getters, `is_active`, and list/map functions are stored as the user's typed `fn` pointer cast
//! to `*const ()` plus a monomorphized `extern "C-unwind"` trampoline that casts it back. This is
//! sound only because each erased pointer is called solely through the trampoline instantiated with
//! the same type parameters at the same construction site. The caller passes the resolved ids
//! (owner, element, key, value) into the trampoline, which checks its input against the owner id
//! (one integer compare when the registries match) and creates child anys with the child's id, so a
//! value of a wrong type yields `None` / `false` / `Err`, never UB. Trampolines that return boxes
//! (factories, list/map remove, rejected inserts, variant constructors) also get an `AnyBoxCtx`
//! with the reflector's registry handle. Rust-side `Option`/`Result` cross the trampolines as
//! `FfiOption`; a rejected list insert hands the element back, a rejected map insert the key and
//! value. A list's or map's insert and remove are stored together in one `FfiOption`: either both
//! exist or the reflector is rigid. Map keys are returned as a freshly collected `FfiVec` snapshot
//! on every call. The default and clone factories (`factories.rs`, one optional pair per
//! reflector) and enum variant constructors are erased the same way; a constructor receives the
//! variant's fields as a [`ReflectVariantFields`] (the boxes plus the fields' resolved ids, in field
//! order) and fails on a count or type mismatch. `AnyMut::replace` is how [`reflect_clone`] and the
//! users of [`resolve_mut`] write a whole value into a place.
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
//! require `T: 'static` (plus what the map itself needs of its key): a field's target is looked up
//! by id at run time, so its reflectability is checked by the registry, not by the type system.
//! Values get both factories; the collections and options get a default factory (it needs no bound
//! on `T`); `Option`/`FfiOption` get variant constructors. The rest comes from
//! `register_with_factories` on concrete instantiations.
//! The derive lives in `aphyn_reflection_macros` and generates the same builder calls as the
//! hand-written example above; its generated code names this crate's items unqualified, including
//! the hidden `__reflect_default_fn!` / `__reflect_clone_fn!` macros (autoref specialization over
//! [`ReflectProbe`]), and takes variant fields with `ReflectVariantFields::take`.

mod factories;
mod field;
mod impls;
mod ops;
mod reflector;
mod registry;

pub use factories::{
    ReflectProbe, ReflectProbeClone, ReflectProbeCloneFallback, ReflectProbeDefault, ReflectProbeDefaultFallback, ReflectVariantFields,
};
pub use field::*;
pub use ops::*;
pub use reflector::*;
pub use registry::*;

pub use aphyn_reflection_macros::*;

pub trait Reflectable: 'static {
    fn reflector() -> Reflector;
}
