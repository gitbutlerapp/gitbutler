// Case: legacy-only parameter remapping for context/object-id style inputs.
// It verifies:
// - `Context`/`&Context`/`&mut Context`/`ThreadSafeContext` -> `project_id` remapping
// - `gix::ObjectId` -> `json::HexHash` remapping
// in both `_json` and `_cmd` wrappers.
// Extend when: supported context-like input forms, parameter naming, or remap types change.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, oid_from_hex, panic_capture};

#[but_api]
pub fn with_context_and_oid(
    _ctx: but_ctx::Context,
    commit: gix::ObjectId,
) -> anyhow::Result<json::HexHash> {
    Ok(json::HexHash::from(commit))
}

#[but_api]
pub fn with_context_ref(_ctx: &but_ctx::Context, value: i32) -> anyhow::Result<i32> {
    Ok(value)
}

#[but_api]
pub fn with_context_mut(_ctx: &mut but_ctx::Context, value: i32) -> anyhow::Result<i32> {
    Ok(value)
}

#[but_api]
pub fn with_thread_safe_context(
    _ctx: but_ctx::ThreadSafeContext,
    value: i32,
) -> anyhow::Result<i32> {
    Ok(value)
}

fn main() {
    let project_id: but_ctx::LegacyProjectId =
        "d7377618-b9cd-4964-a3c3-05c58ed5602b".parse().unwrap();
    let oid = oid_from_hex("0123456789abcdef0123456789abcdef01234567");
    let hash = json::HexHash::from(oid);

    let _ = with_context_and_oid_json(project_id.clone(), hash.clone());
    let _ = with_context_ref_json(project_id.clone(), 1);
    let _ = with_context_mut_json(project_id.clone(), 2);
    let _ = with_thread_safe_context_json(project_id.clone(), 3);

    let _ = with_context_and_oid_cmd(
        serde_json::json!({ "projectId": project_id.to_string(), "commit": hash.to_string() }),
    );
    let _ = with_context_ref_cmd(
        serde_json::json!({ "projectId": project_id.to_string(), "value": 1 }),
    );
    let _ = with_context_mut_cmd(
        serde_json::json!({ "projectId": project_id.to_string(), "value": 2 }),
    );
    let _ = with_thread_safe_context_cmd(
        serde_json::json!({ "projectId": project_id.to_string(), "value": 3 }),
    );
}
