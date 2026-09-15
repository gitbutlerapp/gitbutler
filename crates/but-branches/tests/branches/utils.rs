//! Scenario and metadata helpers, mirroring the ones used by `but-workspace` tests.
use but_core::ref_metadata::ProjectMeta;

/// Open the read-only fixture `name` with an in-memory metadata store.
pub fn named_read_only_in_memory_scenario(
    name: &str,
    dirname: &str,
) -> anyhow::Result<(gix::Repository, but_db::DbHandle)> {
    let repo = but_testsupport::read_only_in_memory_scenario_named(name, dirname)?;
    // The fixture is shared and read-only, so its database cannot live on disk.
    let db = but_testsupport::in_memory_db();
    Ok((repo, db))
}

/// Project metadata whose target is `refs/remotes/origin/main`, pinned to the
/// commit `main` points to.
pub fn project_meta_with_target(repo: &gix::Repository) -> anyhow::Result<ProjectMeta> {
    Ok(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: repo
            .try_find_reference("main")?
            .map(|mut r| r.peel_to_id())
            .transpose()?
            .map(|id| id.detach()),
        ..Default::default()
    })
}

/// Whether a stack participates in the workspace.
pub use but_testsupport::{StackState, add_stack_with_segments};
