// Case: references are restricted; only context-like references and `&gix::refs::FullNameRef` are allowed.
// Extend when: additional reference parameter types become supported.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api]
fn wrong_reference(_value: &String) -> anyhow::Result<String> {
    Ok("ok".into())
}

fn main() {}
