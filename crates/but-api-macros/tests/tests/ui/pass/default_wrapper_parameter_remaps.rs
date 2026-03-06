// Case: non-legacy wrapper remaps added for branch-style APIs.
// It verifies:
// - `&gix::refs::FullNameRef` -> owned `gix::refs::FullName`
// - `Option<gix::ObjectId>` -> `Option<json::HexHash>`
// - `Vec<gix::ObjectId>` -> `Vec<json::HexHash>`
// in `_json`, and in `_cmd` when `legacy` is enabled.

use but_api_macros::but_api;

pub use but_api_macros_tests::{json, oid_from_hex, panic_capture};

#[but_api]
pub fn wrapper_surface(
    existing_branch: &gix::refs::FullNameRef,
    maybe_commit: Option<gix::ObjectId>,
    commit_ids: Vec<gix::ObjectId>,
) -> anyhow::Result<String> {
    let _ = existing_branch;
    Ok(format!("{}:{}", maybe_commit.is_some(), commit_ids.len()))
}

fn main() {
    let existing_branch: gix::refs::FullName = "refs/heads/topic".try_into().unwrap();
    let maybe_commit =
        json::HexHash::from(oid_from_hex("0123456789abcdef0123456789abcdef01234567"));
    let commit_ids = vec![
        json::HexHash::from(oid_from_hex("1111111111111111111111111111111111111111")),
        json::HexHash::from(oid_from_hex("2222222222222222222222222222222222222222")),
    ];

    let _ = wrapper_surface_json(
        existing_branch.clone(),
        Some(maybe_commit.clone()),
        commit_ids.clone(),
    );

    #[cfg(feature = "legacy")]
    {
        let _ = wrapper_surface_cmd(serde_json::json!({
            "existingBranch": existing_branch.to_string(),
            "maybeCommit": maybe_commit.to_string(),
            "commitIds": commit_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }));
    }
}
