// Case: `provides` given twice should fail macro option parsing.
// Extend when: the accepted shape of the tag lists changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture, tags};

#[but_api(napi, provides = [Reviews], provides = [Checks])]
pub fn duplicated_provides() -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash(
        "0123456789abcdef0123456789abcdef01234567".into(),
    ))
}

fn main() {}
