use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, FnArg, ItemFn, Pat, parse_macro_input};

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
        ) -> anyhow::Result<::serde_json::Value> {
            use crate::json::ToJsonError;
            let params: #struct_name = ::serde_json::from_value(params).to_json_error()?;
            let value = #fn_name(#(params.#param_names),*)?;
            Ok(::serde_json::to_value(value)?)
        }

    };

    expanded.into()
}

/// To be used on functions, so a function `func` will be turned into:
/// * `func` - the original item, unchanged
/// * `func_params(FuncParams)` taking a struct with all parameters
/// * `func_cmd` for calls from the frontend, taking `serde_json::Value` and returning `Result<serde_json::Value, Error>`
/// * `func_tauri` for calls from the tauri, args and returning `Result<serde_json::Value, Error>`, with `tauri` support.
#[proc_macro_attribute]
pub fn api_cmd_tauri(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let sig = &input_fn.sig;
    let fn_name = &sig.ident;
    let asyncness = &sig.asyncness;
    let input = &sig.inputs;
    let output = &sig.output;

    enum Conv {
        From,
        TryFrom,
    }

    let path = if attr.is_empty() {
        None
    } else {
        let meta = syn::parse_macro_input!(attr as syn::Meta);
        match meta {
            syn::Meta::Path(path) => {
                // #[api_cmd_tauri(Foo)]
                Some((Conv::From, path))
            }
            syn::Meta::NameValue(nv) => {
                if let (Some(ident), Expr::Path(path)) = (&nv.path.get_ident(), &nv.value) {
                    if *ident == "try_from" {
                        // #[api_cmd_tauri(try_from = Foo)]
                        Some((Conv::TryFrom, path.path.clone()))
                    } else {
                        return syn::Error::new_spanned(
                            ident,
                            "Need `try_from = path` to use try-from instead of from",
                        )
                        .to_compile_error()
                        .into();
                    }
                } else {
                    return syn::Error::new_spanned(
                        nv,
                        "Need `try_from = path` to use try-from instead of from",
                    )
                    .to_compile_error()
                    .into();
                }
            }
            syn::Meta::List(list) => {
                // #[api_cmd_tauri(key, other, try_from = Foo)]
                panic!("Currently unsupported: {list:?}")
            }
        }
    };

    /// Detect `Result<Option<` type
    fn is_result_option(ty: &syn::Type) -> bool {
        if let syn::Type::Path(tp) = ty
            && let Some(seg) = tp.path.segments.last()
            && seg.ident == "Result"
            && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
            && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
            && let syn::Type::Path(tp) = inner
            && let Some(first) = tp.path.segments.last()
            && first.ident == "Option"
        {
            true
        } else {
            false
        }
    }

    let outputs_result_option = path.map(|(conv, p)| {
        (
            conv,
            p,
            is_result_option(match &sig.output {
                syn::ReturnType::Type(_, ty) => ty.as_ref(),
                syn::ReturnType::Default => panic!("function must return a type"),
            }),
        )
    });

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

    let arg_idents: Vec<_> = input
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_ty) => Some(&pat_ty.pat),
            syn::FnArg::Receiver(_) => None, // for &self / self
        })
        .collect();

    let call_fn = if asyncness.is_some() {
        quote! { #fn_name(#(params.#param_names),*).await }
    } else {
        quote! { #fn_name(#(params.#param_names),*) }
    };

    let call_fn_args = if asyncness.is_some() {
        quote! { #fn_name(#(#arg_idents),*).await }
    } else {
        quote! { #fn_name(#(#arg_idents),*) }
    };

    // Struct name: <FunctionName>Params (PascalCase)
    let struct_name = format_ident!("{}Params", fn_name.to_string().to_case(Case::Pascal));

    // Wrapper function name: <function_name>_params
    let wrapper_name = format_ident!("{}_params", fn_name);

    // Cmd function name: <function_name>_cmd
    let cmd_name = format_ident!("{}_cmd", fn_name);

    // Cmd function name: <function_name>_json
    let json_name = format_ident!("{}_json", fn_name);

    // Module name for tauri-renames, to keep the original function names.
    let tauri_mod_name = format_ident!("tauri_{}", fn_name);
    let tauri_cmd_name = format_ident!("__cmd__{}", json_name);
    let tauri_orig_cmd_name = format_ident!("__cmd__{}", fn_name);

    let convert_json = if let Some((conv, path, is_result_opt)) = outputs_result_option {
        match conv {
            Conv::From => {
                if is_result_opt {
                    quote! {
                        let result: Option<#path> = result.map(Into::into);
                    }
                } else {
                    quote! {
                        let result: #path = result.into();
                    }
                }
            }
            Conv::TryFrom => {
                if is_result_opt {
                    quote! {
                        let result: Option<#path> = result.map(TryInto::try_into).transpose()?;
                    }
                } else {
                    quote! {
                        let result: #path = result.try_into()?;
                    }
                }
            }
        }
    } else {
        quote! {}
    };

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
        #asyncness fn #wrapper_name(params: #struct_name) #output {
            #call_fn
        }

        // Cmd function
        #vis #asyncness fn #cmd_name(
            params: ::serde_json::Value,
        ) -> anyhow::Result<::serde_json::Value> {
            let params: #struct_name = ::serde_json::from_value(params)?;
            let result = #call_fn?;
            #convert_json
            Ok(::serde_json::to_value(result)?)
        }

        // tauri function
        #[cfg_attr(feature = "tauri", tauri::command(async))]
        #vis #asyncness fn #json_name(
            #input
        ) -> Result<::serde_json::Value, crate::json::Error> {
            use crate::json::ToJsonError;
            let result = #call_fn_args?;
            #convert_json
            ::serde_json::to_value(result).to_json_error()
        }

        #[cfg(feature = "tauri")]
        pub mod #tauri_mod_name {
            pub use super::#json_name as #fn_name;
            pub use super::#tauri_cmd_name as #tauri_orig_cmd_name;
        }

    };

    expanded.into()
}
