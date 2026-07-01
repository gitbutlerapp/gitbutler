// Case: methods with `self` receivers are unsupported by this macro.
// Extend when: method receiver support (`self`, `&self`, `&mut self`) is implemented.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

struct Api;

impl Api {
    #[but_api]
    fn method(&self, value: i32) -> anyhow::Result<i32> {
        Ok(value)
    }
}

fn main() {}
