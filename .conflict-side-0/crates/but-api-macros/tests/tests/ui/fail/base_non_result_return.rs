// Case: annotated functions must return `Result<...>`.
// Extend when: macro support for non-`Result` return types is added.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api]
fn not_result() -> i32 {
    1
}

fn main() {}
