use crate::PROC_MACRO_NAME;
use proc_macro2::{Ident, Span, TokenStream};
use std::collections::HashMap;

use quote::{quote, ToTokens};

use syn::{DataEnum, DeriveInput, Lit, Meta, NestedMeta, Variant};

/// Attributes taken from `#[derive()]` statement on an enum variant
#[derive(Default, Debug)]
struct VariantEventStoreAttributes {
    // TODO: Support overrides in enum derive attr
    event_namespace: Option<String>,
    event_type: Option<String>,
    // TODO: Support overrides in enum derive attr
    entity_type: Option<String>,
}

/// Get attributes as a nice struct from something like
// `#[event_store(event_namespace = "store", event_type = "ThingCreated", entity_type = "thing")]`
fn get_variant_event_attributes(variant: &Variant) -> Result<VariantEventStoreAttributes, String> {
    let ident_match = Ident::new(PROC_MACRO_NAME, Span::call_site());

    // TODO: Validate there's only one event_store attr
    variant
        .attrs
        .iter()
        // Find only attributes called `event_store`
        .find(|attr| attr.path.is_ident(ident_match.clone()))
        .ok_or(format!(
            "Failed to find attribute {} for variant {}",
            PROC_MACRO_NAME,
            variant.ident.to_string()
        ))
        // Parse metadata
        .and_then(|event_store_attr| event_store_attr.parse_meta().map_err(|e| e.to_string()))
        // Get list of meta key/value paris
        .and_then(|meta| match meta {
            // Metadata must be a [list](https://docs.rs/syn/0.15.34/syn/enum.Meta.html#list)
            Meta::List(meta_key_values) => {
                meta_key_values
                    .nested
                    .iter()
                    .map(|item| match item {
                        // Metadata item in this list must be a `name = "value"` pair
                        NestedMeta::Meta(Meta::NameValue(name_value)) => {
                            let name = name_value.ident.to_string();

                            // The value of this pair must be a string, as that's all that is
                            // supported by event_store_derive right now.
                            match &name_value.lit {
                                Lit::Str(lit) => Ok((name, lit.value().clone())),
                                _ => Err(format!("Value for property {} must be a string", name)),
                            }
                        }
                        _ => Err(format!(
                            "Attribute properties must be a list of key/value pairs"
                        )),
                    })
                    .collect::<Result<HashMap<String, String>, String>>()
                    .map(|mut keys_values| VariantEventStoreAttributes {
                        event_namespace: keys_values.remove(&String::from("event_namespace")),
                        event_type: keys_values.remove(&String::from("event_type")),
                        entity_type: keys_values.remove(&String::from("entity_type")),
                    })
            }
            _ => Err(format!(
                "Metadata must be a list like 'event_namespace = \"foo_bar\"'"
            )),
        })
}

struct VariantExt<'a> {
    variant: &'a Variant,
    event_store_attributes: VariantEventStoreAttributes,
}

pub fn derive_enum(parsed: &DeriveInput, enum_body: &DataEnum) -> TokenStream {
    // let info = EnumInfo::new(&parsed, &enum_body);
    // let &EnumInfo { ref item_ident, .. } = &info;

    // let ser = impl_serialize(&info);
    // let de = impl_deserialize(&info);

    let item_ident = parsed.clone().ident.into_token_stream();

    let dummy_const = Ident::new(
        &format!("_IMPL_EVENT_STORE_ENUM_FOR_{}", item_ident),
        Span::call_site(),
    );

    // let (impl_generics, ty_generics, _where_clause) = info.generics.split_for_impl();

    let variant_attributes = enum_body
        .variants
        .iter()
        .map(|variant| {
            get_variant_event_attributes(variant).map(|event_store_attributes| VariantExt {
                variant,
                event_store_attributes,
            })
        })
        .collect::<Result<Vec<VariantExt>, String>>()
        .expect("All enum variants must have an #[event_store(...)] attribute");

    quote! {
        // #[allow(non_upper_case_globals, unused_attributes, unused_imports)]
        const #dummy_const: () = {
            // extern crate serde;
            // extern crate event_store_derive_internals;

            // use serde::ser;
            // use serde::de::{Deserialize, Deserializer, IntoDeserializer};
            // use serde::ser::{Serialize, Serializer, SerializeMap};

            // impl #impl_generics event_store_derive_internals::Events for #item_ident #ty_generics {}

            // #ser
            // #de
        };
    }
}
