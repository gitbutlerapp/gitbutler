// Case: `Result<Option<T>>` plus explicit `From` conversion.
// It verifies option-aware conversion (`Option<T>` -> `Option<JsonT>`) is generated.
// Extend when: option conversion rules or output typing for optional returns changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{UiValue, json, panic_capture};

#[but_api(UiValue)]
pub fn maybe_value(flag: bool) -> anyhow::Result<Option<i32>> {
    Ok(flag.then_some(9))
}

fn main() {
    let _ = maybe_value_json(true);
    #[cfg(feature = "legacy")]
    {
        let _ = maybe_value_cmd(serde_json::json!({ "flag": true }));
    }
}
