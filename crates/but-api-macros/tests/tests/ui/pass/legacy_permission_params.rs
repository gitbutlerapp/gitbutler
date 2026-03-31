// Case: explicit repository permission tokens are hidden from generated legacy wrappers.
// It verifies:
// - `&mut RepoExclusive` acquires exclusive access internally
// - `&RepoShared` acquires shared access internally
// - generated `_json` and `_cmd` entrypoints do not expose the permission parameter

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, panic_capture};

#[but_api]
pub fn exclusive_surface(
    _ctx: &mut but_ctx::Context,
    value: i32,
    _perm: &mut but_ctx::access::RepoExclusive,
) -> anyhow::Result<i32> {
    Ok(value)
}

#[but_api]
pub fn shared_surface(
    _perm: &but_ctx::access::RepoShared,
    _ctx: &but_ctx::Context,
    value: i32,
) -> anyhow::Result<i32> {
    Ok(value)
}

fn main() {
    let project_id: but_ctx::LegacyProjectId =
        "d7377618-b9cd-4964-a3c3-05c58ed5602b".parse().unwrap();
    let project = but_ctx::ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id.clone());

    let _ = exclusive_surface_json(project.clone(), 1);
    let _ = shared_surface_json(project.clone(), 2);

    let _ = exclusive_surface_cmd(
        serde_json::json!({ "projectId": project_id.to_string(), "value": 1 }),
    );
    let _ = shared_surface_cmd(serde_json::json!({ "projectId": project_id.to_string(), "value": 2 }));
}
