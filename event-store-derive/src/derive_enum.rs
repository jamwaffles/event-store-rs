use crate::ns::get_enum_struct_names;
use crate::ns::EnumInfo;
use proc_macro2::{Ident, Span, TokenStream};
use std::iter::repeat;
use syn::{DataEnum, DeriveInput};

fn impl_serialize(info: &EnumInfo) -> TokenStream {
    let EnumInfo {
        item_ident,
        variant_idents,
        generics,
        ..
    } = info;

    let item_idents = repeat(item_ident);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let impl_generics = if generics.lifetimes().count() == 0 {
        quote! { <'de> }
    } else {
        quote! { #impl_generics }
    };

    quote! {
        impl #impl_generics Serialize for #item_ident #ty_generics #where_clause {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                match self {
                    #(#item_idents::#variant_idents(evt) =>
                        evt.serialize(serializer).map_err(ser::Error::custom)
                    ,)*
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
        generics,
        ..
    } = info;

    let variant_idents2 = variant_idents.iter();
    let variant_idents3 = variant_idents.iter();

    let struct_idents = get_enum_struct_names(&enum_body);
    let item_idents = repeat(&info.item_ident);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let impl_generics = if generics.lifetimes().count() == 0 {
        quote! { <'de> }
    } else {
        quote! { #impl_generics }
    };

    quote! {
        impl #impl_generics serde::Deserialize #impl_generics for #item_ident #ty_generics #where_clause {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer #impl_generics,
            {
                use serde::de;

                #[derive(Deserialize)]
                #[serde(untagged)]
                enum Output #ty_generics #where_clause {
                    #(#variant_idents(#struct_idents),)*
                }

                let out = Output::deserialize(deserializer).map_err(de::Error::custom)?;

                let mapped = match out {
                    #(Output::#variant_idents2(evt) => #item_idents::#variant_idents3(evt),)*
                };

                Ok(mapped)
            }
        }
    }
}

pub fn derive_enum(parsed: &DeriveInput, enum_body: &DataEnum) -> TokenStream {
    let info = EnumInfo::new(&parsed, &enum_body);
    let &EnumInfo { ref item_ident, .. } = &info;

    let ser = impl_serialize(&info);
    let de = impl_deserialize(&info);

    let dummy_const = Ident::new(
        &format!("_IMPL_EVENT_STORE_ENUM_FOR_{}", item_ident),
        Span::call_site(),
    );

    let (impl_generics, ty_generics, _where_clause) = info.generics.split_for_impl();

    quote! {
        #[allow(non_upper_case_globals, unused_attributes, unused_imports)]
        const #dummy_const: () = {
            extern crate serde;
            extern crate event_store_derive_internals;

            use serde::ser;
            use serde::de::{Deserialize, Deserializer, IntoDeserializer};
            use serde::ser::{Serialize, Serializer, SerializeMap};

            impl #impl_generics event_store_derive_internals::Events for #item_ident #ty_generics {}

            #ser
            #de
        };
    }
}
