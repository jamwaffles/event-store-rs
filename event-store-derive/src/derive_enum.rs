use ns::get_enum_struct_names;
use ns::get_namespaces;
use ns::get_quoted_namespaces;
use ns::EnumInfo;
use proc_macro2::{Ident, Span, TokenStream};
use std::iter::repeat;
use syn::{DataEnum, DeriveInput};

fn impl_serialize(info: &EnumInfo) -> TokenStream {
    let EnumInfo {
        item_ident,
        variant_idents,
        enum_body,
        enum_namespace,
        renamed_variant_idents,
        ..
    } = info;

    let item_idents = repeat(item_ident);

    let namespaces_quoted = get_quoted_namespaces(&enum_body, &enum_namespace);
    let namespaces = get_namespaces(&enum_body, &enum_namespace);

    let types_quoted = renamed_variant_idents.iter().map(|ident| ident.to_string());

    let struct_idents = get_enum_struct_names(&enum_body);

    let namespace_and_types_combined = namespaces
        .iter()
        .zip(renamed_variant_idents.iter())
        .map(|(ns, ty)| format!("{}.{}", ns, ty));

    quote! {
        impl Serialize for #item_ident {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                match self {
                    #(#item_idents::#variant_idents(evt) => {
                        #[derive(Serialize)]
                        struct Output<'a> {
                            #[serde(rename = "type")]
                            event_namespace_and_type: &'a str,
                            event_type: &'a str,
                            event_namespace: &'a str,
                            #[serde(flatten)]
                            payload: &'a #struct_idents,
                        }

                        let out = Output {
                            payload: evt,
                            event_namespace_and_type: #namespace_and_types_combined,
                            event_namespace: #namespaces_quoted,
                            event_type: #types_quoted
                        };

                        out.serialize(serializer).map_err(ser::Error::custom)
                    },)*
                }
            }
        }
    }
}

fn impl_deserialize(info: &EnumInfo) -> TokenStream {
    let EnumInfo {
        enum_body,
        item_ident,
        variant_idents,
        renamed_variant_idents,
        enum_namespace,
        ..
    } = info;

    let variant_idents2 = variant_idents.iter();

    let renamed_variant_idents2 = renamed_variant_idents.iter();
    let renamed_variant_idents3 = renamed_variant_idents.iter();
    let renamed_variant_idents4 = renamed_variant_idents.iter();

    let struct_idents = get_enum_struct_names(&enum_body);
    let struct_idents2 = struct_idents.clone();
    let item_idents = repeat(&info.item_ident);
    let item_idents2 = repeat(&info.item_ident);
    let variant_namespaces_quoted = get_quoted_namespaces(&enum_body, &enum_namespace);

    let namespaces = get_namespaces(&enum_body, &enum_namespace);

    let namespace_and_types_combined = namespaces
        .iter()
        .zip(renamed_variant_idents.iter())
        .map(|(ns, ty)| format!("{}.{}", ns, ty));

    quote! {
        impl<'de> Deserialize<'de> for #item_ident {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                use serde::de;

                #[derive(Deserialize)]
                #[serde(tag = "type")]
                enum OldOutput {
                    #(
                        #[serde(rename = #namespace_and_types_combined)]
                        #renamed_variant_idents(#struct_idents),
                    )*
                }

                #[derive(Deserialize)]
                #[serde(tag = "event_type")]
                enum Output {
                    #(#renamed_variant_idents2(#struct_idents2),)*
                }

                // TODO: Add underscores or something to meta fields to scronch conflicts
                #[derive(Deserialize)]
                struct Helper {
                    event_namespace: Option<String>,
                    #[serde(flatten)]
                    payload: Option<Output>,
                    #[serde(flatten)]
                    old_payload: Option<OldOutput>,
                }

                let type_helper = Helper::deserialize(deserializer).map_err(de::Error::custom)?;

                if let Some(ns) = type_helper.event_namespace {
                    match (ns.as_str(), type_helper.payload) {
                        #((#variant_namespaces_quoted, Some(Output::#renamed_variant_idents3(evt))) => {
                            Ok(#item_idents::#variant_idents(evt))
                        },)*
                        _ => Err(de::Error::custom("Could not find matching variant using 'event_type' and 'event_namespace' fields"))
                    }
                } else {
                    match type_helper.old_payload {
                        #(Some(OldOutput::#renamed_variant_idents4(evt)) => {
                            Ok(#item_idents2::#variant_idents2(evt))
                        },)*
                        _ => Err(de::Error::custom("Could not find matching variant using 'type' field"))
                    }
                }
            }
        }
    }
}

pub fn derive_enum(parsed: &DeriveInput, enum_body: &DataEnum) -> TokenStream {
    let info = EnumInfo::new(&parsed, &enum_body);
    let &EnumInfo {
        ref enum_namespace,
        ref enum_body,
        ref item_ident,
        ref renamed_variant_idents,
        ref variant_idents,
        ..
    } = &info;

    let item_idents = repeat(item_ident);
    let item_idents2 = repeat(item_ident);
    let item_idents3 = repeat(item_ident);
    let variant_idents2 = variant_idents.iter();
    let variant_idents3 = variant_idents.iter();

    let namespaces_quoted = get_quoted_namespaces(&enum_body, &enum_namespace);

    let types_quoted = renamed_variant_idents.iter().map(|ident| ident.to_string());

    let namespace_and_types_quoted = namespaces_quoted
        .iter()
        .zip(renamed_variant_idents.iter())
        .map(|(ns, ty)| format!("{}.{}", ns, ty))
        .collect::<Vec<String>>();

    let ser = impl_serialize(&info);
    let de = impl_deserialize(&info);

    let struct_idents = get_enum_struct_names(&enum_body);

    let namespace_and_types_quoted_clone = namespace_and_types_quoted.clone();
    let namespaces_quoted_clone = namespaces_quoted.clone();
    let types_quoted_clone = types_quoted.clone();

    let dummy_const = Ident::new(
        &format!("_IMPL_EVENT_STORE_ENUM_FOR_{}", item_ident),
        Span::call_site(),
    );

    quote! {
        #[allow(non_upper_case_globals, unused_attributes, unused_imports)]
        const #dummy_const: () = {
            extern crate serde;
            extern crate event_store_derive_internals;

            use serde::ser;
            use serde::de::{Deserialize, Deserializer, IntoDeserializer};
            use serde::ser::{Serialize, Serializer, SerializeMap};

            // Get the type or namespace of an instance of an events enum
            impl event_store_derive_internals::Events for #item_ident {
                fn event_namespace_and_type(&self) -> &'static str {
                    match self {
                        #(#item_idents::#variant_idents(_) => #namespace_and_types_quoted_clone,)*
                    }
                }
                fn event_namespace(&self) -> &'static str {
                    match self {
                        #(#item_idents2::#variant_idents2(_) => #namespaces_quoted_clone,)*
                    }
                }
                fn event_type(&self) -> &'static str {
                    match self {
                        #(#item_idents3::#variant_idents3(_) => #types_quoted_clone,)*
                    }
                }
            }

            #(
                impl event_store_derive_internals::EventData for #struct_idents {
                    fn event_namespace_and_type() -> &'static str { #namespace_and_types_quoted }
                    fn event_namespace() -> &'static str { #namespaces_quoted }
                    fn event_type() -> &'static str { #types_quoted }
                }
            )*
            #ser
            #de
        };
    }
}
