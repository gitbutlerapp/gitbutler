// Case: `#[but_api(Type)]` using infallible `From` conversion for result values.
// Extend when: conversion mode selection, or converted JSON return typing, changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{UiValue, json, panic_capture};

#[but_api(UiValue)]
pub fn value() -> anyhow::Result<i32> {
    Ok(42)
}

fn main() {
    let _ = value_json();
    #[cfg(feature = "legacy")]
    {
        let _ = value_cmd(serde_json::json!({}));
    }
}
