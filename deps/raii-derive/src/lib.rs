extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Data, DeriveInput, Error};

#[proc_macro_derive(RAII)]
pub fn derive_raii(input: TokenStream) -> TokenStream {
    let st = syn::parse_macro_input!(input as DeriveInput);
    if !matches!(st.data, Data::Struct(_)) {
        return Error::new(Span::call_site(), "RAII object can only be a struct")
            .to_compile_error()
            .into();
    }
    let struct_name_literal = &st.ident;
    let struct_generics = &st.generics;
    quote! {
        static_assertions::assert_not_impl_any!(#struct_name_literal: Clone, Copy);
        // static_assertions::assert_impl_any!(#struct_name_literal: Drop);
        impl<#struct_generics> raii::RaiiBound for #struct_name_literal<#struct_generics> {}
    }
    .into()
}
