use crate::ns::StructInfo;
use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;
use syn::Fields;
use syn::{DataStruct, DeriveInput};

fn impl_serialize(info: &StructInfo) -> TokenStream {
    let &StructInfo {
        ref field_idents,
        ref item_ident,
        ref renamed_item_ident_quoted,
        ref struct_body,
        ref struct_namespace_quoted,
        ..
    } = info;

    let field_idents2 = field_idents.iter();

    let body = if let Fields::Named(fields) = struct_body.clone().fields {
        fields.named.into_token_stream()
    } else {
        panic!("Unnamed and unit structs are not supported");
    };

    quote! {
        impl Serialize for #item_ident {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                #[derive(Serialize)]
                struct Helper<'a> {
                    event_type: &'a str,
                    event_namespace: &'a str,
                    #body
                }

                let out = Helper {
                    event_namespace: #struct_namespace_quoted,
                    event_type: #renamed_item_ident_quoted,
                    #(#field_idents: self.#field_idents2.clone(), )*
                };

                out.serialize(serializer).map_err(ser::Error::custom)
            }
        }
    }
}

fn impl_deserialize(info: &StructInfo) -> TokenStream {
    let &StructInfo {
        ref field_idents,
        ref item_ident,
        ref renamed_item_ident_quoted,
        ref struct_body,
        ref struct_namespace_quoted,
        ..
    } = info;

    let field_idents2 = field_idents.iter();

    let body = if let Fields::Named(fields) = struct_body.clone().fields {
        fields.named.into_token_stream()
    } else {
        panic!("Unnamed and unit structs are not supported");
    };

    quote! {
        impl<'de> Deserialize<'de> for #item_ident {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                use serde::de;

                #[derive(Deserialize, Clone)]
                struct Helper {
                    event_type: String,
                    event_namespace: String,
                    #body
                }

                let helper = Helper::deserialize(deserializer).map_err(de::Error::custom)?;

                if helper.event_namespace != #struct_namespace_quoted {
                    Err(de::Error::custom(format!("Incorrect event namespace {}, expected {}", helper.event_namespace, #struct_namespace_quoted)))
                } else if helper.event_type != #renamed_item_ident_quoted {
                    Err(de::Error::custom(format!("Incorrect event type {}, expected {}", helper.event_type, #renamed_item_ident_quoted)))
                } else {
                    Ok(#item_ident {
                        #(#field_idents: helper.#field_idents2,)*
                    })
                }
            }
        }
    }
}

pub fn derive_struct(parsed: &DeriveInput, struct_body: &DataStruct) -> TokenStream {
    let info = StructInfo::new(&parsed, &struct_body);

    let &StructInfo {
        ref struct_namespace_quoted,
        ref item_ident,
        ref renamed_namespace_and_type,
        ref renamed_item_ident_quoted,
        ..
    } = &info;

    let ser = impl_serialize(&info);
    let de = impl_deserialize(&info);

    let dummy_const = Ident::new(
        &format!("_IMPL_EVENT_STORE_STRUCT_FOR_{}", item_ident),
        Span::call_site(),
    );

    quote! {
        #[allow(non_upper_case_globals, unused_attributes, unused_imports)]
        const #dummy_const: () = {
            extern crate serde;
            extern crate serde_derive;
            extern crate event_store_derive_internals;

            use serde::ser;
            use serde::de::{Deserialize, Deserializer};
            use serde::ser::{Serialize, Serializer, SerializeMap};

            impl event_store_derive_internals::EventData for #item_ident {
                fn event_namespace_and_type() -> &'static str { #renamed_namespace_and_type }
                fn event_namespace() -> &'static str { #struct_namespace_quoted }
                fn event_type() -> &'static str { #renamed_item_ident_quoted }
            }

            #ser
            #de
        };
    }
}
