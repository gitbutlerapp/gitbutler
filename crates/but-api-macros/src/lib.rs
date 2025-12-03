use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
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

    let is_result_option = is_result_option(match output {
        syn::ReturnType::Type(_, ty) => ty.as_ref(),
        syn::ReturnType::Default => panic!("function must return a type"),
    });

    let opts = if attr.is_empty() {
        Options::default()
    } else {
        let meta = syn::parse_macro_input!(attr as syn::Meta);
        match parse_attrs_to_options(meta, is_result_option) {
            Ok(opts) => opts,
            Err(err) => return err.into_compile_error().into(),
        }
    };

    let json_ty_by_name = match build_json_type_mapping(input.iter()) {
        Ok(m) => m,
        Err(err) => return err.into_compile_error().into(),
    };

    // Collect parameter names and types
    let mut struct_fields_with_json_types = Vec::new();
    let mut param_field_names = Vec::new();
    for arg in input {
        if let FnArg::Typed(pat_ty) = arg {
            let pat = &pat_ty.pat;
            if let Pat::Ident(ident) = &**pat {
                let name = &ident.ident;
                let name_type_declaration = if let Some((json_type, _from_mode)) =
                    json_ty_by_name.get(&ident.ident.to_string())
                {
                    quote! { pub #name: #json_type }
                } else {
                    let ty = &pat_ty.ty;
                    quote! { pub #name: #ty }
                };
                struct_fields_with_json_types.push(name_type_declaration);
                param_field_names.push(name);
            }
        }
    }

    // JSON-typed input parameters for the json function
    let mut json_fn_input_params: Vec<FnArg> = Vec::new();
    // Each JSON parameter gets a conversion to turn it into our desired type.
    let mut param_conversions = Vec::new();
    // The names of all of our parameters for the purpose of calling the inner function.
    let mut call_arg_idents = Vec::new();
    for arg in input {
        match arg {
            FnArg::Typed(pat_ty) => {
                let pat = &pat_ty.pat;
                let Pat::Ident(ident) = &**pat else {
                    return syn::Error::new_spanned(pat_ty, "Cannot handle this identifier")
                        .to_compile_error()
                        .into();
                };

                let name = &ident.ident;
                let json_type_mapping = json_ty_by_name.get(&ident.ident.to_string());
                let (arg_ident, ty) = match &*pat_ty.ty {
                    syn::Type::Reference(r) if json_type_mapping.is_some() => {
                        // Only if a remapping happens we want to change the argument identifier to use
                        // when passing then converted arguments to the function, while always producing an owned
                        // version.
                        let and = &r.and_token;
                        let mutability = &r.mutability;
                        let arg_ident: syn::Type = syn::parse_quote! { #and #mutability #name };
                        (arg_ident, &*r.elem)
                    }
                    other => (syn::parse_quote! { #name }, other),
                };
                call_arg_idents.push(arg_ident);
                let param = if let Some((json_type, from_mode)) = json_type_mapping {
                    // We control these conversions, and must just make them work to keep this simple.
                    param_conversions.push(match from_mode {
                        FromMode::From => {
                            quote! {
                                let mut #name = <#ty>::from(#name);
                            }
                        }
                        FromMode::TryFrom => {
                            quote! {
                                let mut #name = <#ty>::try_from(#name)?;
                            }
                        }
                    });
                    syn::parse_quote! { #name: #json_type }
                } else {
                    arg.clone()
                };
                json_fn_input_params.push(param);
            }
            FnArg::Receiver(r) => {
                return syn::Error::new_spanned(r, "Cannot handle &self, &mut self or self")
                    .to_compile_error()
                    .into();
            }
        }
    }

    let call_fn_args = if asyncness.is_some() {
        quote! { #fn_name(#(#call_arg_idents),*).await }
    } else {
        quote! { #fn_name(#(#call_arg_idents),*) }
    };

    // Struct name: <FunctionName>Params (PascalCase)
    let param_struct_name = format_ident!("{}Params", fn_name.to_string().to_case(Case::Pascal));

    // Cmd function name: <function_name>_cmd
    let fn_cmd_name = format_ident!("{}_cmd", fn_name);

    // Cmd function name: <function_name>_json
    let fn_json_name = format_ident!("{}_json", fn_name);

    // Module name for tauri-renames, to keep the original function names.
    let tauri_mod_name = format_ident!("tauri_{}", fn_name);
    let tauri_cmd_name = format_ident!("__cmd__{}", fn_json_name);
    let tauri_orig_cmd_name = format_ident!("__cmd__{}", fn_name);

    let (convert_to_json_result_type, json_ty) = if let Some(ResultConversion {
        mode,
        is_result_option,
        json_ty,
        json_ty_rval,
    }) = opts.result_conversion
    {
        let convert = match mode {
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
        };
        (convert, json_ty_rval)
    } else {
        let return_type = match extract_ok_type(output) {
            Ok(ty_path) => ty_path,
            Err(err) => return err.to_compile_error().into(),
        };
        (quote! {}, return_type)
    };

    let legacy_cfg_if_json_mapping_is_used = if !json_ty_by_name.is_empty() {
        quote! { #[cfg(feature = "legacy")] }
    } else {
        quote! {}
    };

    let expanded = quote! {
        // Generated struct
        #[cfg(feature = "legacy")]
        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct #param_struct_name {
            #(#struct_fields_with_json_types,)*
        }

        // Original function stays
        #input_fn

        /// Cmd function - this is legacy just while most of its functionality depend on `LegacyProjectId`.
        /// parameter struct input via json value, json output.
        #[cfg(feature = "legacy")]
        #vis #asyncness fn #fn_cmd_name(
            params: ::serde_json::Value,
        ) -> anyhow::Result<::serde_json::Value> {
            let #param_struct_name { #(#param_field_names),* } = ::serde_json::from_value(params)?;
            #(#param_conversions);*
            let result = #call_fn_args?;
            #convert_to_json_result_type
            Ok(::serde_json::to_value(result)?)
        }

        /// tauri function - json input, json output, by #fn_name
        #[cfg_attr(feature = "tauri", tauri::command(async))]
        #legacy_cfg_if_json_mapping_is_used
        #vis #asyncness fn #fn_json_name(
            #(#json_fn_input_params),*
        ) -> Result<#json_ty, crate::json::Error> {
            #(#param_conversions);*
            let result = #call_fn_args?;
            #convert_to_json_result_type
            Ok(result)
        }

        /// A dummy module just to make generated tauri functions available *and* working.
        #[cfg(feature = "tauri")]
        pub mod #tauri_mod_name {
            pub use super::#fn_json_name as #fn_name;
            pub use super::#tauri_cmd_name as #tauri_orig_cmd_name;
        }

    };

    expanded.into()
}

/// The mapping is from type name to their respective json types. We assume they have a `NonJson::try_from(json)` implementation
/// to turn them into their respective non-json types.
fn build_json_type_mapping<'a>(
    input: impl IntoIterator<Item = &'a syn::FnArg>,
) -> Result<HashMap<String, (syn::Path, FromMode)>, syn::Error> {
    let mut out = HashMap::new();

    for arg in input {
        let syn::FnArg::Typed(pat_ty) = arg else {
            continue;
        };

        let pat = &pat_ty.pat;
        let ty = &pat_ty.ty;

        // We only accept patterns like `arg: &T` where `arg` is an ident.
        let Pat::Ident(pat_ident) = &**pat else {
            continue;
        };

        let (path, is_reference) = if let syn::Type::Reference(r) = &**ty {
            // Extract the referenced type
            let inner = &r.elem;

            // Expect something like &Context or &but_ctx::Context
            let syn::Type::Path(tp) = &**inner else {
                return Err(syn::Error::new_spanned(
                    inner,
                    "Expected a type path inside reference",
                ));
            };

            // Path segments for matching
            (&tp.path, true)
        } else if let syn::Type::Path(ty_path) = &**ty {
            (&ty_path.path, false)
        } else {
            continue;
        };

        let segments = &path.segments;
        if segments.is_empty() {
            return Err(syn::Error::new_spanned(
                ty,
                "Unexpected empty type path in reference",
            ));
        }

        let last = &segments.last().unwrap().ident;
        if last == "Context" && (segments.len() == 1 || segments[0].ident == "but_ctx") {
            // Map this parameter to ProjectId
            let project_id_path: syn::Path = syn::parse_str("but_ctx::LegacyProjectId").unwrap();
            out.insert(
                pat_ident.ident.to_string(),
                (project_id_path, FromMode::TryFrom),
            );
        } else if last == "ObjectId" && (segments.len() == 1 || segments[0].ident == "gix") {
            // Map this parameter to HexHash
            let project_id_path: syn::Path = syn::parse_str("crate::json::HexHash").unwrap();
            out.insert(
                pat_ident.ident.to_string(),
                (project_id_path, FromMode::From),
            );
        } else if is_reference {
            return Err(syn::Error::new_spanned(
                ty,
                "Only `&Context` or `&but_ctx::Context` may be references",
            ));
        }
    }

    Ok(out)
}

fn extract_ok_type(output: &syn::ReturnType) -> syn::Result<syn::Type> {
    let ty = match output {
        syn::ReturnType::Type(_, ty) => ty.as_ref(),
        _ => {
            return Err(syn::Error::new_spanned(
                output,
                "function must return a type",
            ));
        }
    };

    let syn::Type::Path(tp) = ty else {
        return Err(syn::Error::new_spanned(ty, "expected a type path"));
    };

    // Expect outer type: Result<...>
    let last = tp
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(tp, "unexpected empty type path"))?;

    if last.ident != "Result" {
        return Err(syn::Error::new_spanned(
            last,
            "expected Result<T> or Result<T, E>",
        ));
    }

    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return Err(syn::Error::new_spanned(
            last,
            "expected Result<T> or Result<T, E>",
        ));
    };

    if args.args.is_empty() {
        return Err(syn::Error::new_spanned(
            args,
            "Result must have at least one generic parameter",
        ));
    }

    // First generic argument is T in Result<T, E>
    match args.args.first().unwrap() {
        syn::GenericArgument::Type(t) => Ok(t.clone()),
        other => Err(syn::Error::new_spanned(
            other,
            "expected a type as first generic parameter",
        )),
    }
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
    /// The type to convert *to* for json.
    json_ty: syn::Type,
    /// The resulting JSON type after applying option wrapping.
    json_ty_rval: syn::Type,
}

fn parse_attrs_to_options(meta: syn::Meta, is_result_option: bool) -> Result<Options, syn::Error> {
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

    let result_conversion = path.map(|(conv, p)| {
        let base_ty = syn::Type::Path(syn::TypePath {
            qself: None,
            path: p.clone(),
        });

        let rval_ty = if is_result_option {
            syn::parse_quote! { Option<#base_ty> }
        } else {
            base_ty.clone()
        };

        ResultConversion {
            mode: conv,
            is_result_option,
            json_ty: base_ty,
            json_ty_rval: rval_ty,
        }
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
