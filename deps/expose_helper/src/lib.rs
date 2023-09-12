extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Error, ItemFn, Visibility};

#[proc_macro_attribute]
pub fn ffi_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(Span::call_site(), "expect an empty attribute: `#[ffi_interface]`")
            .to_compile_error()
            .into();
    }

    let func = syn::parse_macro_input!(item as ItemFn);
    match func.vis {
        Visibility::Public(_) => {}
        _ => {
            return Error::new(Span::call_site(), "`must decorate a public function: #[ffi_interface]`")
                .to_compile_error()
                .into();
        }
    }
    let func_block = &func.block; // { some statement or expression here }

    let func_decl = func.sig;

    quote! {
        #[no_mangle]
        #[inline(never)]
        pub extern "C" #func_decl {
            #func_block
        }
    }
    .into()
}
