use proc_macro::TokenStream;
use quote::ToTokens;
use std::{collections::HashSet, fmt::Write};
use syn::DeriveInput;

pub fn derive(stream: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(stream as DeriveInput);

    let mut result = String::new();

    let impl_generics = input.generics.to_token_stream().to_string();
    let mut type_generics = String::new();

    for param in &input.generics.params {
        match param {
            syn::GenericParam::Lifetime(lifetime_param) => {
                write!(type_generics, "{},", lifetime_param.lifetime.to_string()).unwrap()
            }
            syn::GenericParam::Type(type_param) => write!(type_generics, "{},", type_param.ident.to_string()).unwrap(),
            syn::GenericParam::Const(const_param) => write!(type_generics, "{},", const_param.ident.to_string()).unwrap(),
        }
    }

    let mut field_types = Vec::new();

    match &input.data {
        syn::Data::Struct(data_struct) => {
            for field in &data_struct.fields {
                field_types.push(field.ty.to_token_stream().to_string());
            }
        },
        syn::Data::Enum(data_enum) => {
            for variant in &data_enum.variants {
                for field in &variant.fields {
                    field_types.push(field.ty.to_token_stream().to_string());
                }
            }
        },
        syn::Data::Union(data_union) => todo!(),
    }

    let serializer_ctx_state_ty = "SerializerCtxStateTy";

    let mut serializer_ctx_state_ty_constraint = String::from("Copy");

    for field_ty in field_types.iter().collect::<HashSet<_>>() {
        serializer_ctx_state_ty_constraint.push_str(&format!(" + SerializerCtxState<{field_ty}>"));
    }

    let mut generic_params = input.generics.params.iter().map(|p| p.to_token_stream().to_string()).collect::<Vec<_>>();
    generic_params.push(format!("{serializer_ctx_state_ty}: {serializer_ctx_state_ty_constraint}"));
    let generic_params = generic_params.join(", ");

    let type_name = input.ident.to_string();

    write!(
        result,
        r#"impl<{generic_params}> Serializable<{serializer_ctx_state_ty}> for {type_name}<{type_generics}> where Self: 'static + Default "#
    )
    .unwrap();

    let mut where_clause_text = String::new();

    if let Some(where_clause) = &input.generics.where_clause {
        where_clause_text.push_str(", ");
        where_clause_text.push_str(&where_clause.predicates.to_token_stream().to_string());
        result.push_str(&where_clause_text);
    }

    result.push_str(" { ");

    result.push_str(&serialize_impl(&input, serializer_ctx_state_ty));
    result.push_str(&deserialize_impl(&input, serializer_ctx_state_ty));
    result.push_str(" } ");

    match result.parse() {
        Ok(res) => return res,
        Err(original_err) => {
            match format!(r###"const fail: &str = r##"{result}"##;"###).parse() {
                Ok(res) => return res,
                Err(_) => panic!("{}", original_err),
            }
        }
    }
}

fn serialize_impl(input: &DeriveInput, serializer_ctx_state_ty: &str) -> String {
    let mut impl_serialize = String::new();

    write!(
        impl_serialize,
        "fn serialize(&self, mut ctx: SerializerCtx<{serializer_ctx_state_ty}>, path: &str) -> SerializedValue {{ "
    )
    .unwrap();

    match &input.data {
        syn::Data::Struct(data_struct) => {
            match &data_struct.fields {
                syn::Fields::Named(fields_named) => {
                    impl_serialize.push_str("ctx.serialize_map(path)");

                    for field in &fields_named.named {
                        let field = field.ident.as_ref().unwrap().to_token_stream().to_string();

                        write!(
                            impl_serialize,
                            r#".with_field("{field}", &self.{field})"#
                        )
                        .unwrap();
                    }
                   
                    impl_serialize.push_str(".finish_as_map(true)");
                },
                syn::Fields::Unnamed(fields_unnamed) => {
                    impl_serialize.push_str("ctx.serialize_map(path)");

                    for field in 0..fields_unnamed.unnamed.len() {
                        write!(
                            impl_serialize,
                            r#".with_field("{field}", &self.{field})"#
                        )
                        .unwrap();
                    }
                   
                    impl_serialize.push_str(".finish_as_map(true)");
                },
                syn::Fields::Unit => {
                    impl_serialize.push_str("ctx.serialize_map(path).finish_as_map(true)");
                },
            }
        },
        syn::Data::Enum(data_enum) => {
            let mut variants_line = String::new();

            variants_line.push_str("let variants = [");

            for (i, variant) in data_enum.variants.iter().enumerate() {
                if i > 0 {
                    variants_line.push_str(", ");
                }

                variants_line.push('"');
                variants_line.push_str(&variant.ident.to_token_stream().to_string());
                variants_line.push('"');
            }

            variants_line.push_str("].into_iter().map(|s| s.into()).collect();");

            //

            impl_serialize.push_str(&variants_line);

            impl_serialize.push_str("match self {");

            for variant in &data_enum.variants {
                let variant_name = variant.ident.to_token_stream().to_string();

                match &variant.fields {
                    syn::Fields::Named(fields_named) => {
                        let fields_deconstruct = fields_named.named.iter()
                            .map(|f| f.ident.to_token_stream().to_string())
                            .map(|f| format!("{f}: f_{f}"))
                            .collect::<Vec<_>>()
                            .join(", ");

                        write!(
                            impl_serialize,
                            r#"Self::{variant_name} {{ {fields_deconstruct} }} => ctx.serialize_map(path)"#
                        )
                        .unwrap();

                        for field in &fields_named.named {
                            let field = field.ident.to_token_stream().to_string();

                            write!(
                                impl_serialize,
                                r#".with_field("{field}", f_{field})"#
                            )
                            .unwrap();
                        }
                       
                        write!(
                            impl_serialize,
                            r#".finish_as_enum(true, "{variant_name}", variants),"#
                        )
                        .unwrap();
                    },
                    syn::Fields::Unnamed(fields_unnamed) => {
                        let fields_deconstruct = (0..fields_unnamed.unnamed.len())
                            .map(|f| format!("f_{f}"))
                            .collect::<Vec<_>>()
                            .join(", ");

                        write!(
                            impl_serialize,
                            r#"Self::{variant_name}({fields_deconstruct}) => ctx.serialize_map(path)"#
                        )
                        .unwrap();

                        for field in 0..fields_unnamed.unnamed.len() {
                            write!(
                                impl_serialize,
                                r#".with_field("{field}", f_{field})"#
                            )
                            .unwrap();
                        }
                       
                        write!(
                            impl_serialize,
                            r#".finish_as_enum(true, "{variant_name}", variants),"#
                        )
                        .unwrap();
                    },
                    syn::Fields::Unit => {
                        write!(
                            impl_serialize,
                            r#"Self::{variant_name} => ctx.serialize_map(path).finish_as_enum(true, "{variant_name}", variants),"#
                        )
                        .unwrap();
                    },
                }
            }

            impl_serialize.push_str("}");
        },
        syn::Data::Union(_) => panic!("Union types are not supported."),
    }
   
    impl_serialize.push_str(" } ");

    impl_serialize
}

fn deserialize_impl(input: &DeriveInput, serializer_ctx_state_ty: &str) -> String {
    let mut impl_deserialize = String::new();

    write!(
        impl_deserialize,
        "fn deserialize(&mut self, mut ctx: SerializerCtx<{serializer_ctx_state_ty}>, path: &str, value: &SerializedValue) {{ "
    )
    .unwrap();

    match &input.data {
        syn::Data::Struct(data_struct) => {

            match &data_struct.fields {
                syn::Fields::Named(fields_named) => {
                    impl_deserialize.push_str("ctx.deserialize_map(self, path, value)");

                    for field in &fields_named.named {
                        let field = field.ident.as_ref().unwrap().to_token_stream().to_string();

                        write!(
                            impl_deserialize,
                            r#".with_field("{field}", &mut self.{field})"#
                        )
                        .unwrap();
                    }
                   
                    impl_deserialize.push_str(";");
                },
                syn::Fields::Unnamed(fields_unnamed) => {
                    impl_deserialize.push_str("ctx.deserialize_map(self, path, value)");

                    for field in 0..fields_unnamed.unnamed.len() {
                        write!(
                            impl_deserialize,
                            r#".with_field("{field}", &mut self.{field})"#
                        )
                        .unwrap();
                    }
                   
                    impl_deserialize.push_str(";");
                },
                syn::Fields::Unit => {
                    impl_deserialize.push_str("ctx.deserialize_map(self, path, value);");
                },
            }
        },
        syn::Data::Enum(data_enum) => {
            impl_deserialize.push_str("ctx.deserialize_enum(path, value)");

            for variant in &data_enum.variants {
                let variant_name = variant.ident.to_string();

                write!(
                    impl_deserialize,
                    r#".with_variant("{variant_name}", || Self::{variant_name}"#
                )
                .unwrap();

                match &variant.fields {
                    syn::Fields::Named(fields_named) => {
                        impl_deserialize.push_str(" { ");

                        for field in &fields_named.named {
                            let field = field.ident.as_ref().unwrap().to_token_stream().to_string();

                            write!(
                                impl_deserialize,
                                r#"{field}: Default::default(),"#
                            )
                            .unwrap();
                        }

                        impl_deserialize.push_str(" } ");
                    },
                    syn::Fields::Unnamed(fields_unnamed) => {
                        impl_deserialize.push_str(" ( ");

                        for field in 0..fields_unnamed.unnamed.len() {
                            write!(
                                impl_deserialize,
                                r#"Default::default(),"#
                            )
                            .unwrap();
                        }
                   
                        impl_deserialize.push_str(")");
                    },
                    syn::Fields::Unit => {},
                }

                impl_deserialize.push_str(")");
            }
           
            impl_deserialize.push_str(".finish(self, |this, ctx| { match this {");
            
            for variant in &data_enum.variants {
                let variant_name = variant.ident.to_string();

                let mut field_names = Vec::new();

                match &variant.fields {
                    syn::Fields::Named(fields_named) => {
                        for field in &fields_named.named {
                            let field = field.ident.as_ref().unwrap().to_token_stream().to_string();

                            field_names.push(field);
                        }
                    },
                    syn::Fields::Unnamed(fields_unnamed) => {
                        for field in 0..fields_unnamed.unnamed.len() {
                            field_names.push(field.to_string());
                        }
                    },
                    syn::Fields::Unit => {},
                }

                write!(
                    impl_deserialize,
                    r#"Self::{variant_name} {{"#
                )
                .unwrap();

                for field_name in &field_names {
                    write!(
                        impl_deserialize,
                        r#"{field_name}: f_{field_name},"#
                    )
                    .unwrap();
                }

                write!(
                    impl_deserialize,
                    r#"}} => _ = ctx"#
                )
                .unwrap();

                for field_name in &field_names {
                    write!(
                        impl_deserialize,
                        r#".with_field("{field_name}", f_{field_name})"#
                    )
                    .unwrap();
                }

                impl_deserialize.push_str(",");
            }

            impl_deserialize.push_str("}})");
        },
        syn::Data::Union(_) => panic!("Union types are not supported."),
    }

    impl_deserialize.push_str(" } ");

    impl_deserialize
}
