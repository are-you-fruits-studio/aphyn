//! # aphyn_reflection_macros
//!
//! Provides `#[derive(Reflectable)]`, which writes the `Reflectable` impl of a struct or enum
//! so the type can be registered in a reflection registry without hand-written builder code.
//!
//! This crate is the procedural-macro half of `aphyn_reflection`; the derive is re-exported
//! from there, so users derive it through `aphyn_reflection` (or the `aphyn` facade) rather
//! than depending on this crate directly.
//!
//! # How to use
//!
//! The generated code names `Reflectable`, `Reflector`, `StructReflectorBuilder` and
//! `EnumReflectorBuilder` unqualified, so they must be in scope at the derive site —
//! `use aphyn_reflection::*;` brings them in. Usage and registration are covered in
//! `aphyn_reflection`.
//!
//! ```ignore
//! use aphyn_reflection::*;
//!
//! #[derive(Reflectable)]
//! struct Player {
//!     health: u32,
//!     position: (f32, f32),
//! }
//!
//! #[derive(Reflectable)]
//! enum Shape {
//!     Empty,
//!     Circle(f32),
//!     Rect { w: f32, h: f32 },
//! }
//! ```
//!
//! # How to maintain
//!
//! [`derive_reflectable`] emits `impl Reflectable for T { fn reflector() -> Reflector { … } }`
//! with `quote!`. Every type parameter gets a `'static` bound (the trait requires `'static`);
//! lifetime parameters and unions are rejected with a panic, which surfaces as a compile error
//! at the derive site. The type's own `where` clause is kept.
//!
//! - Structs become a `StructReflectorBuilder::<Self>` chain with one `.field(name, getter,
//!   getter_mut)` per field. Getters are non-capturing closures (`|v| Some(&v.name)`) that
//!   coerce to the builder's `fn` pointer parameters. Tuple fields are named by index (`"0"`,
//!   `"1"`, …); unit structs have no fields.
//! - Enums become an `EnumReflectorBuilder::<Self>` chain with one `.variant(name, is_active,
//!   fields)` per variant. `is_active` is `matches!(v, Self::Name { .. })`, which works for unit,
//!   tuple and struct variants alike. Each variant field getter `match`es the variant and binds
//!   the field (by name, or by position padded with `_` for tuple variants), returning `None`
//!   for other variants. `reflector()` carries `#[allow(unreachable_patterns)]` because the
//!   fallback `_ => None` arm is unreachable for single-variant enums.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericParam, Index, parse_quote};

#[proc_macro_derive(Reflectable)]
pub fn derive_reflectable(stream: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(stream as DeriveInput);

    let mut static_bounds = Vec::new();
    for param in &input.generics.params {
        match param {
            GenericParam::Lifetime(_) => panic!("Reflectable requires 'static types; lifetime parameters are not supported."),
            GenericParam::Type(param) => static_bounds.push(param.ident.clone()),
            GenericParam::Const(_) => {}
        }
    }
    let where_clause = input.generics.make_where_clause();
    for ident in static_bounds {
        where_clause.predicates.push(parse_quote!(#ident: 'static));
    }

    let body = match &input.data {
        Data::Struct(data) => struct_body(&data.fields),
        Data::Enum(data) => {
            let variants = data.variants.iter().map(|variant| {
                let ident = &variant.ident;
                let name = ident.to_string();
                let fields = variant_fields(ident, &variant.fields);

                quote! {
                    .variant(#name, |v| matches!(v, Self::#ident { .. }), |b| b #(#fields)*)
                }
            });

            quote! {
                EnumReflectorBuilder::<Self>::new() #(#variants)* .build()
            }
        }
        Data::Union(_) => panic!("Union types are not supported."),
    };

    let type_name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics Reflectable for #type_name #type_generics #where_clause {
            #[allow(unreachable_patterns)]
            fn reflector() -> Reflector {
                #body
            }
        }
    }
    .into()
}

fn struct_body(fields: &Fields) -> TokenStream2 {
    let fields = fields.iter().enumerate().map(|(i, field)| {
        let (name, access) = match &field.ident {
            Some(ident) => (ident.to_string(), quote!(#ident)),
            None => {
                let index = Index::from(i);
                (i.to_string(), quote!(#index))
            }
        };

        quote! {
            .field(#name, |v| Some(&v.#access), |v| Some(&mut v.#access))
        }
    });

    quote! {
        StructReflectorBuilder::<Self>::new() #(#fields)* .build()
    }
}

fn variant_fields(variant: &syn::Ident, fields: &Fields) -> Vec<TokenStream2> {
    let binding = format_ident!("field");

    fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let (name, pattern) = match &field.ident {
                Some(ident) => (ident.to_string(), quote!(Self::#variant { #ident: #binding, .. })),
                None => {
                    let skipped = (0..i).map(|_| quote!(_));
                    (i.to_string(), quote!(Self::#variant(#(#skipped,)* #binding, ..)))
                }
            };

            quote! {
                .field(
                    #name,
                    |v| match v { #pattern => Some(#binding), _ => None },
                    |v| match v { #pattern => Some(#binding), _ => None },
                )
            }
        })
        .collect()
}
