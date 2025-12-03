use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, FnArg, ItemFn, Pat, parse_macro_input};

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

    let opts = if attr.is_empty() {
        Options::default()
    } else {
        let meta = syn::parse_macro_input!(attr as syn::Meta);
        match parse_attrs_to_options(meta, output) {
            Ok(opts) => opts,
            Err(err) => return err.into_compile_error().into(),
        }
    };

    // Collect parameter names and types
    let mut fields = Vec::new();
    let mut param_names = Vec::new();
    for arg in input {
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

    let convert_json = if let Some(ResultConversion {
        mode,
        is_result_option,
        json_ty,
    }) = opts.result_conversion
    {
        match mode {
            FromMode::From => {
                if is_result_option {
                    quote! {
                        let result: Option<#json_ty> = result.map(Into::into);
                    }
                } else {
                    quote! {
                        let result: #json_ty = result.into();
                    }
                }
            }
            FromMode::TryFrom => {
                if is_result_option {
                    quote! {
                        let result: Option<#json_ty> = result.map(TryInto::try_into).transpose()?;
                    }
                } else {
                    quote! {
                        let result: #json_ty = result.try_into()?;
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

/// How to convert a result value/outcome to its serialised version.
#[derive(Debug)]
enum FromMode {
    From,
    TryFrom,
}

#[derive(Default)]
struct Options {
    /// It's `None` if the result type converts to JSON naturally.
    /// Otherwise, we convert to it.
    result_conversion: Option<ResultConversion>,
}

struct ResultConversion {
    /// If the result type conversion is fallbile.
    mode: FromMode,
    /// If the function returns `Result<Option<T>>>`
    is_result_option: bool,
    /// The path to the type to convert *to* for json.
    json_ty: syn::Path,
}

fn parse_attrs_to_options(
    meta: syn::Meta,
    output: &syn::ReturnType,
) -> Result<Options, syn::Error> {
    let path = match meta {
        syn::Meta::Path(path) => {
            // #[api_cmd_tauri(Foo)]
            Some((FromMode::From, path))
        }
        syn::Meta::NameValue(nv) => {
            if let (Some(ident), Expr::Path(path)) = (&nv.path.get_ident(), &nv.value) {
                if *ident == "try_from" {
                    // #[api_cmd_tauri(try_from = Foo)]
                    Some((FromMode::TryFrom, path.path.clone()))
                } else {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "Need `try_from = path` to use try-from instead of from",
                    ));
                }
            } else {
                return Err(syn::Error::new_spanned(
                    nv,
                    "Need `try_from = path` to use try-from instead of from",
                ));
            }
        }
        syn::Meta::List(list) => {
            // #[api_cmd_tauri(key, other, try_from = Foo)]
            panic!("Currently unsupported: {list:?}")
        }
    };

    let is_result_option = is_result_option(match output {
        syn::ReturnType::Type(_, ty) => ty.as_ref(),
        syn::ReturnType::Default => panic!("function must return a type"),
    });

    let result_conversion = path.map(|(conv, p)| ResultConversion {
        mode: conv,
        is_result_option,
        json_ty: p,
    });
    Ok(Options { result_conversion })
}

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
