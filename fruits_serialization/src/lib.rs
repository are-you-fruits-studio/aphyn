//! # fruits_serialization
//!
//! A format-agnostic serialization framework: it converts Rust values to and from an
//! in-memory value tree that a concrete format (currently JSON) can be projected onto. The
//! serializer for a nested value is chosen either statically or at runtime from a registry.
//! The engine uses it to persist and load components, prefabs, assets, and other world data.
//!
//! # How to use
//!
//! The unit of data is [`SerializedValue`], an in-memory tree of nulls, primitives, and
//! composites. A type becomes serializable by implementing [`Serializable`] (usually through
//! the [`derive macro`](derive@Serializable)). Serialization runs through a [`SerializerCtx`],
//! which carries a *state* that decides how nested values are handled, and an error callback.
//! Two states ship with the crate: [`PureSerializerCtxState`] dispatches statically to each
//! type's own [`Serializable`] impl, and [`TransSerializerCtxState`] looks the serializer up at
//! runtime in a [`TransSerializerRegistry`]. Inside the engine the shared registry already lives
//! in the world as [`SerializersResource`], inserted (and filled with the common types) by the
//! engine's default-modules setup.
//!
//! #### Round-trip a value without a registry
//!
//! Derive [`Serializable`] (the type must also implement [`Default`]) and drive it with a
//! [`PureSerializerCtxState`] context. Deserialization writes into an existing value;
//! [`deserialize_default`](SerializerCtx::deserialize_default) starts from `T::default()`:
//!
//! ```
//! use fruits_serialization::*;
//!
//! #[derive(Serializable, Default, PartialEq, Debug)]
//! struct Player {
//!     name: String,
//!     score: u32,
//! }
//!
//! let mut on_err = |err: SerializationError| panic!("{err}");
//! let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);
//!
//! let player = Player { name: String::from("Ada"), score: 42 };
//!
//! let serialized = ctx.serialize(&player, "");
//! let restored: Player = ctx.deserialize_default("", &serialized);
//!
//! assert_eq!(restored, player);
//! ```
//!
//! #### Round-trip a value through a registry
//!
//! Register a [`StandardSerializer`] for the type and for every type it contains, then call the
//! registry's [`serialize`](TransSerializerRegistry::serialize) /
//! [`deserialize`](TransSerializerRegistry::deserialize). Errors are passed to the callback
//! instead of being returned:
//!
//! ```
//! use fruits_serialization::*;
//!
//! #[derive(Serializable, Default, PartialEq, Debug)]
//! struct Player {
//!     name: String,
//!     score: u32,
//! }
//!
//! let mut registry = TransSerializerRegistry::new();
//! // every type that appears in the data must have a serializer registered
//! registry.register(StandardSerializer::<String>::default());
//! registry.register(StandardSerializer::<u32>::default());
//! registry.register(StandardSerializer::<Player>::default());
//!
//! let player = Player { name: String::from("Ada"), score: 42 };
//!
//! let mut errors = Vec::new();
//! let mut on_err = |err| errors.push(err);
//!
//! let serialized = registry.serialize(&player, "", &mut on_err);
//! let mut restored = Player::default();
//! registry.deserialize(&mut restored, "", &serialized, &mut on_err);
//!
//! assert!(errors.is_empty());
//! assert_eq!(restored, player);
//! ```
//!
//! #### Read or write a single field by path
//!
//! Every call takes a `/`-separated path. An empty path addresses the whole value; a non-empty
//! path addresses one nested field or list element, so only that part is produced or
//! overwritten:
//!
//! ```
//! use fruits_serialization::*;
//!
//! #[derive(Serializable, Default)]
//! struct Player {
//!     name: String,
//!     score: u32,
//! }
//!
//! let mut on_err = |err: SerializationError| panic!("{err}");
//! let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);
//!
//! let mut player = Player { name: String::from("Ada"), score: 42 };
//!
//! let score = ctx.serialize(&player, "score");
//! assert!(matches!(score, SerializedValue::Primitive(SerializedPrimitive::Int(42))));
//!
//! ctx.deserialize(&mut player, "score", &SerializedPrimitive::Int(7).into());
//! assert_eq!(player.score, 7);
//! assert_eq!(player.name, "Ada");
//! ```
//!
//! #### Project to and from JSON
//!
//! [`SerializedValue::to_json`] renders the value tree as a [`serde_json::Value`], and
//! [`SerializedValue::from_json`] reads one back:
//!
//! ```
//! use fruits_serialization::*;
//!
//! let value = SerializedValue::Primitive(SerializedPrimitive::Int(42));
//!
//! let json = value.to_json();
//! assert_eq!(json.to_string(), "42");
//!
//! let restored = SerializedValue::from_json(&json);
//! assert_eq!(restored.to_json().to_string(), "42");
//! ```
//!
//! #### Implement `Serializable` by hand
//!
//! When the derive does not fit, implement [`Serializable`] for every state that can handle the
//! field types. Build a map with [`SerializerCtx::serialize_map`] and read it back with
//! [`SerializerCtx::deserialize_map`]; both honour the path, so the impl supports field access
//! for free:
//!
//! ```
//! use fruits_serialization::*;
//!
//! #[derive(Default, PartialEq, Debug)]
//! struct Point { x: f32, y: f32 }
//!
//! impl<S: SerializerCtxState<f32>> Serializable<S> for Point {
//!     fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
//!         ctx.serialize_map(path)
//!             .with_field("x", &self.x)
//!             .with_field("y", &self.y)
//!             .finish_as_map(true)
//!     }
//!
//!     fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
//!         ctx.deserialize_map(self, path, serialized)
//!             .with_field("x", &mut self.x)
//!             .with_field("y", &mut self.y);
//!     }
//! }
//!
//! let mut on_err = |err: SerializationError| panic!("{err}");
//! let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);
//!
//! let point = Point { x: 1.0, y: 2.0 };
//! let serialized = ctx.serialize(&point, "");
//! let restored: Point = ctx.deserialize_default("", &serialized);
//!
//! assert_eq!(restored, point);
//! ```
//!
//! # How to maintain
//!
//! #### The value tree
//!
//! [`SerializedValue`] is the format-independent intermediate representation everything
//! passes through. It is `Null`, a [`SerializedPrimitive`] (`Bool`, `Int` widened to `i128`,
//! `Float` widened to `f64`, or `String`), or a [`SerializedComposite`]. A composite holds
//! either an ordered [`SerializedMap`] (an `FfiIndexMap`, so field order is preserved) or a
//! `List`, plus an `is_rigid` flag marking values that came from a fixed shape — a struct,
//! tuple, array, or enum variant — as opposed to a dynamic collection. A map may also carry
//! [`SerializedEnumMetadata`] recording the active variant and the full variant list. All of
//! these types are `#[repr(C)]` and built from `fruits_ffi` containers.
//! [`similar`](SerializedValue::similar) compares two trees while ignoring `is_rigid` and the
//! variant list (the editor uses it to detect changed components), and
//! [`get_by_path`](SerializedValue::get_by_path) /
//! [`get_or_insert_by_path`](SerializedValue::get_or_insert_by_path) walk a tree by path.
//! `SerializedValue` is itself [`Serializable`], which is how prefabs carry component payloads
//! verbatim.
//!
//! #### Paths
//!
//! Paths are `/`-separated (leading and trailing slashes are trimmed by
//! [`normalize_serialization_path`]); [`decompose_serialization_path`] splits off the first
//! segment. Every [`Serializable`] impl must honour the path: with an empty path it handles the
//! whole value; otherwise it forwards the remainder to the addressed field or element only, and
//! primitives return `Null` / ignore the input. The map and list builders implement this by
//! switching between a *root* state (collect all fields) and a *field* state (serialize or
//! deserialize only the matching one). [`deserialize_map`](SerializerCtx::deserialize_map) and
//! [`deserialize_list`](SerializerCtx::deserialize_list) reset the target to its default only
//! for an empty path, so a field missing from the input keeps its default, while a path write
//! leaves the rest of the value untouched.
//!
//! #### Context states
//!
//! [`Serializable<S>`] is generic over the context state `S`, and the state decides how a
//! nested `T` is handled through [`SerializerCtxState<T>`]. [`PureSerializerCtxState`]
//! implements it for every `T: Serializable<PureSerializerCtxState>` by calling `T`'s impl
//! directly. [`TransSerializerCtxState`] implements it for every `T: 'static` by looking up a
//! serializer for `T` in its registry, then in each wider state it wraps
//! ([`wrap_with_local`](TransSerializerCtxState::wrap_with_local)), innermost first. This is how
//! the asset and prefab code in `fruits_asset_loading` layers transient, borrowing serializers
//! (for example one that remaps `EntityId`s) over the shared [`SerializersResource`].
//! [`SerializerCtx::map_state`] swaps the state, which the prefab loader uses to drop from a
//! registry-backed context to a pure one.
//!
//! #### Serializers and the registry
//!
//! [`Serializer<S>`] is what a registry stores: an object that (de)serializes a `Deserialized`
//! type and can create a default instance of it
//! ([`serializable_default`](Serializer::serializable_default)). [`StandardSerializer<T>`]
//! bridges a [`Serializable`] type to it, which is why callers register
//! `StandardSerializer::<T>::default()` rather than the type itself; custom serializers (asset
//! handles, entity ids) implement [`Serializer`] directly.
//!
//! [`TransSerializerRegistry`] keys each entry by `std::any::type_name` of its `Deserialized`
//! type and stores it as a [`TransSerializerFfi`]: the serializer boxed in an `FfiDroppable`
//! plus a static `extern "C-unwind"` vtable, so the registry stays FFI-safe. Its lifetime
//! parameter lets a registry hold serializers that borrow local data. Typed lookups cast the
//! erased pointers back to `T` guarded only by the type-name string. **Caveat for
//! maintainers:** `type_name` is not guaranteed to be unique or stable across builds, so this
//! scheme is sound only as long as distinct types never collide on their `type_name` string.
//!
//! #### Type-erased entry points
//!
//! On a [`TransSerializerCtxState`] context, [`serialize_any`](SerializerCtx::serialize_any) and
//! [`deserialize_any`](SerializerCtx::deserialize_any) take an `FfiAnyRef` / `FfiAnyMut` and
//! find the serializer by the value's type-info name (the same `type_name` string), and
//! [`deserialize_default_any`](SerializerCtx::deserialize_default_any) creates the value from
//! the serializer's default and returns it as an `FfiAny`. `fruits_prefab` uses these to
//! (de)serialize components it only knows by type name.
//!
//! #### Errors are reported, not returned; deserialization is lenient
//!
//! A [`SerializerCtx`] carries an `FfiFnMutMut` error callback, and every problem is passed to
//! it as a [`SerializationError`] while (de)serialization carries on. Deserialization is
//! deliberately coercive: reading an int from a string, a bool from a number, a `Vec` from a
//! rigid composite, and so on each yields a best-effort value while reporting an `InvalidInput`
//! error. A missing registered serializer reports `MissingSerializer`; serialization then yields
//! `Null` and deserialization leaves the target unchanged.
//!
//! #### Enum decoding
//!
//! [`SerializerCtx::deserialize_enum`] builds an [`EnumDeserializerCtx`]; each
//! [`with_variant`](EnumDeserializerCtx::with_variant) supplies a constructor for that variant
//! with defaulted fields. At the root path, `finish` picks the variant named in the enum
//! metadata; if none matches it reports an error and falls back to the first variant. It then
//! replaces the target with the constructed variant and deserializes its fields. At a non-empty
//! path the current variant is kept and the path is forwarded into its fields. An enum with no
//! variants panics.
//!
//! #### The JSON projection is lossy
//!
//! [`SerializedValue::to_json`] clamps `Int` into the `[i64::MIN, u64::MAX]` range and maps
//! non-representable (e.g. non-finite) `Float`s to `Null`. Enum metadata is written under the
//! reserved key `$enum_variant`; a real field literally named `$enum_variant` is skipped with
//! a warning on `stderr`. [`SerializedValue::from_json`] cannot recover the `is_rigid` flag
//! (always reconstructed as `false`) nor the enum variant list (left empty), so a
//! JSON round-trip is not structure-preserving for those fields.
//!
//! #### Built-in impls and gaps
//!
//! The crate implements [`Serializable`] for integers, floats, `bool`, `char`, `String`,
//! `FfiString`, `FfiSmallString`, `&'static str` (serialize only; deserializing reports an
//! error), `()`, `Vec`, `FfiVec`, `Option`, `FfiOption` (as enums with `None` / `Some`),
//! fixed-size arrays, and the `fruits_math` vectors and quaternion (as maps keyed
//! `x`/`y`/`z`/`w`). `u128` values above `i128::MAX` are clamped. The `Mat<N, T>` impl is a
//! `todo!()` and panics if called. The numerous `// todo: ffi` markers flag where the FFI
//! surface is still being built out, alongside the crate root's `todo` list and the
//! commented-out older `FfiAny` support in `serialization_transitive.rs`.
//!
//! #### Code generation
//!
//! The [`Serializable`](derive@Serializable) derive (in `fruits_serialization_macros`,
//! re-exported here) emits impls by building Rust *source text* and parsing it. The impl is
//! generic over a context state bounded by `SerializerCtxState` for every field type, and
//! requires `Self: 'static + Default`. Named structs become rigid maps keyed by field name;
//! tuple structs become rigid maps keyed by the field index as a string (`"0"`, `"1"`, …);
//! enums become rigid maps tagged with the variant name and full variant list. Unions are
//! rejected with a panic.

mod serialization_transitive;
mod serialization_pure;
mod serialization_registry;
mod serialization_impls;
mod serialization_model;
mod serialization_ecs;
mod serialization_utils;
mod serialization_core;
mod serialization_ctx;

pub use fruits_serialization_macros::*;
pub use serialization_transitive::*;
pub use serialization_pure::*;
pub use serialization_registry::*;
pub use serialization_impls::*;
pub use serialization_model::*;
pub use serialization_ecs::*;
pub use serialization_utils::*;
pub use serialization_core::*;
pub use serialization_ctx::*;

// todo:
// + impls for standard types
// + macros
// - editor
// - ffi
// + names tuple/struct -> list/map
// - ecs resource (and other public APIs)
// - refactor?