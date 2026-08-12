// Case: an endpoint declaring both `provides` and `invalidates` should fail —
// it is either a read or a mutation, never both.
// Extend when: that exclusivity rule changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture, tags};

#[but_api(napi, provides = [Reviews], invalidates = [Checks])]
pub fn provides_and_invalidates() -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash(
        "0123456789abcdef0123456789abcdef01234567".into(),
    ))
}

fn main() {}
