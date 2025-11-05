use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, parse_macro_input};

/// To be used on functions, so a function `func` will be turned into:
/// * `func` - the original item, unchanged
/// * `func_params(FuncParams)` taking a struct with all parameters
/// * `func_cmd` for calls from the frontend, taking `serde_json::Value` and returning `Result<serde_json::Value, Error>`
#[proc_macro_attribute]
pub fn api_cmd(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let sig = &input_fn.sig;
    let fn_name = &sig.ident;
    let output = &sig.output;

    // Collect parameter names and types
    let mut fields = Vec::new();
    let mut param_names = Vec::new();
    for arg in &sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let pat = &pat_type.pat;
            if let Pat::Ident(ident) = &**pat {
                let name = &ident.ident;
                fields.push(quote! { pub #name: #ty });
                param_names.push(name);
            }
        }
    }

    // Struct name: <FunctionName>Params (PascalCase)
    let struct_name = format_ident!("{}Params", fn_name.to_string().to_case(Case::Pascal));

    // Wrapper function name: <function_name>_params
    let wrapper_name = format_ident!("{}_params", fn_name);

    // Cmd function name: <function_name>_cmd
    let cmd_name = format_ident!("{}_cmd", fn_name);

    let expanded = quote! {
        // Original function stays
        #input_fn

        // Generated struct
        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct #struct_name {
            #(#fields,)*
        }

        // Wrapper function
        fn #wrapper_name(params: #struct_name) #output {
            #fn_name(#(params.#param_names),*)
        }

        // Cmd function
        #vis fn #cmd_name(
            params: ::serde_json::Value,
        ) -> Result<::serde_json::Value, crate::error::Error> {
            use crate::error::ToError;
            let params: #struct_name = ::serde_json::from_value(params).to_error()?;
            let result = #fn_name(#(params.#param_names),*)?;
            ::serde_json::to_value(result).to_error()
        }

    };

    expanded.into()
}
