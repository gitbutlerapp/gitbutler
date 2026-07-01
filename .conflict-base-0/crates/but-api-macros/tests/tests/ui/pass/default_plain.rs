// Case: baseline `#[but_api]` expansion without explicit return conversion.
// It verifies that `<fn>_json` always exists, and `<fn>_cmd` exists under `legacy`.
// Extend when: default wrapper signatures or generated function naming changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api]
pub fn sum(left: i32, right: i32) -> anyhow::Result<i32> {
    Ok(left + right)
}

fn main() {
    let _ = sum_json(1, 2);
    #[cfg(feature = "legacy")]
    {
        let _ = sum_cmd(serde_json::json!({ "left": 1, "right": 2 }));
    }
}
