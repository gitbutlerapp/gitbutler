// Case: `refreshes` given twice should fail macro option parsing.
// Extend when: the accepted shape of `refreshes` changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(refreshes, refreshes)]
pub fn duplicated_refreshes(_ctx: &but_ctx::Context) -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash(
        "0123456789abcdef0123456789abcdef01234567".into(),
    ))
}

fn main() {}
