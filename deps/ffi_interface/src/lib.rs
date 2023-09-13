extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Error, ItemFn, Visibility};

#[proc_macro_attribute]
pub fn c_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(Span::call_site(), "expect an empty attribute")
            .to_compile_error()
            .into();
    }

    let func = syn::parse_macro_input!(item as ItemFn);
    match func.vis {
        Visibility::Public(_) => {}
        _ => {
            return Error::new(Span::call_site(), "`must decorate a public function")
                .to_compile_error()
                .into();
        }
    }
    let func_block = &func.block; // { some statement or expression here }

    let func_decl = func.sig;

    quote! {
        #[no_mangle]
        #[inline(never)]
        #[forbid(elided_lifetimes_in_paths)]
        #[forbid(improper_ctypes_definitions)]
        pub extern "C" #func_decl {
            #func_block
        }
    }
    .into()
}
