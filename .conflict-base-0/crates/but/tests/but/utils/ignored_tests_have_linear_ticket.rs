use std::path::Path;

use quote::ToTokens;
use syn::{ItemFn, LitStr, Token, parse::Parse};

use crate::utils::make_absolute;

/// Parse the file at the given path and ensure that all tests with `#[ignore]` have a "reason"
/// that includes a link to a Linear ticket.
#[track_caller]
pub fn assert_ignored_tests_have_linear_ticket(path: impl AsRef<Path>) {
    let mut path = path.as_ref().to_owned();
    if !path.is_absolute() {
        path = make_absolute(path);
    }

    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("failed to read {}", path.display()));
    let syntax =
        syn::parse_file(&src).unwrap_or_else(|_| panic!("failed to parse {}", path.display()));

    struct GatherTestFns<'a, 'ast>(&'a mut Vec<&'ast ItemFn>);

    impl<'a, 'ast> syn::visit::Visit<'ast> for GatherTestFns<'a, 'ast> {
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            let is_test_fn = item.attrs.iter().any(|attr| {
                let path = match &attr.meta {
                    syn::Meta::Path(path) => path,
                    syn::Meta::List(..) | syn::Meta::NameValue(..) => return false,
                };
                path.is_ident("test")
            });
            if is_test_fn {
                self.0.push(item);
            }
        }
    }

    let mut test_fns = Vec::new();
    let mut visitor = GatherTestFns(&mut test_fns);
    for item in &syntax.items {
        syn::visit::visit_item(&mut visitor, item);
    }

    let mut ignored_tests_without_ticket = Vec::new();
    for item_fn in test_fns {
        for attr in &item_fn.attrs {
            let tokens = attr.to_token_stream();
            let Ok(IgnoredAttr { reason }) = syn::parse2::<IgnoredAttr>(tokens) else {
                continue;
            };
            if reason
                .is_none_or(|reason| !reason.contains("https://linear.app/gitbutler/issue/GB-"))
            {
                ignored_tests_without_ticket.push(item_fn);
            }
        }
    }

    if ignored_tests_without_ticket.is_empty() {
        return;
    }

    let fn_names = ignored_tests_without_ticket
        .into_iter()
        .map(|item| format!("  - `{}` in {}", item.sig.ident, path.display()))
        .collect::<Vec<_>>()
        .join("\n");
    panic!(
        "  All ignored tests must have a `reason = \"...\"` that includes a link to a Linear ticket. The following tests dont:\n{fn_names}"
    );
}

mod kw {
    syn::custom_keyword!(ignore);
}

#[derive(Debug)]
struct IgnoredAttr {
    reason: Option<String>,
}

impl Parse for IgnoredAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<Token![#]>()?;
        let content;
        syn::bracketed!(content in input);

        content.parse::<kw::ignore>()?;

        let reason = if content.peek(Token![=]) {
            content.parse::<Token![=]>()?;
            Some(content.parse::<LitStr>()?.value())
        } else {
            None
        };

        Ok(Self { reason })
    }
}
