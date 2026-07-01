// Case: list-form `napi` with an unsupported key should fail macro option parsing.
// Extend when: accepted list-form keys for `napi` change.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi, from = json::HexHash)]
pub fn invalid_napi_attr_list_key() -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash(
        "0123456789abcdef0123456789abcdef01234567".into(),
    ))
}

fn main() {}
