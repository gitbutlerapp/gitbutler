use std::collections::HashMap;

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, FnArg, ItemFn, Pat, parse_macro_input};

/// A macro to help generate wrappers which are used by some clients to support deserialisation of parameters
/// for calls, and serialisation of return values, usually with JSON in mind.
///
/// # Parameters
///
/// This paragraph explains how the proc-macro can be called.
///
/// * `JSONReturnType`
///     - Use it like `but_api(JSONReturnType)` where `JSONReturnType::from(actual_return_type)` is implemented.
///     - Controls how the actual return value is converted for JSON serialization in `func_json` and `func_cmd`.
/// * `try_from = JSONReturnType`
///     - Use it like `but_api(try_from = JSONReturnType)` where `JSONReturnType::try_from(actual_return_type)?` is implemented.
///     - Controls how the actual return value is fallibly converted for JSON serialization in `func_json` and `func_cmd`.
///
/// # Generated Functions
///
/// This paragraph explains what it generates.
///
/// * `func` - the original item, unchanged
/// * `func_json` for calls from the frontend, taking `(#(json_params*),)` and returning `Result<JsonRVal, json::Error>`
///     - This is also annotated with the `tauri` macro when the feature is enabled in the `but-api` crate.
///     - **Parameter Transformation**
///         - It supports `but_ctx::Context`, `&Context`, `&mut Context` or `ThreadSafeContext` as parameter,
///           which will be translated to `LegacyProjectId` with the `project_id` parameter name.
///         - `gix::ObjectId` will be translated into `json::HexHash`.
/// * `func_cmd` for calls from the `but-server`, taking `(params: Params) ` and returning `Result<serde_json::Value, json::Error>`.
///     - It performs all **Parameter Transformations** of `func_json`.
/// * `func_napi` (opt-in) for calls from Node.js via napi-rs, taking individual typed parameters and returning `napi::Result<serde_json::Value>`.
///     - **Only generated when `napi` is specified**: `#[but_api(napi)]` or `#[but_api(napi, try_from = Foo)]`.
///     - Gated behind `#[cfg(feature = "napi")]`.
///     - **Parameter Transformation**
///         - `Context`/`&Context`/`&mut Context`/`ThreadSafeContext` → `String` named `project_id`
///         - `gix::ObjectId` / `HexHash` → `String` (hex-encoded)
///         - `BString` → `String`
///         - Other serde-compatible types → `serde_json::Value`
///     - Automatically converts `anyhow::Error` → `napi::Error`.
#[proc_macro_attribute]
pub fn but_api(attr: TokenStream, item: TokenStream) -> TokenStream {
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
                let (name, name_type_declaration) = if let Some(JsonParameterMapping {
                    json_ty,
                    json_ident,
                    from_mode: _,
                }) = json_ty_by_name.get(&ident.ident.to_string())
                {
                    let name = json_ident.as_ref().unwrap_or(name);
                    (name, quote! { pub #name: #json_ty })
                } else {
                    let ty = &pat_ty.ty;
                    (name, quote! { pub #name: #ty })
                };
                param_field_names.push(name);
                struct_fields_with_json_types.push(name_type_declaration);
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

                let ident = &ident.ident;
                let json_type_mapping = json_ty_by_name.get(&ident.to_string());
                let (arg_ident, ty) = match &*pat_ty.ty {
                    syn::Type::Reference(r) if json_type_mapping.is_some() => {
                        // Only if a remapping happens we want to change the argument identifier to use
                        // when passing then converted arguments to the function, while always producing an owned
                        // version.
                        let and = &r.and_token;
                        let mutability = &r.mutability;
                        let arg_ident: syn::Type = syn::parse_quote! { #and #mutability #ident };
                        (arg_ident, &*r.elem)
                    }
                    other => (syn::parse_quote! { #ident }, other),
                };
                call_arg_idents.push(arg_ident);
                let param = if let Some(JsonParameterMapping {
                    json_ty,
                    json_ident,
                    from_mode,
                }) = json_type_mapping
                {
                    // We control these conversions, and must just make them work to keep this simple.
                    let json_ident = json_ident.as_ref().unwrap_or(ident);
                    param_conversions.push(match from_mode {
                        FromMode::From => {
                            quote! {
                                let mut #ident = <#ty>::from(#json_ident);
                            }
                        }
                        FromMode::TryFrom => {
                            quote! {
                                let mut #ident = <#ty>::try_from(#json_ident)?;
                            }
                        }
                    });
                    syn::parse_quote! { #json_ident: #json_ty }
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
        quote! {{
            let __call_result =
                ::futures::FutureExt::catch_unwind(::std::panic::AssertUnwindSafe(async {
                    #fn_name(#(#call_arg_idents),*).await
                }))
                .await
                .map_err(|__panic_payload| {
                    crate::panic_capture::panic_payload_to_anyhow(stringify!(#fn_name), __panic_payload)
                })?;
            __call_result
        }}
    } else {
        quote! {{
            let __call_result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                #fn_name(#(#call_arg_idents),*)
            }))
            .map_err(|__panic_payload| {
                crate::panic_capture::panic_payload_to_anyhow(stringify!(#fn_name), __panic_payload)
            })?;
            __call_result
        }}
    };

    // Build napi-specific parameter list and conversions.
    // For napi, Context → String (project_id), ObjectId/HexHash → String,
    // BString → String, other serde types → serde_json::Value.
    let napi_info = match build_napi_params(input.iter(), &json_ty_by_name) {
        Ok(info) => info,
        Err(err) => return err.into_compile_error().into(),
    };

    let napi_fn_params = &napi_info.params;
    let napi_param_conversions = &napi_info.conversions;
    let napi_call_arg_idents = &napi_info.call_arg_idents;

    let napi_call_fn_args = if asyncness.is_some() {
        quote! { #fn_name(#(#napi_call_arg_idents),*).await }
    } else {
        quote! { #fn_name(#(#napi_call_arg_idents),*) }
    };

    // Struct name: <FunctionName>Params (PascalCase)
    let param_struct_name = format_ident!("{}Params", fn_name.to_string().to_case(Case::Pascal));

    // Cmd function name: <function_name>_cmd
    let fn_cmd_name = format_ident!("{}_cmd", fn_name);

    // Cmd function name: <function_name>_json
    let fn_json_name = format_ident!("{}_json", fn_name);

    // Napi function name: <function_name>_napi
    let fn_napi_name = format_ident!("{}_napi", fn_name);

    // Module name for tauri-renames, to keep the original function names.
    let napi_mod_name = format_ident!("napi_{}", fn_name);

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

    // Compute the TypeScript return type name string for napi's ts_return_type attribute.
    let ts_return_type_str = type_to_ts_name(&json_ty);

    // The napi function needs `legacy` when json_ty_by_name is non-empty (Context parameter).
    let napi_legacy_cfg = if !json_ty_by_name.is_empty() {
        quote! { #[cfg(all(feature = "napi", feature = "legacy"))] }
    } else {
        quote! { #[cfg(feature = "napi")] }
    };

    // Build the napi function body.
    // For async functions, we use an `async {}` block; for sync, a closure.
    // Both return anyhow::Result to handle any error type uniformly.
    let napi_body = if asyncness.is_some() {
        quote! {
            let __napi_body_result: ::anyhow::Result<::serde_json::Value> = async {
                let result = #napi_call_fn_args?;
                #convert_to_json_result_type
                Ok(::serde_json::to_value(result)?)
            }.await;
            __napi_body_result.map_err(|e: ::anyhow::Error| {
                let ctx = but_error::AnyhowContextExt::custom_context_or_error_chain(&e);
                let message = ctx
                    .message
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| format!("{e:#}"));
                napi::Error::new(napi::Status::GenericFailure, message)
            })
        }
    } else {
        quote! {
            let __napi_body_result: ::anyhow::Result<::serde_json::Value> = (|| {
                let result = #napi_call_fn_args?;
                #convert_to_json_result_type
                Ok(::serde_json::to_value(result)?)
            })();
            __napi_body_result.map_err(|e: ::anyhow::Error| {
                let ctx = but_error::AnyhowContextExt::custom_context_or_error_chain(&e);
                let message = ctx
                    .message
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| format!("{e:#}"));
                napi::Error::new(napi::Status::GenericFailure, message)
            })
        }
    };

    let napi_fn_block = if opts.napi {
        quote! {
            /// napi function - strongly typed params, serde_json::Value output, automatic error conversion.
            #napi_legacy_cfg
            #[napi_derive::napi(ts_return_type = #ts_return_type_str)]
            #vis #asyncness fn #fn_napi_name(
                #(#napi_fn_params),*
            ) -> napi::Result<::serde_json::Value> {
                #(#napi_param_conversions);*
                #napi_body
            }

            /// A module to re-export napi functions with the original function name.
            #napi_legacy_cfg
            pub mod #napi_mod_name {
                pub use super::#fn_napi_name as #fn_name;
            }
        }
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

        const _: () = {
            #[allow(dead_code)]
            fn keep_json(_json: #json_ty) {}
        };

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

        #napi_fn_block
    };

    expanded.into()
}

struct JsonParameterMapping {
    /// The mapped type to which the actual type can be converted.
    json_ty: syn::Path,
    /// The identifier to use when referring to the `json_ty`.
    ///
    /// This is important as the frontend actually uses the parameter names as identifiers.
    json_ident: Option<syn::Ident>,
    /// How to convert `json_ty` to the actual type.
    from_mode: FromMode,
}

/// The mapping is from type name to their respective json types.
fn build_json_type_mapping<'a>(
    input: impl IntoIterator<Item = &'a syn::FnArg>,
) -> Result<HashMap<String, JsonParameterMapping>, syn::Error> {
    let mut out = HashMap::new();

    for arg in input {
        let syn::FnArg::Typed(pat_ty) = arg else {
            continue;
        };

        let pat = &pat_ty.pat;
        let ty = &pat_ty.ty;

        let Pat::Ident(pat_ident) = &**pat else {
            continue;
        };

        let (path, is_reference) = if let syn::Type::Reference(r) = &**ty {
            // Extract the referenced type
            let inner = &r.elem;
            let syn::Type::Path(tp) = &**inner else {
                return Err(syn::Error::new_spanned(inner, "Expected a type path inside reference"));
            };

            (&tp.path, true)
        } else if let syn::Type::Path(ty_path) = &**ty {
            (&ty_path.path, false)
        } else {
            continue;
        };

        let segments = &path.segments;
        if segments.is_empty() {
            return Err(syn::Error::new_spanned(ty, "Unexpected empty type path in reference"));
        }

        let last = &segments.last().unwrap().ident;
        let (name, mapping) = if (last == "Context" || last == "ThreadSafeContext")
            && (segments.len() == 1 || segments[0].ident == "but_ctx")
        {
            (
                pat_ident.ident.to_string(),
                JsonParameterMapping {
                    json_ty: syn::parse_str("but_ctx::LegacyProjectId")?,
                    json_ident: Some(syn::parse_str("project_id")?),
                    from_mode: FromMode::TryFrom,
                },
            )
        } else if last == "ObjectId" && (segments.len() == 1 || segments[0].ident == "gix") {
            (
                pat_ident.ident.to_string(),
                JsonParameterMapping {
                    json_ty: syn::parse_str("crate::json::HexHash")?,
                    json_ident: None,
                    from_mode: FromMode::From,
                },
            )
        } else if is_reference {
            return Err(syn::Error::new_spanned(
                ty,
                "Only `&Context` or `&but_ctx::Context` may be references",
            ));
        } else {
            continue;
        };
        out.insert(name, mapping);
    }

    Ok(out)
}

fn extract_ok_type(output: &syn::ReturnType) -> syn::Result<syn::Type> {
    let ty = match output {
        syn::ReturnType::Type(_, ty) => ty.as_ref(),
        _ => {
            return Err(syn::Error::new_spanned(output, "function must return a type"));
        }
    };

    let syn::Type::Path(tp) = ty else {
        return Err(syn::Error::new_spanned(ty, "expected a type path"));
    };

    let last = tp
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(tp, "unexpected empty type path"))?;

    if last.ident != "Result" {
        return Err(syn::Error::new_spanned(last, "expected Result<T> or Result<T, E>"));
    }

    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return Err(syn::Error::new_spanned(last, "expected Result<T> or Result<T, E>"));
    };

    if args.args.is_empty() {
        return Err(syn::Error::new_spanned(
            args,
            "Result must have at least one generic parameter",
        ));
    }

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
    /// If `true`, generate a `_napi` function for Node.js bindings.
    /// Enabled by writing `#[but_api(napi)]` or `#[but_api(napi, try_from = Foo)]`.
    napi: bool,
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
    let mut napi = false;
    let path = match meta {
        syn::Meta::Path(path) => {
            if path.is_ident("napi") {
                // #[but_api(napi)]
                napi = true;
                None
            } else {
                // #[but_api(Foo)]
                Some((FromMode::From, path))
            }
        }
        syn::Meta::NameValue(nv) => {
            if let (Some(ident), Expr::Path(path)) = (&nv.path.get_ident(), &nv.value) {
                if *ident == "try_from" {
                    // #[but_api(try_from = Foo)]
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
            // #[but_api(napi, try_from = Foo)] or #[but_api(napi, Foo)]
            let mut conversion_path = None;
            list.parse_nested_meta(|nested| {
                if nested.path.is_ident("napi") {
                    napi = true;
                    Ok(())
                } else if nested.path.is_ident("try_from") {
                    // try_from = Foo
                    let value = nested.value()?;
                    let path: syn::Path = value.parse()?;
                    conversion_path = Some((FromMode::TryFrom, path));
                    Ok(())
                } else {
                    // Bare path like `Foo` → FromMode::From
                    conversion_path = Some((FromMode::From, nested.path.clone()));
                    Ok(())
                }
            })?;
            conversion_path
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
    Ok(Options {
        result_conversion,
        napi,
    })
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

/// Information about the napi-specific parameters for a function.
struct NapiParamsInfo {
    /// The napi-compatible function parameters.
    params: Vec<proc_macro2::TokenStream>,
    /// Code to convert napi parameters into the types the original function expects.
    conversions: Vec<proc_macro2::TokenStream>,
    /// The identifiers to pass to the original function call (may include `&` or `&mut`).
    call_arg_idents: Vec<proc_macro2::TokenStream>,
}

/// Build napi-compatible parameter lists and conversions.
///
/// For napi, we need to map Rust types to types that napi-rs can handle:
/// - `Context`/`&Context`/`&mut Context`/`ThreadSafeContext` → `String` (project_id)
/// - `gix::ObjectId` → `String` (hex-encoded)
/// - `json::HexHash` → `String` (hex-encoded)
/// - `BString` → `String`
/// - Other types that implement Serialize/Deserialize → `serde_json::Value`
fn build_napi_params<'a>(
    input: impl IntoIterator<Item = &'a syn::FnArg>,
    json_ty_by_name: &HashMap<String, JsonParameterMapping>,
) -> Result<NapiParamsInfo, syn::Error> {
    let mut params = Vec::new();
    let mut conversions = Vec::new();
    let mut call_arg_idents = Vec::new();

    for arg in input {
        let syn::FnArg::Typed(pat_ty) = arg else {
            continue;
        };
        let Pat::Ident(pat_ident) = &*pat_ty.pat else {
            return Err(syn::Error::new_spanned(
                &pat_ty.pat,
                "Cannot handle this pattern in napi generation",
            ));
        };
        let ident = &pat_ident.ident;

        // Check if this parameter has a json type mapping (Context, ObjectId)
        if let Some(mapping) = json_ty_by_name.get(&ident.to_string()) {
            let param_name = mapping.json_ident.as_ref().unwrap_or(ident);
            let last_segment = mapping.json_ty.segments.last().unwrap();
            let last_ident = &last_segment.ident;

            if *last_ident == "LegacyProjectId" {
                // Context → String project_id, then convert to Context
                params.push(quote! { #param_name: String });
                // Determine the actual type we need to produce (stripping references)
                let actual_ty = match &*pat_ty.ty {
                    syn::Type::Reference(r) => &*r.elem,
                    other => other,
                };
                conversions.push(quote! {
                    let project_id: but_ctx::LegacyProjectId = #param_name.parse()
                        .map_err(|e: <but_ctx::LegacyProjectId as ::std::str::FromStr>::Err| {
                            napi::Error::new(napi::Status::InvalidArg, format!("{e:#}"))
                        })?;
                    let mut #ident = <#actual_ty>::try_from(project_id)
                        .map_err(|e: anyhow::Error| napi::Error::new(napi::Status::GenericFailure, format!("{e:#}")))?;
                });
                // Pass as reference if original was a reference
                let call_ident = match &*pat_ty.ty {
                    syn::Type::Reference(r) => {
                        let mutability = &r.mutability;
                        quote! { &#mutability #ident }
                    }
                    _ => quote! { #ident },
                };
                call_arg_idents.push(call_ident);
            } else if *last_ident == "HexHash" {
                // ObjectId via HexHash → String, then parse
                params.push(quote! { #param_name: String });
                conversions.push(quote! {
                    let #ident = ::std::str::FromStr::from_str(&#param_name)
                        .map_err(|e: gix::hash::decode::Error| napi::Error::new(napi::Status::InvalidArg, format!("{e}")))?;
                });
                let call_ident = match &*pat_ty.ty {
                    syn::Type::Reference(r) => {
                        let mutability = &r.mutability;
                        quote! { &#mutability #ident }
                    }
                    _ => quote! { #ident },
                };
                call_arg_idents.push(call_ident);
            } else {
                // Fallback: use serde_json::Value with ts_arg_type for proper TS typing
                let ts_type_str = type_to_ts_name(&pat_ty.ty);
                params.push(quote! { #[napi(ts_arg_type = #ts_type_str)] #param_name: ::serde_json::Value });
                let actual_ty = match &*pat_ty.ty {
                    syn::Type::Reference(r) => &*r.elem,
                    other => other,
                };
                conversions.push(quote! {
                    let mut #ident: #actual_ty = ::serde_json::from_value(#param_name)
                        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, format!("{e}")))?;
                });
                let call_ident = match &*pat_ty.ty {
                    syn::Type::Reference(r) => {
                        let mutability = &r.mutability;
                        quote! { &#mutability #ident }
                    }
                    _ => quote! { #ident },
                };
                call_arg_idents.push(call_ident);
            }
        } else {
            // No json mapping — use the original type information
            let ty = &*pat_ty.ty;
            let (base_ty, is_ref) = match ty {
                syn::Type::Reference(r) => (&*r.elem, true),
                other => (other, false),
            };

            // Check for known types that need special napi handling
            let type_name = type_last_segment_name(base_ty);
            match type_name.as_deref() {
                Some("BString") => {
                    params.push(quote! { #ident: String });
                    conversions.push(quote! {
                        let #ident: bstr::BString = #ident.into();
                    });
                    if is_ref {
                        call_arg_idents.push(quote! { &#ident });
                    } else {
                        call_arg_idents.push(quote! { #ident });
                    }
                }
                Some("HexHash") => {
                    // json::HexHash used directly as parameter (not via ObjectId mapping)
                    params.push(quote! { #ident: String });
                    conversions.push(quote! {
                        let #ident: crate::json::HexHash = ::std::str::FromStr::from_str(&#ident)
                            .map(crate::json::HexHash)
                            .map_err(|e: gix::hash::decode::Error| napi::Error::new(napi::Status::InvalidArg, format!("{e}")))?;
                    });
                    call_arg_idents.push(quote! { #ident });
                }
                _ => {
                    // For all other types: check for napi-incompatible types first
                    if let Some((napi_ty, cast_expr)) = napi_type_remap(base_ty) {
                        // Type needs remapping (e.g., usize → i64)
                        params.push(quote! { #ident: #napi_ty });
                        conversions.push(quote! {
                            let #ident = #ident #cast_expr;
                        });
                        call_arg_idents.push(quote! { #ident });
                    } else if is_simple_napi_type(base_ty) {
                        // Simple types (String, bool, numbers) can be passed directly
                        params.push(quote! { #ident: #ty });
                        call_arg_idents.push(quote! { #ident });
                    } else {
                        // Complex types → serde_json::Value with ts_arg_type for proper TS typing
                        let ts_type_str = type_to_ts_name(ty);
                        params.push(quote! { #[napi(ts_arg_type = #ts_type_str)] #ident: ::serde_json::Value });
                        conversions.push(quote! {
                            let #ident: #base_ty = ::serde_json::from_value(#ident)
                                .map_err(|e| napi::Error::new(napi::Status::InvalidArg, format!("{e}")))?;
                        });
                        if is_ref {
                            call_arg_idents.push(quote! { &#ident });
                        } else {
                            call_arg_idents.push(quote! { #ident });
                        }
                    }
                }
            }
        }
    }

    Ok(NapiParamsInfo {
        params,
        conversions,
        call_arg_idents,
    })
}

/// Get the last segment name of a type path, if it is a simple path.
fn type_last_segment_name(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(tp) = ty {
        tp.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

/// Check if a type is "simple" enough to pass directly to napi without serde_json::Value.
/// This includes primitive types and String.
fn is_simple_napi_type(ty: &syn::Type) -> bool {
    let Some(name) = type_last_segment_name(ty) else {
        return false;
    };
    matches!(
        name.as_str(),
        "String" | "bool" | "u8" | "u16" | "u32" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64"
    )
}

/// Check if a Rust type needs to be remapped for napi compatibility.
/// Returns Some(napi_type, conversion_code) if the type needs remapping.
fn napi_type_remap(ty: &syn::Type) -> Option<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    let name = type_last_segment_name(ty)?;
    match name.as_str() {
        // napi doesn't support usize/isize — remap to i64
        "usize" => Some((quote! { i64 }, quote! { as usize })),
        "isize" => Some((quote! { i64 }, quote! { as isize })),
        _ => None,
    }
}

/// Convert a Rust `syn::Type` to a TypeScript type name string.
///
/// This produces a string suitable for napi-rs's `ts_return_type` attribute
/// and for schema registration names.
fn type_to_ts_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(tp) => {
            let segments: Vec<_> = tp.path.segments.iter().collect();
            if segments.is_empty() {
                return "any".to_string();
            }
            let last = segments.last().unwrap();
            let name = last.ident.to_string();

            match name.as_str() {
                // Primitive type mappings
                "String" | "str" => "string".to_string(),
                "bool" => "boolean".to_string(),
                "u8" | "u16" | "u32" | "i8" | "i16" | "i32" | "f32" | "f64" | "usize" | "isize" => "number".to_string(),
                "i64" | "u64" | "i128" | "u128" => "number".to_string(),
                // Unit type
                "()" => "void".to_string(),
                // Generic containers
                "Vec" => {
                    if let syn::PathArguments::AngleBracketed(args) = &last.arguments
                        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
                    {
                        let inner_name = type_to_ts_name(inner);
                        return format!("Array<{inner_name}>");
                    }
                    "Array<any>".to_string()
                }
                "Option" => {
                    if let syn::PathArguments::AngleBracketed(args) = &last.arguments
                        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
                    {
                        let inner_name = type_to_ts_name(inner);
                        return format!("{inner_name} | null");
                    }
                    "any | null".to_string()
                }
                "HashMap" | "BTreeMap" => {
                    if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                        let mut iter = args.args.iter();
                        if let (Some(syn::GenericArgument::Type(k)), Some(syn::GenericArgument::Type(v))) =
                            (iter.next(), iter.next())
                        {
                            let key_name = type_to_ts_name(k);
                            let val_name = type_to_ts_name(v);
                            return format!("Record<{key_name}, {val_name}>");
                        }
                    }
                    "Record<string, any>".to_string()
                }
                // Tuple of two elements (e.g., (HexHash, HexHash))
                "HexHash" | "HexHashString" => "string".to_string(),
                "ObjectId" => "string".to_string(),
                "BString" => "string".to_string(),
                // Named types — use their name as-is (these will be defined in the generated .d.ts)
                other => other.to_string(),
            }
        }
        syn::Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                "void".to_string()
            } else {
                let inner: Vec<_> = tuple.elems.iter().map(type_to_ts_name).collect();
                format!("[{}]", inner.join(", "))
            }
        }
        syn::Type::Reference(r) => type_to_ts_name(&r.elem),
        _ => "any".to_string(),
    }
}
