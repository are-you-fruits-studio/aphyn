//! # fruits_serialization_macros
//!
//! Provides the `#[derive(Serializable)]` macro that writes the boilerplate `Serializable`
//! impl for a struct or enum, so types opt into the engine's serialization framework without
//! hand-writing field-by-field (de)serialization.
//!
//! This crate is the procedural-macro half of `fruits_serialization`; the macro is
//! re-exported from there, so users derive it through `fruits_serialization` rather than
//! depending on this crate directly.
//!
//! # How to use
//!
//! Annotate a type with `#[derive(Serializable)]`. The type must also implement `Default`
//! (deserialization starts from it) and be `'static`. The derive generates code that names
//! `Serializable`, `SerializerCtx`, `SerializerCtxState`, and `SerializedValue` unqualified,
//! so those names must be in scope at the derive site — `use fruits_serialization::*;` brings
//! them in. Registering the type and round-tripping it is covered in `fruits_serialization`.
//!
//! #### Derive on a struct
//!
//! Each named field becomes a map entry keyed by the field name:
//!
//! ```ignore
//! use fruits_serialization::*;
//!
//! #[derive(Serializable, Default)]
//! struct Player {
//!     name: String,
//!     score: u32,
//! }
//! ```
//!
//! Tuple structs are supported too — their fields are keyed by index (`"0"`, `"1"`, …) — as
//! are unit structs, which serialize as an empty map.
//!
//! #### Derive on an enum
//!
//! Every variant is supported (unit, tuple, and struct variants). The serialized value records
//! which variant is active along with the full list of variant names. Because the type needs a
//! `Default`, mark a default variant:
//!
//! ```ignore
//! use fruits_serialization::*;
//!
//! #[derive(Serializable, Default)]
//! enum Shape {
//!     #[default]
//!     Empty,
//!     Circle(f32),
//!     Rect { w: f32, h: f32 },
//! }
//! ```
//!
//! #### Derive on a generic type
//!
//! Generic parameters, including lifetimes and const generics, are carried through to the
//! generated impl, and any `where` clause on the type is preserved:
//!
//! ```ignore
//! use fruits_serialization::*;
//!
//! #[derive(Serializable, Default)]
//! struct Wrapper<T> {
//!     value: T,
//! }
//! ```
//!
//! # How to maintain
//!
//! #### Code generation by string-building
//!
//! The entry point [`derive_serializable`] forwards to `impl_serializable::derive`. Unlike most
//! derives, it does not emit a token stream with `quote!`. It appends Rust *source text* to a
//! `String` and parses it back at the end. The two methods are produced separately by
//! `serialize_impl` and `deserialize_impl`, which each `match` on `input.data` and write the
//! method body for the struct/enum shape at hand. When changing the generated code, remember
//! you are writing strings: every brace in a `write!` format string that should reach the
//! output must be escaped (`{{` / `}}`). If the generated text fails to parse, the macro emits
//! it as a `const fail: &str = r##"…"##;` item instead (so the source can be inspected) and
//! panics only if even that fails to parse.
//!
//! #### Generics and the emitted header
//!
//! The impl is generic over an extra context-state parameter, `SerializerCtxStateTy`, bounded
//! by `Copy` plus `SerializerCtxState<FieldTy>` for every distinct field type (collected as
//! token strings from all struct fields or all enum-variant fields). The type's own generic
//! params are prepended to it, and `type_generics` is rebuilt by iterating the params and
//! emitting the lifetime, type, or const ident for each (trailing comma included). The header
//! is `impl<{params}, SerializerCtxStateTy: …> Serializable<SerializerCtxStateTy> for
//! {type_name}<{type_generics}> where Self: 'static + Default`, with the type's own `where`
//! predicates appended after a comma when present.
//!
//! #### How each shape maps to a `SerializedValue`
//!
//! Serialization always builds a *rigid* composite through `ctx.serialize_map(path)` and
//! `finish_as_map(true)` / `finish_as_enum(true, …)`, so the path handling of the map builder
//! applies. Named struct fields are keyed by field name; tuple fields are keyed by their index
//! rendered as a string; unit structs produce an empty map. Enums emit a map tagged via
//! `finish_as_enum` with the active variant name and a `variants` vector of every variant name
//! (built once, before the `match`). Deserialization is in place: structs go through
//! `ctx.deserialize_map(self, path, value)` followed by `.with_field("…", &mut self.…)` for each
//! field, and enums go through `ctx.deserialize_enum(path, value)` with one
//! `.with_variant("Name", || Self::Name { …: Default::default() })` per variant, then
//! `.finish(self, …)` whose closure feeds the chosen variant's fields to `with_field`.
//!
//! #### Variant binding names
//!
//! When generating the `match` arm for an enum variant, fields are bound to `f_<field>` for
//! struct variants and `f_<index>` for tuple variants, keeping the generated bindings from
//! colliding with the field/index names used as map keys.
//!
//! #### Unions are rejected
//!
//! `syn::Data::Union` is unsupported: collecting field types hits a `todo!()` for it, and both
//! `serialize_impl` and `deserialize_impl` also panic on it, surfacing as a compile error at
//! the derive site.

use proc_macro::TokenStream;

mod impl_serializable;

#[proc_macro_derive(Serializable)]
pub fn derive_serializable(stream: TokenStream) -> TokenStream {
    impl_serializable::derive(stream)
}
