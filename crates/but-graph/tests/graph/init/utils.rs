use but_graph::VirtualBranchesTomlMetadata;
use gitbutler_stack::{StackId, Target};
use gix::Repository;

pub fn read_only_in_memory_scenario(
    name: &str,
) -> anyhow::Result<(
    gix::Repository,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
)> {
    named_read_only_in_memory_scenario("scenarios", name)
}

fn named_read_only_in_memory_scenario(
    script: &str,
    name: &str,
) -> anyhow::Result<(
    gix::Repository,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
)> {
    let repo = read_only_in_memory_scenario_named(script, name)?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        repo.path()
            .join(".git")
            .join("should-never-be-written.toml"),
    )?;
    Ok((repo, std::mem::ManuallyDrop::new(meta)))
}

/// Provide a scenario but assure the returned repository will write objects to memory, in a subdirectory `dirname`.
pub fn read_only_in_memory_scenario_named(
    script_name: &str,
    dirname: &str,
) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only(format!("{script_name}.sh"))
        .map_err(anyhow::Error::from_boxed)?;
    let repo =
        gix::open_opts(root.join(dirname), gix::open::Options::isolated())?.with_object_memory();
    Ok(repo)
}

pub enum StackState {
    #[allow(dead_code)]
    InWorkspace,
    Inactive,
}

pub fn add_workspace(meta: &mut VirtualBranchesTomlMetadata) {
    add_stack(
        meta,
        StackId::from_number_for_testing(u128::MAX),
        "definitely outside of the workspace just to have it",
        StackState::Inactive,
    );
}

pub fn add_workspace_without_target(meta: &mut VirtualBranchesTomlMetadata) {
    add_workspace(meta);
    meta.data_mut().default_target = None;
}

pub fn add_stack(
    meta: &mut VirtualBranchesTomlMetadata,
    stack_id: StackId,
    stack_name: &str,
    state: StackState,
) -> StackId {
    add_stack_with_segments(meta, stack_id, stack_name, state, &[])
}

// Add parameters as needed.
pub fn add_stack_with_segments(
    meta: &mut VirtualBranchesTomlMetadata,
    stack_id: StackId,
    stack_name: &str,
    state: StackState,
    segments: &[&str],
) -> StackId {
    let mut stack = gitbutler_stack::Stack::new_with_just_heads(
        segments
            .iter()
            .rev()
            .map(|stack_name| {
                gitbutler_stack::StackBranch::new_with_zero_head(
                    (*stack_name).into(),
                    None,
                    None,
                    None,
                    false,
                )
            })
            .chain(std::iter::once(
                gitbutler_stack::StackBranch::new_with_zero_head(
                    stack_name.into(),
                    None,
                    None,
                    None,
                    false,
                ),
            ))
            .collect(),
        0,
        meta.data().branches.len(),
        match state {
            StackState::InWorkspace => true,
            StackState::Inactive => false,
        },
    );
    stack.id = stack_id;
    meta.data_mut().branches.insert(stack_id, stack);
    // Assure we have a target set.
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "does not matter".to_string(),
        sha: git2::Oid::zero(),
        push_remote_name: None,
    });
    stack_id
}

pub fn id_at<'repo>(repo: &'repo Repository, name: &str) -> (gix::Id<'repo>, gix::refs::FullName) {
    let mut rn = repo
        .find_reference(name)
        .expect("statically known reference exists");
    let id = rn.peel_to_id_in_place().expect("must be valid reference");
    (id, rn.inner.name)
}

pub fn id_by_rev<'repo>(repo: &'repo gix::Repository, rev: &str) -> gix::Id<'repo> {
    repo.rev_parse_single(rev)
        .expect("well-known revspec when testing")
}

pub fn standard_options() -> but_graph::init::Options {
    but_graph::init::Options {
        collect_tags: true,
        commits_limit_hint: None,
        commits_limit_recharge_location: vec![],
        hard_limit: None,
    }
}
