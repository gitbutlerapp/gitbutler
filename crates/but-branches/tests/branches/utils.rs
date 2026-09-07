//! Scenario and metadata helpers, mirroring the ones used by `but-workspace` tests.
use but_core::ref_metadata::{ProjectMeta, StackId};
use but_meta::virtual_branches_legacy_types::{Stack, StackBranch};

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
pub enum StackState {
    /// The stack is applied.
    InWorkspace,
    /// The stack is known but not applied.
    Inactive,
}

/// Add a stack whose tip is `stack_name` with `segments` below it, in the given `state`.
pub fn add_stack_with_segments(
    db: &mut but_db::DbHandle,
    stack_id: u128,
    stack_name: &str,
    state: StackState,
    segments: &[&str],
) -> StackId {
    let mut stack = Stack::new_with_just_heads(
        segments
            .iter()
            .rev()
            .map(|segment_name| {
                StackBranch::new_with_zero_head((*segment_name).into(), None, None, false)
            })
            .chain(std::iter::once(StackBranch::new_with_zero_head(
                stack_name.into(),
                None,
                None,
                false,
            )))
            .collect(),
        db.virtual_branches()
            .get_snapshot()
            .unwrap()
            .map_or(0, |snapshot| snapshot.stacks.len()),
        match state {
            StackState::InWorkspace => true,
            StackState::Inactive => false,
        },
    );
    stack.order = stack_id as usize;
    let stack_id = StackId::from_number_for_testing(stack_id);
    stack.id = stack_id;
    but_testsupport::edit_legacy_metadata(db, |data| data.branches.insert(stack_id, stack))
        .expect("fixture metadata can be saved");
    stack_id
}
