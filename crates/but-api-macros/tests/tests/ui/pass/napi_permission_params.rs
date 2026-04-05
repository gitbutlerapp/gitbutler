// Case: `#[but_api(napi)]` generation hides explicit repository permission tokens.
// It verifies napi wrapper generation when the annotated function takes `RepoExclusive`.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi)]
pub fn napi_surface_with_perm(
    _ctx: &mut but_ctx::Context,
    _perm: &mut but_ctx::access::RepoExclusive,
    value: i32,
) -> anyhow::Result<i32> {
    Ok(value)
}

fn main() {
    let _ = napi_surface_with_perm_napi;
    let _ = napi_napi_surface_with_perm::napi_surface_with_perm;
}
