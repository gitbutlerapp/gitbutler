// Case: `Result<Vec<T>>` plus explicit `From` conversion.
// Extend when: collection conversion rules or output typing for vectors changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{UiValue, UiValueTry, json, panic_capture};

#[but_api(UiValue)]
pub fn values() -> anyhow::Result<Vec<i32>> {
    Ok(vec![1, 2])
}

#[but_api(try_from = UiValueTry)]
pub fn checked_values() -> anyhow::Result<Vec<i32>> {
    Ok(vec![1, 2])
}

// A single-parameter `Result` alias in scope must not break the generated
// `collect()`, which is why the expansion names the standard `Result` in full.
mod aliased_result {
    use super::*;

    #[allow(dead_code)]
    type Result<T> = anyhow::Result<T>;

    #[but_api(try_from = UiValueTry)]
    pub fn aliased_values() -> anyhow::Result<Vec<i32>> {
        Ok(vec![1, 2])
    }
}

fn main() {
    let _: Result<Vec<UiValue>, _> = values_json();
    let _: Result<Vec<UiValueTry>, _> = checked_values_json();
    let _: Result<Vec<UiValueTry>, _> = aliased_result::aliased_values_json();
    #[cfg(feature = "legacy")]
    {
        let _ = values_cmd(serde_json::json!({}));
        let _ = checked_values_cmd(serde_json::json!({}));
    }
}
