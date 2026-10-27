//! # fruits_reflection_macros
//!
//! Derive macro that gives a type a compile-time description of its own shape,
//! so the engine's reflection system can inspect fields and variants without
//! hand-written boilerplate.
//!
//! # How to use
//!
//! This crate is not used directly — it is re-exported by `fruits_reflection`,
//! which also supplies the [`ReflectTy`] trait and the `ReflTy`, `ReflTyStruct`,
//! `ReflTyId`, and `ReflTyEnum` types the generated code refers to. Use it
//! through that crate.
//!
//! #### Describing a struct
//!
//! Derive [`ReflectTy`] on a struct with named fields to obtain a `refl_ty()`
//! that returns the struct's fields as `(name, type id)` pairs.
//!
//! ```ignore
//! use fruits_reflection::{ReflectTy, ReflTy, ReflTyStruct};
//!
//! #[derive(ReflectTy)]
//! struct Player {
//!     health: u8,
//!     name: String,
//! }
//!
//! // The derive implements `ReflectTy`, so the shape is available at runtime.
//! let ReflTy::Struct(ReflTyStruct::Named(fields)) = Player::refl_ty() else {
//!     unreachable!()
//! };
//! assert_eq!(fields[0].0, "health");
//! assert_eq!(fields[1].0, "name");
//! ```
//!
//! # How to maintain
//!
//! The single entry point is [`derive_reflect_ty`], the `ReflectTy` derive. It
//! does **not** use `quote!` to assemble its output: it writes the `impl` body
//! into a `String` with `write!` and then `parse()`s that string back into a
//! `TokenStream`. Every emitted fragment is therefore plain Rust source text,
//! which is why the generated code names the `fruits_reflection` types by bare
//! identifier and relies on them being in scope at the use site (this crate has
//! no dependency on `fruits_reflection`, so those names cannot be intra-doc
//! linked from here).
//!
//! Generics are reproduced verbatim: the full generic clause becomes the
//! `impl<..>` header, while type and const parameter idents are collected into
//! the `Type<..>` reference. A non-generic type produces an empty `Type<>`,
//! which is valid Rust. Lifetime parameters are rejected with a panic
//! (`"Non-static types are not supported"`), matching the `'static` bound on the
//! [`ReflectTy`] trait. Unions panic as well.
//!
//! The struct branch always emits a `ReflTyStruct::Named` list, pairing each
//! field's identifier with `ReflTyId::of::<FieldType>()`. It does not branch on
//! the field kind, so only structs with named fields are fully supported; tuple
//! and unit structs are not handled. The enum branch reproduces each variant
//! name and maps unit variants to `ReflTyStruct::Unit`; the named- and
//! tuple-variant arms are still incomplete and should be revisited before
//! relying on enums with fields. `ReflTyId::of` itself currently derives its id
//! from `std::any::type_name`, so type identity is only as stable as that name.

use proc_macro::TokenStream;
use quote::ToTokens;
use std::fmt::Write;
use syn::DeriveInput;

#[proc_macro_derive(ReflectTy)]
pub fn derive_reflect_ty(stream: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(stream as DeriveInput);

    let mut result = String::new();

    let impl_generics = input.generics.to_token_stream().to_string();
    let mut type_generics = String::new();

    for param in input.generics.params {
        match param {
            syn::GenericParam::Lifetime(_) => panic!("Non-static types are not supported"),
            syn::GenericParam::Type(type_param) => write!(type_generics, "{},", type_param.ident.to_string()).unwrap(),
            syn::GenericParam::Const(const_param) => write!(type_generics, "{},", const_param.ident.to_string()).unwrap(),
        }
    }

    let type_name = input.ident.to_string();

    write!(
        result,
        r#"impl{impl_generics} ReflectTy for {type_name}<{type_generics}> {{ "#
    )
    .unwrap();
    write!(result, r#"fn refl_ty() -> ReflTy {{ "#).unwrap();

    match input.data {
        syn::Data::Struct(data_struct) => {
            result.push_str("ReflTy::Struct(ReflTyStruct::Named(vec![");

            for field in data_struct.fields {
                let field_name = field.ident.to_token_stream().to_string();
                let field_type = field.ty.to_token_stream().to_string();

                write!(result, r#"(String::from("{field_name}"), ReflTyId::of::<{field_type}>()),"#).unwrap();
            }

            result.push_str(" ]))");
        }
        syn::Data::Enum(data_enum) => {
            result.push_str("ReflTy::Enum(ReflTyEnum { variants: vec![ ");

            for variant in data_enum.variants {
                let variant_name = variant.ident.to_string();

                write!(result, r#"(String::from("{variant_name}"), "#).unwrap();

                match variant.fields {
                    syn::Fields::Named(fields_named) => {
                        write!(result, r#"ReflTyStruct::Named(vec!["#).unwrap();

                        for field in fields_named.named {
                            let field_type = field.ty.to_token_stream().to_string();

                            write!(result, r#"ReflTyId::of::<{field_type}>(),"#).unwrap();
                        }

                        write!(result, r#"])"#).unwrap();
                    }
                    syn::Fields::Unnamed(fields_unnamed) => {
                        write!(result, r#"ReflTyStruct::Tuple(vec!["#).unwrap();

                        for field in fields_unnamed.unnamed {
                            let field_name = field.ident.to_token_stream().to_string();
                            let field_type = field.ty.to_token_stream().to_string();

                            write!(result, r#"(String::from("{field_name}"), ReflTyId::of::<{field_type}>()),"#).unwrap();
                        }

                        write!(result, r#"])"#).unwrap();
                    }
                    syn::Fields::Unit => write!(result, r#"ReflTyStruct::Unit"#).unwrap(),
                }

                write!(result, r#" ),"#).unwrap();
            }
        }
        syn::Data::Union(_) => panic!("Union types are not supported."),
    }

    result.push_str(" } ");
    result.push_str(" } ");

    result.parse().unwrap()
}
