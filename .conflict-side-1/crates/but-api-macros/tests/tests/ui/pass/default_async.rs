// Case: async function expansion with panic-capture wrapper.
// It verifies async `<fn>_json` and, under `legacy`, async `<fn>_cmd` generation.
// Extend when: async wrapper call path or panic handling integration changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api]
pub async fn sum_async(left: i32, right: i32) -> anyhow::Result<i32> {
    Ok(left + right)
}

fn main() {
    let _future = sum_async_json(1, 2);
    #[cfg(feature = "legacy")]
    {
        let _future = sum_async_cmd(serde_json::json!({ "left": 1, "right": 2 }));
    }
}
