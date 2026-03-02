// Case: `#[but_api(try_from = Type)]` using fallible `TryFrom` conversion.
// Extend when: try-conversion error propagation or return typing changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{UiValueTry, json, panic_capture};

#[but_api(try_from = UiValueTry)]
pub fn value_checked() -> anyhow::Result<i32> {
    Ok(7)
}

fn main() {
    let _ = value_checked_json();
    #[cfg(feature = "legacy")]
    {
        let _ = value_checked_cmd(serde_json::json!({}));
    }
}
