// Case: `refreshes` on a function without a context should fail: there is no
// project storage directory to write the refresh sentinel to.
// Extend when: `refreshes` learns another way to locate the project.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api(refreshes)]
pub fn refreshes_without_context(value: i32) -> anyhow::Result<i32> {
    Ok(value)
}

fn main() {}
