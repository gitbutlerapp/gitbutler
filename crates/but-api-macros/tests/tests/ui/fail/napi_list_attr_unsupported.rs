// Case: list-form combination `#[but_api(napi, try_from = ...)]` currently fails to parse.
// This snapshots current behavior so parser changes are explicit.
// Extend when: list-form napi+conversion attributes become supported.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(napi, try_from = json::HexHash)]
pub fn unsupported_napi_attr_list() -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash(
        "0123456789abcdef0123456789abcdef01234567".into(),
    ))
}

fn main() {}
