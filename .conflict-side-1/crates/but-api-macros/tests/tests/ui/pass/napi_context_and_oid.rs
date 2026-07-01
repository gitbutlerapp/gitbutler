// Case: `#[but_api(napi)]` generation with legacy context remapping plus ObjectId input.
// It verifies `_napi` function/module creation and napi-safe parameter conversion codepaths.
// Extend when: napi context/object-id conversion behavior or exported naming changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi)]
pub fn napi_ctx_and_oid(_ctx: but_ctx::Context, oid: gix::ObjectId) -> anyhow::Result<String> {
    Ok(oid.to_hex().to_string())
}

fn main() {
    let _ = napi_ctx_and_oid_napi;
    let _ = napi_napi_ctx_and_oid::napi_ctx_and_oid;
}
