// Case: `_cmd` wrappers for context-remapped functions remain legacy-only.
// It verifies:
// - calling `<fn>_cmd` without `legacy` fails to compile
// - the legacy feature gate on generated cmd entry points is preserved
// Extend when: cmd generation feature-gating or naming changes.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api]
pub fn with_context(_ctx: but_ctx::Context, value: i32) -> anyhow::Result<i32> {
    Ok(value)
}

fn main() {
    let _ = with_context_cmd(serde_json::json!({
        "projectId": "L3RtcC9yZXBv",
        "value": 1
    }));
}
