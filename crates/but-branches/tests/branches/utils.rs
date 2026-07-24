//! Scenario and metadata helpers, mirroring the ones used by `but-workspace` tests.
use but_core::ref_metadata::{ProjectMeta, StackId};
use but_meta::{
    VirtualBranchesTomlMetadata,
    virtual_branches_legacy_types::{Stack, StackBranch},
};

/// Open the read-only fixture `name` with an in-memory metadata store.
pub fn named_read_only_in_memory_scenario(
    name: &str,
    dirname: &str,
) -> anyhow::Result<(
    gix::Repository,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
)> {
    let repo = but_testsupport::read_only_in_memory_scenario_named(name, dirname)?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        repo.path()
            .join(".git")
            .join("should-never-be-written.toml"),
    )?;
    Ok((repo, std::mem::ManuallyDrop::new(meta)))
}

/// Project metadata whose target is `refs/remotes/origin/main`, pinned to the
/// commit that same ref points to.
pub fn project_meta_with_target(repo: &gix::Repository) -> anyhow::Result<ProjectMeta> {
    const TARGET_REF: &str = "refs/remotes/origin/main";
    Ok(ProjectMeta {
        target_ref: Some(TARGET_REF.try_into()?),
        // Peel the target ref itself: deriving the commit from a different ref,
        // such as a local `main`, would silently disagree with `target_ref` in a
        // fixture where the two diverge or the local branch is absent.
        target_commit_id: repo
            .try_find_reference(TARGET_REF)?
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
    meta: &mut VirtualBranchesTomlMetadata,
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
        meta.data().branches.len(),
        match state {
            StackState::InWorkspace => true,
            StackState::Inactive => false,
        },
    );
    stack.order = stack_id as usize;
    let stack_id = StackId::from_number_for_testing(stack_id);
    stack.id = stack_id;
    meta.data_mut().branches.insert(stack_id, stack);
    stack_id
}
