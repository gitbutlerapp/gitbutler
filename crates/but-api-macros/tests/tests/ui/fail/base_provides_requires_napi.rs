// Case: an endpoint declaring `provides` without `napi` should fail — the
// declaration is only read from the napi registry, so it would be silently
// dropped.
// Extend when: tag declarations gain a non-napi consumer.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture, tags};

#[but_api(provides = [Reviews])]
pub fn provides_without_napi() -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash(
        "0123456789abcdef0123456789abcdef01234567".into(),
    ))
}

fn main() {}
