//! Integration tests for the graph edit lifecycle:
//! `NodeGraph -> into_mut() -> MutableNodeGraph -> rebase() -> Rebased -> materialize_changes()`.
//!
//! Ported 1:1 from `but-rebase/tests/rebase/graph_rebase`.
use but_core::ref_metadata::StackId;
use but_meta::{
    VirtualBranchesTomlMetadata,
    virtual_branches_legacy_types::{Stack, StackBranch, Target},
};
use but_testsupport::StackState;

mod change_id;
mod cherry_pick;
mod conflictable_restriction;
mod disconnect;
mod edge;
mod editor_creation;
mod empty_lane;
mod graph_workspace;
mod insert;
mod insert_segment;
mod materialize;
mod merge_commit_changes;
mod multiple_operations;
mod order_commit_selectors_by_parentage;
mod rebase_identities;
mod replace;
mod sha256;
mod signing_preferences;
mod workspace_commit_behaviour;

// Add parameters as needed.
pub fn add_stack_with_segments(
    meta: &mut VirtualBranchesTomlMetadata,
    stack_id: usize,
    stack_name: &str,
    state: StackState,
    segments: &[&str],
) -> StackId {
    let mut stack = Stack::new_with_just_heads(
        segments
            .iter()
            .rev()
            .map(|stack_name| {
                StackBranch::new_with_zero_head((*stack_name).into(), None, None, false)
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
    stack.order = stack_id;
    let stack_id = StackId::from_number_for_testing(stack_id as u128);
    stack.id = stack_id;
    meta.data_mut().branches.insert(stack_id, stack);
    // Assure we have a target set.
    if meta.data_mut().default_target.is_none() {
        meta.data_mut().default_target = Some(Target {
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            remote_url: "does not matter".to_string(),
            sha: gix::hash::Kind::Sha1.null(),
            push_remote_name: None,
        });
    }
    stack_id
}

/// Extract the pick's commit id, panicking if the node doesn't read as a pick.
pub fn pick_id(pick: Option<but_graph::edit::Pick>) -> gix::ObjectId {
    match pick {
        Some(pick) => pick.id,
        None => panic!("expected a pick node"),
    }
}

pub mod utils {
    use anyhow::Result;
    use but_meta::VirtualBranchesTomlMetadata;

    /// Returns a fixture that may not be written to, objects will never touch disk either.
    pub fn fixture(
        fixture_name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let repo = but_testsupport::read_only_in_memory_scenario(fixture_name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, std::mem::ManuallyDrop::new(meta)))
    }

    /// Returns a fixture that may be written to.
    pub fn fixture_writable(
        fixture_name: &str,
    ) -> Result<(
        gix::Repository,
        tempfile::TempDir,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let (repo, tmp) = but_testsupport::writable_scenario(fixture_name);
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, tmp, std::mem::ManuallyDrop::new(meta)))
    }

    /// Returns a fixture that may be written to.
    pub fn fixture_writable_with_signing(
        fixture_name: &str,
    ) -> Result<(
        gix::Repository,
        tempfile::TempDir,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let (repo, tmp) = but_testsupport::writable_scenario_with_ssh_key(fixture_name);
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, tmp, std::mem::ManuallyDrop::new(meta)))
    }
}
