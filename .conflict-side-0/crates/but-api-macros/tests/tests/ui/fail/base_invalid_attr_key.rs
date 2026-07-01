// Case: invalid attribute key; only `try_from = ...` is currently accepted in name-value form.
// Extend when: additional named macro attributes are introduced.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(from = json::HexHash)]
fn wrong_attr() -> anyhow::Result<gix::ObjectId> {
    Ok(gix::ObjectId::null(gix::hash::Kind::Sha1))
}

fn main() {}
