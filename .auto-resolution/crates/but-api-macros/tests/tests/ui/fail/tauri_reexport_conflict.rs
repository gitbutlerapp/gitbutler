// Case: tauri-enabled expansion currently conflicts for this shape (`__cmd__*` re-export).
// This snapshots current compile failure to make future behavior changes intentional.
// Extend when: tauri module/export generation semantics are changed or fixed.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api]
pub fn ping(name: String) -> anyhow::Result<String> {
    Ok(name)
}

fn main() {
    let _ = tauri_ping::ping;
}
