use std::collections::HashMap;

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, parse_macro_input};

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
/// These can be combined with commas, e.g. `#[but_api(napi, try_from = json::CommitDetails)]`
/// or `#[but_api(napi, json::CommitDetails)]`.
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
///           which will be translated to `project_id`:
///           - in legacy builds: `ProjectHandleOrLegacyProjectId`
///           - without legacy: `ProjectHandle`
///         - `gix::ObjectId` will be translated into `json::HexHash`.
/// * `func_cmd` for calls from the `but-server`, taking `(params: Params) ` and returning `Result<serde_json::Value, json::Error>`.
///     - It performs all **Parameter Transformations** of `func_json`.
/// * `func_napi` (opt-in) for calls from Node.js via napi-rs, taking individual typed parameters and returning `napi::Result<serde_json::Value>`.
///     - **Only generated when `napi` is specified**: `#[but_api(napi)]`, `#[but_api(napi, Foo)]`, or `#[but_api(napi, try_from = Foo)]`.
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
        match syn::parse::Parser::parse(
            |input: syn::parse::ParseStream| parse_options(input, is_result_option),
            attr,
        ) {
            Ok(opts) => opts,
            Err(err) => return err.into_compile_error().into(),
        }
    };

    let wrapper_params = match build_wrapper_params(input.iter()) {
        Ok(info) => info,
        Err(err) => return err.into_compile_error().into(),
    };
    let struct_fields_with_json_types = &wrapper_params.struct_fields_with_json_types;
    let param_field_names = &wrapper_params.param_field_names;
    let json_fn_input_params = &wrapper_params.json_fn_input_params;
    let param_conversions = &wrapper_params.param_conversions;
    let call_arg_idents = &wrapper_params.call_arg_idents;

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
    let napi_info = if opts.napi {
        let json_ty_by_name = match build_json_type_mapping(input.iter()) {
            Ok(m) => m,
            Err(err) => return err.into_compile_error().into(),
        };
        match build_napi_params(input.iter(), &json_ty_by_name) {
            Ok(info) => info,
            Err(err) => return err.into_compile_error().into(),
        }
    } else {
        NapiParamsInfo::default()
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

    let legacy_cfg_if_json_mapping_is_used = if wrapper_params.requires_legacy {
        quote! { #[cfg(feature = "legacy")] }
    } else {
        quote! {}
    };

    // Compute the TypeScript return type name string for napi's ts_return_type attribute.
    let ts_return_type_str = format!("Promise<{}>", type_to_ts_name(&json_ty));

    // The napi function needs `legacy` when json_ty_by_name is non-empty (Context parameter).
    let napi_legacy_cfg = if wrapper_params.requires_legacy {
        quote! { #[cfg(all(feature = "napi", feature = "legacy"))] }
    } else {
        quote! { #[cfg(feature = "napi")] }
    };

    // Build the napi function body.
    // For async functions, we use an `async {}` block; for sync, spawn_blocking.
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
        // For sync functions, param conversions must be inside spawn_blocking
        // so that non-Send types (e.g. Context with Rc) are never moved across threads.
        quote! {
            ::tokio::task::spawn_blocking(move || {
                #(#napi_param_conversions);*
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
            })
            .await
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("spawn_blocking join error: {e}")))?
        }
    };

    // For async functions, param conversions happen outside the body (in the async fn).
    // For sync functions, they're already inside spawn_blocking in napi_body.
    let napi_external_conversions = if asyncness.is_some() {
        quote! { #(#napi_param_conversions);* }
    } else {
        quote! {}
    };

    let napi_fn_block = if opts.napi {
        quote! {
            /// napi function - strongly typed params, serde_json::Value output, automatic error conversion.
            #napi_legacy_cfg
            #[napi_derive::napi(ts_return_type = #ts_return_type_str)]
            #vis async fn #fn_napi_name(
                #(#napi_fn_params),*
            ) -> napi::Result<::serde_json::Value> {
                #napi_external_conversions
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

        /// Cmd function - this is legacy while most of its functionality depends on legacy project integration.
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

struct WrapperParamsInfo {
    struct_fields_with_json_types: Vec<proc_macro2::TokenStream>,
    param_field_names: Vec<syn::Ident>,
    json_fn_input_params: Vec<FnArg>,
    param_conversions: Vec<proc_macro2::TokenStream>,
    call_arg_idents: Vec<proc_macro2::TokenStream>,
    requires_legacy: bool,
}

struct WrapperParameterMapping {
    transport_ty: syn::Type,
    binding_ty: syn::Type,
    json_ident: Option<syn::Ident>,
    conversion_kind: WrapperConversionKind,
    call_arg_kind: WrapperCallArgKind,
    requires_legacy: bool,
}

enum WrapperConversionKind {
    From,
    TryFrom,
    OptionFrom,
    VecFrom,
    Identity,
}

enum WrapperCallArgKind {
    Same,
    Ref { mutable: bool },
    AsRef,
}

fn build_wrapper_params<'a>(
    input: impl IntoIterator<Item = &'a syn::FnArg>,
) -> Result<WrapperParamsInfo, syn::Error> {
    let mut struct_fields_with_json_types = Vec::new();
    let mut param_field_names = Vec::new();
    let mut json_fn_input_params = Vec::new();
    let mut param_conversions = Vec::new();
    let mut call_arg_idents = Vec::new();
    let mut requires_legacy = false;

    for arg in input {
        let FnArg::Typed(pat_ty) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "Cannot handle &self, &mut self or self",
            ));
        };
        let Pat::Ident(pat_ident) = &*pat_ty.pat else {
            return Err(syn::Error::new_spanned(
                pat_ty,
                "Cannot handle this identifier",
            ));
        };
        let ident = &pat_ident.ident;

        if let Some(mapping) = build_wrapper_parameter_mapping(&pat_ty.ty)? {
            let json_ident = mapping.json_ident.unwrap_or_else(|| ident.clone());
            let transport_ty = &mapping.transport_ty;
            let binding_ty = &mapping.binding_ty;
            param_field_names.push(json_ident.clone());
            struct_fields_with_json_types.push(quote! { pub #json_ident: #transport_ty });
            json_fn_input_params.push(syn::parse_quote! { #json_ident: #transport_ty });
            param_conversions.push(match mapping.conversion_kind {
                WrapperConversionKind::From => quote! {
                    let mut #ident: #binding_ty = <#binding_ty>::from(#json_ident);
                },
                WrapperConversionKind::TryFrom => quote! {
                    let mut #ident: #binding_ty = <#binding_ty>::try_from(#json_ident)?;
                },
                WrapperConversionKind::OptionFrom => quote! {
                    let mut #ident: #binding_ty = #json_ident.map(Into::into);
                },
                WrapperConversionKind::VecFrom => quote! {
                    let mut #ident: #binding_ty = #json_ident.into_iter().map(Into::into).collect();
                },
                WrapperConversionKind::Identity => quote! {
                    let mut #ident: #binding_ty = #json_ident;
                },
            });
            call_arg_idents.push(match mapping.call_arg_kind {
                WrapperCallArgKind::Same => quote! { #ident },
                WrapperCallArgKind::Ref { mutable: true } => quote! { &mut #ident },
                WrapperCallArgKind::Ref { mutable: false } => quote! { &#ident },
                WrapperCallArgKind::AsRef => quote! { #ident.as_ref() },
            });
            requires_legacy |= mapping.requires_legacy;
        } else {
            param_field_names.push(ident.clone());
            let ty = &pat_ty.ty;
            struct_fields_with_json_types.push(quote! { pub #ident: #ty });
            json_fn_input_params.push(arg.clone());
            call_arg_idents.push(quote! { #ident });
        }
    }

    Ok(WrapperParamsInfo {
        struct_fields_with_json_types,
        param_field_names,
        json_fn_input_params,
        param_conversions,
        call_arg_idents,
        requires_legacy,
    })
}

fn build_wrapper_parameter_mapping(
    ty: &syn::Type,
) -> Result<Option<WrapperParameterMapping>, syn::Error> {
    match ty {
        syn::Type::Reference(reference) => {
            let syn::Type::Path(type_path) = &*reference.elem else {
                return Err(syn::Error::new_spanned(
                    &reference.elem,
                    "Expected a type path inside reference",
                ));
            };
            let path = &type_path.path;
            if is_context_path(path) {
                let binding_ty: syn::Type = (*reference.elem).clone();
                return Ok(Some(WrapperParameterMapping {
                    transport_ty: syn::parse_quote! { but_ctx::LegacyProjectId },
                    binding_ty,
                    json_ident: Some(syn::parse_str("project_id")?),
                    conversion_kind: WrapperConversionKind::TryFrom,
                    call_arg_kind: WrapperCallArgKind::Ref {
                        mutable: reference.mutability.is_some(),
                    },
                    requires_legacy: true,
                }));
            }
            if is_full_name_ref_path(path) {
                return Ok(Some(WrapperParameterMapping {
                    transport_ty: syn::parse_quote! { gix::refs::FullName },
                    binding_ty: syn::parse_quote! { gix::refs::FullName },
                    json_ident: None,
                    conversion_kind: WrapperConversionKind::Identity,
                    call_arg_kind: WrapperCallArgKind::AsRef,
                    requires_legacy: false,
                }));
            }
            Err(syn::Error::new_spanned(
                ty,
                "Only `&Context`, `&but_ctx::Context`, `&ThreadSafeContext`, or `&gix::refs::FullNameRef` may be references",
            ))
        }
        syn::Type::Path(type_path) => {
            let path = &type_path.path;
            if is_context_path(path) {
                return Ok(Some(WrapperParameterMapping {
                    transport_ty: syn::parse_quote! { but_ctx::LegacyProjectId },
                    binding_ty: ty.clone(),
                    json_ident: Some(syn::parse_str("project_id")?),
                    conversion_kind: WrapperConversionKind::TryFrom,
                    call_arg_kind: WrapperCallArgKind::Same,
                    requires_legacy: true,
                }));
            }
            if is_object_id_path(path) {
                return Ok(Some(WrapperParameterMapping {
                    transport_ty: syn::parse_quote! { crate::json::HexHash },
                    binding_ty: ty.clone(),
                    json_ident: None,
                    conversion_kind: WrapperConversionKind::From,
                    call_arg_kind: WrapperCallArgKind::Same,
                    requires_legacy: false,
                }));
            }
            if let Some(inner_ty) = single_generic_type_arg(path, "Option")
                && is_object_id_type(inner_ty)
            {
                return Ok(Some(WrapperParameterMapping {
                    transport_ty: syn::parse_quote! { Option<crate::json::HexHash> },
                    binding_ty: ty.clone(),
                    json_ident: None,
                    conversion_kind: WrapperConversionKind::OptionFrom,
                    call_arg_kind: WrapperCallArgKind::Same,
                    requires_legacy: false,
                }));
            }
            if let Some(inner_ty) = single_generic_type_arg(path, "Vec")
                && is_object_id_type(inner_ty)
            {
                return Ok(Some(WrapperParameterMapping {
                    transport_ty: syn::parse_quote! { Vec<crate::json::HexHash> },
                    binding_ty: ty.clone(),
                    json_ident: None,
                    conversion_kind: WrapperConversionKind::VecFrom,
                    call_arg_kind: WrapperCallArgKind::Same,
                    requires_legacy: false,
                }));
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn is_context_path(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|last| last.ident == "Context" || last.ident == "ThreadSafeContext")
        && (path.segments.len() == 1 || path.segments[0].ident == "but_ctx")
}

fn is_object_id_path(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|last| last.ident == "ObjectId")
        && (path.segments.len() == 1 || path.segments[0].ident == "gix")
}

fn is_object_id_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(type_path) if is_object_id_path(&type_path.path))
}

fn is_full_name_ref_path(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|last| last.ident == "FullNameRef")
        && (path.segments.len() == 1
            || (path.segments.len() >= 3
                && path.segments[0].ident == "gix"
                && path.segments[1].ident == "refs"))
}

fn single_generic_type_arg<'a>(path: &'a syn::Path, expected: &str) -> Option<&'a syn::Type> {
    let last = path.segments.last()?;
    if last.ident != expected {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    match args.args.first()? {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    }
}

struct JsonParameterMapping {
    /// The mapped type to which the actual type can be converted.
    json_ty: syn::Path,
    /// The identifier to use when referring to the `json_ty`.
    ///
    /// This is important as the frontend actually uses the parameter names as identifiers.
    json_ident: Option<syn::Ident>,
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
                return Err(syn::Error::new_spanned(
                    inner,
                    "Expected a type path inside reference",
                ));
            };

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
        let (name, mapping) = if (last == "Context" || last == "ThreadSafeContext")
            && (segments.len() == 1 || segments[0].ident == "but_ctx")
        {
            (
                pat_ident.ident.to_string(),
                JsonParameterMapping {
                    json_ty: syn::parse_str("but_ctx::ProjectHandleOrLegacyProjectId")?,
                    json_ident: Some(syn::parse_str("project_id")?),
                },
            )
        } else if last == "ObjectId" && (segments.len() == 1 || segments[0].ident == "gix") {
            (
                pat_ident.ident.to_string(),
                JsonParameterMapping {
                    json_ty: syn::parse_str("crate::json::HexHash")?,
                    json_ident: None,
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
            return Err(syn::Error::new_spanned(
                output,
                "function must return a type",
            ));
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

fn parse_options(input: syn::parse::ParseStream, is_result_option: bool) -> syn::Result<Options> {
    let mut napi = false;
    let mut conversion_path: Option<(FromMode, syn::Path)> = None;

    while !input.is_empty() {
        if input.peek(syn::Ident) && input.peek2(syn::Token![=]) {
            // try_from = Path
            let ident: syn::Ident = input.parse()?;
            if ident != "try_from" {
                return Err(syn::Error::new_spanned(
                    ident,
                    "Expected `try_from = Type`; only `try_from` is supported as a key",
                ));
            }
            input.parse::<syn::Token![=]>()?;
            let path: syn::Path = input.parse()?;
            if conversion_path.is_some() {
                return Err(syn::Error::new_spanned(
                    path,
                    "Only one conversion type may be specified",
                ));
            }
            conversion_path = Some((FromMode::TryFrom, path));
        } else {
            let path: syn::Path = input.parse()?;
            if path.is_ident("napi") {
                napi = true;
            } else {
                if conversion_path.is_some() {
                    return Err(syn::Error::new_spanned(
                        path,
                        "Only one conversion type may be specified",
                    ));
                }
                conversion_path = Some((FromMode::From, path));
            }
        }

        if !input.is_empty() {
            input.parse::<syn::Token![,]>()?;
        }
    }

    let result_conversion = conversion_path.map(|(mode, p)| {
        let base_ty = syn::Type::Path(syn::TypePath {
            qself: None,
            path: p,
        });
        let json_ty_rval = if is_result_option {
            syn::parse_quote! { Option<#base_ty> }
        } else {
            base_ty.clone()
        };
        ResultConversion {
            mode,
            is_result_option,
            json_ty: base_ty,
            json_ty_rval,
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
#[derive(Default)]
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

            if *last_ident == "LegacyProjectId" || *last_ident == "ProjectHandleOrLegacyProjectId" {
                // Context → String project_id, then convert to Context
                params.push(quote! { #param_name: String });
                // Determine the actual type we need to produce (stripping references)
                let actual_ty = match &*pat_ty.ty {
                    syn::Type::Reference(r) => &*r.elem,
                    other => other,
                };
                let json_ty = &mapping.json_ty;
                conversions.push(quote! {
                    let project_id: #json_ty = #param_name.parse()
                        .map_err(|e: <#json_ty as ::std::str::FromStr>::Err| {
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
                params.push(
                    quote! { #[napi(ts_arg_type = #ts_type_str)] #param_name: ::serde_json::Value },
                );
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
                    if let Some(napi_ty) = napi_type_remap(base_ty) {
                        // Type needs remapping (e.g., usize → i64)
                        params.push(quote! { #ident: #napi_ty });
                        let arg_name = ident.to_string();
                        let conversion = match type_name.as_deref() {
                            Some("usize") => quote! {
                                let #ident: usize = ::std::convert::TryFrom::try_from(#ident).map_err(|_| {
                                    napi::Error::new(
                                        napi::Status::InvalidArg,
                                        format!(
                                            "argument '{}' must be a non-negative integer that fits in usize",
                                            #arg_name
                                        ),
                                    )
                                })?;
                            },
                            Some("isize") => quote! {
                                let #ident: isize = ::std::convert::TryFrom::try_from(#ident).map_err(|_| {
                                    napi::Error::new(
                                        napi::Status::InvalidArg,
                                        format!(
                                            "argument '{}' must be an integer that fits in isize",
                                            #arg_name
                                        ),
                                    )
                                })?;
                            },
                            _ => quote! {},
                        };
                        conversions.push(conversion);
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
/// Returns Some(napi_type) if the type needs remapping.
fn napi_type_remap(ty: &syn::Type) -> Option<proc_macro2::TokenStream> {
    let name = type_last_segment_name(ty)?;
    match name.as_str() {
        // napi doesn't support usize/isize — remap to i64
        "usize" => Some(quote! { i64 }),
        "isize" => Some(quote! { i64 }),
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
                "u8" | "u16" | "u32" | "i8" | "i16" | "i32" | "f32" | "f64" | "usize" | "isize" => {
                    "number".to_string()
                }
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
                        if let (
                            Some(syn::GenericArgument::Type(k)),
                            Some(syn::GenericArgument::Type(v)),
                        ) = (iter.next(), iter.next())
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

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::{FnArg, parse_quote};

    use super::{
        WrapperCallArgKind, WrapperConversionKind, build_wrapper_parameter_mapping,
        build_wrapper_params,
    };

    #[test]
    fn maps_object_id_to_hex_hash_transport() {
        let ty = parse_quote!(gix::ObjectId);
        let mapping = build_wrapper_parameter_mapping(&ty).unwrap().unwrap();
        let transport_ty = &mapping.transport_ty;

        assert_eq!(
            quote!(#transport_ty).to_string(),
            quote!(crate::json::HexHash).to_string()
        );
        assert!(
            matches!(mapping.conversion_kind, WrapperConversionKind::From),
            "gix::ObjectId should use a direct From-based transport conversion"
        );
        assert!(
            matches!(mapping.call_arg_kind, WrapperCallArgKind::Same),
            "gix::ObjectId wrapper parameters should be passed through without call-site remapping"
        );
    }

    #[test]
    fn maps_option_and_vec_of_object_id_to_hex_hash_containers() {
        let option_ty = parse_quote!(Option<gix::ObjectId>);
        let option_mapping = build_wrapper_parameter_mapping(&option_ty)
            .unwrap()
            .unwrap();
        let option_transport_ty = &option_mapping.transport_ty;
        assert_eq!(
            quote!(#option_transport_ty).to_string(),
            quote!(Option<crate::json::HexHash>).to_string()
        );
        assert!(
            matches!(
                option_mapping.conversion_kind,
                WrapperConversionKind::OptionFrom
            ),
            "Option<gix::ObjectId> should map with OptionFrom conversion"
        );

        let vec_ty = parse_quote!(Vec<gix::ObjectId>);
        let vec_mapping = build_wrapper_parameter_mapping(&vec_ty).unwrap().unwrap();
        let vec_transport_ty = &vec_mapping.transport_ty;
        assert_eq!(
            quote!(#vec_transport_ty).to_string(),
            quote!(Vec<crate::json::HexHash>).to_string()
        );
        assert!(matches!(
            vec_mapping.conversion_kind,
            WrapperConversionKind::VecFrom
        ),);
    }

    #[test]
    fn maps_context_to_project_id() {
        let arg: FnArg = parse_quote!(ctx: &mut but_ctx::Context);
        let params = build_wrapper_params([&arg]).unwrap();
        let struct_fields = &params.struct_fields_with_json_types;
        let json_inputs = &params.json_fn_input_params;
        let call_args = &params.call_arg_idents;

        assert_eq!(
            quote!(#(#struct_fields),*).to_string(),
            quote!(pub project_id: but_ctx::LegacyProjectId).to_string()
        );
        assert_eq!(
            quote!(#(#json_inputs),*).to_string(),
            quote!(project_id: but_ctx::LegacyProjectId).to_string()
        );
        assert_eq!(
            quote!(#(#call_args),*).to_string(),
            quote!(&mut ctx).to_string()
        );
        assert!(
            params.requires_legacy,
            "context parameters should require the legacy project-id wrapper setup"
        );
    }

    #[test]
    /// Verifies that `&FullNameRef` parameters are transported as owned `FullName` values.
    fn maps_full_name_ref_to_owned_full_name_transport() {
        let arg: FnArg = parse_quote!(existing_branch: &gix::refs::FullNameRef);
        let params = build_wrapper_params([&arg]).unwrap();
        let struct_fields = &params.struct_fields_with_json_types;
        let json_inputs = &params.json_fn_input_params;
        let call_args = &params.call_arg_idents;

        assert_eq!(
            quote!(#(#struct_fields),*).to_string(),
            quote!(pub existing_branch: gix::refs::FullName).to_string()
        );
        assert_eq!(
            quote!(#(#json_inputs),*).to_string(),
            quote!(existing_branch: gix::refs::FullName).to_string()
        );
        assert_eq!(
            quote!(#(#call_args),*).to_string(),
            quote!(existing_branch.as_ref()).to_string()
        );
        assert!(
            !params.requires_legacy,
            "full-name references should not force legacy wrapper generation"
        );
    }
}
