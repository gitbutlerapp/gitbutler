use but_core::ref_metadata::{ProjectMeta, StackId};
use but_meta::{
    VirtualBranchesTomlMetadata,
    virtual_branches_legacy_types::{Stack, StackBranch},
};
use but_testsupport::gix_testtools::scripted_fixture_read_only;

pub fn read_only_in_memory_scenario(
    name: &str,
) -> anyhow::Result<(
    gix::Repository,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    but_db::DbHandle,
)> {
    named_read_only_in_memory_scenario("scenarios", name)
}

pub fn named_read_only_in_memory_scenario(
    script: &str,
    name: &str,
) -> anyhow::Result<(
    gix::Repository,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    but_db::DbHandle,
)> {
    let repo = read_only_in_memory_scenario_named(script, name)?;
    let meta = in_memory_meta(repo.path().join(".git"))?;
    // The fixture is shared and read-only, so its database cannot live on disk.
    let db = but_testsupport::in_memory_db();
    Ok((repo, meta, db))
}

pub fn in_memory_meta(
    dir: impl AsRef<std::path::Path>,
) -> anyhow::Result<std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>> {
    let meta =
        VirtualBranchesTomlMetadata::from_path(dir.as_ref().join("should-never-be-written.toml"))?;
    Ok(std::mem::ManuallyDrop::new(meta))
}

/// Provide a scenario but assure the returned repository will write objects to memory, in a subdirectory `dirname`.
pub fn read_only_in_memory_scenario_named(
    script_name: &str,
    dirname: &str,
) -> anyhow::Result<gix::Repository> {
    let root = scripted_fixture_read_only(format!("{script_name}.sh"))
        .map_err(anyhow::Error::from_boxed)?;
    let repo =
        gix::open_opts(root.join(dirname), gix::open::Options::isolated())?.with_object_memory();
    Ok(repo)
}

pub enum StackState {
    InWorkspace,
    Inactive,
}

pub fn add_workspace(meta: &mut VirtualBranchesTomlMetadata) {
    add_stack(
        meta,
        usize::MAX,
        "definitely-outside-of-the-workspace-just-to-have-it",
        StackState::Inactive,
    );
}

pub fn add_workspace_with_target(
    meta: &mut VirtualBranchesTomlMetadata,
    target_commit: impl Into<gix::ObjectId>,
) -> ProjectMeta {
    add_stack(
        meta,
        usize::MAX,
        "definitely-outside-of-the-workspace-just-to-have-it",
        StackState::Inactive,
    );
    ProjectMeta {
        target_commit_id: Some(target_commit.into()),
        ..default_project_meta()
    }
}

pub fn default_project_meta() -> ProjectMeta {
    ProjectMeta {
        target_ref: Some(
            "refs/remotes/origin/main"
                .try_into()
                .expect("statically known to be valid"),
        ),
        ..Default::default()
    }
}

pub fn add_stack(
    meta: &mut VirtualBranchesTomlMetadata,
    stack_id: usize,
    stack_name: &str,
    state: StackState,
) -> StackId {
    add_stack_with_segments(meta, stack_id, stack_name, state, &[])
}

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
    stack_id
}

pub fn standard_options() -> but_graph::init::Options {
    but_graph::init::Options {
        collect_tags: true,
        commits_limit_hint: None,
        commits_limit_recharge_location: vec![],
        hard_limit: None,
        extra_target_commit_id: None,
        dangerously_skip_postprocessing_for_debugging: false,
        worktrees: false,
    }
}

pub fn standard_options_with_extra_target(
    repo: &gix::Repository,
    name: &str,
) -> but_graph::init::Options {
    but_graph::init::Options {
        extra_target_commit_id: Some(repo.rev_parse_single(name).expect("present").detach()),
        ..standard_options()
    }
}

pub use but_testsupport::{id_at, id_by_rev};
