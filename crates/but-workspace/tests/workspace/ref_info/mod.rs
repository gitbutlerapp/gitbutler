#![expect(
    deprecated,
    reason = "covers calls to but_workspace::legacy::stacks_v3 and but_workspace::legacy::stack_details_v3"
)]

use snapbox::prelude::*;
use std::borrow::Cow;

use bstr::ByteSlice;
use but_core::{RefMetadata, WORKSPACE_REF_NAME, ref_metadata::StackId};
use but_workspace::{legacy::StacksFilter, ref_info};
use gix::prelude::ObjectIdExt;

use crate::ref_info::utils::{
    named_writable_scenario_with_args, read_only_in_memory_scenario, standard_options,
};

/// All tests that use a workspace commit for a fully managed, explicit workspace.
pub(crate) mod with_workspace_commit;

pub fn head_info(
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
    mut opts: but_workspace::ref_info::Options,
) -> anyhow::Result<but_workspace::RefInfo> {
    opts.project_meta = meta
        .workspace(WORKSPACE_REF_NAME.try_into()?)?
        .project_meta();
    but_workspace::head_info(repo, meta, opts)
}

#[deprecated(
    note = "Use head_info() and the returned RefInfo instead. Callers that already have a Context should prefer ctx.workspace_* helpers."
)]
pub fn stacks_v3(
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
    filter: StacksFilter,
    ref_name_override: Option<&gix::refs::FullNameRef>,
) -> anyhow::Result<Vec<but_workspace::legacy::ui::StackEntry>> {
    but_workspace::legacy::stacks_v3(
        repo,
        meta,
        &meta
            .workspace(WORKSPACE_REF_NAME.try_into()?)?
            .project_meta(),
        filter,
        ref_name_override,
    )
}

#[deprecated(
    note = "Use head_info() and the returned RefInfo instead. Callers that already have a Context should prefer ctx.workspace_* helpers."
)]
pub fn stack_details_v3(
    stack_id: Option<StackId>,
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
) -> anyhow::Result<but_workspace::ui::StackDetails> {
    but_workspace::legacy::stack_details_v3(
        stack_id,
        repo,
        meta,
        &meta
            .workspace(WORKSPACE_REF_NAME.try_into()?)?
            .project_meta(),
    )
}

fn first_commit(info: &but_workspace::RefInfo) -> &but_workspace::ref_info::LocalCommit {
    &info.stacks[0].segments[0].commits[0]
}

#[test]
fn commit_change_id_derives_fallback_for_headerless_commit() -> anyhow::Result<()> {
    let (repo, _meta) = read_only_in_memory_scenario("single-branch-10-commits")?;
    let commit = but_core::Commit::from_id(repo.head_commit()?.id())?;
    let commit = but_workspace::ref_info::Commit::from(commit);

    assert_eq!(commit.change_id, None);
    let actual = commit.change_id();
    assert_eq!(
        actual.as_ref(),
        &but_core::commit::Headers::synthetic_change_id_from_commit_id(commit.id)
    );
    assert!(
        matches!(actual, Cow::Owned(_)),
        "owned because it was created on the fly"
    );

    Ok(())
}

#[test]
fn commit_header_change_id_is_preferred_to_synthetic_fallback() -> anyhow::Result<()> {
    let (repo, meta) =
        crate::ref_info::with_workspace_commit::utils::named_read_only_in_memory_scenario(
            "journey03",
            "01-with-local-amended-after-integration",
        )?;
    let commit_id = repo.find_reference("A")?.peel_to_id()?.detach();
    let header_change_id = but_core::Commit::from_id(commit_id.attach(&repo))?
        .headers()
        .and_then(|headers| headers.change_id)
        .expect("fixture commit has change id in header");
    let info = but_workspace::ref_info(repo.find_reference("A")?, &*meta, standard_options())?;
    let commit = first_commit(&info);

    assert_eq!(commit.id, commit_id);
    assert_eq!(commit.change_id, Some(header_change_id));

    Ok(())
}

#[test]
fn commit_change_id_prefers_stored_header_value() -> anyhow::Result<()> {
    let (repo, _meta) =
        crate::ref_info::with_workspace_commit::utils::named_read_only_in_memory_scenario(
            "journey03",
            "01-with-local-amended-after-integration",
        )?;
    let commit_id = repo.find_reference("A")?.peel_to_id()?;
    let commit = but_workspace::ref_info::Commit::from(but_core::Commit::from_id(commit_id)?);
    let header_change_id = commit
        .change_id
        .as_ref()
        .expect("fixture commit has change id");

    let actual = commit.change_id();
    assert_eq!(actual.as_ref(), header_change_id);
    assert!(
        matches!(actual, Cow::Borrowed(_)),
        "borrowed because it's stored on the commit"
    );

    Ok(())
}

#[test]
fn unborn_untracked() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn-untracked")?;
    let info = head_info(&repo, &meta, standard_options())?;
    assert_eq!(
        info.stacks[0].segments[0].id, None,
        "the legacy empty segment has no backing graph node"
    );
    // It's clear that this branch is unborn as there is not a single commit,
    // in absence of a target ref.
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
RefInfo {
    workspace_ref_info: Some(
        RefInfo {
            ref_name: FullName(
                "refs/heads/main",
            ),
            commit_id: None,
            worktree: Some(
                Worktree {
                    kind: Main,
                    owned_by_repo: true,
                },
            ),
        },
    ),
    symbolic_remote_names: {},
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: None,
            segments: [
                ref_info::ui::Segment {
                    id: None,
                    ref_name: "►main[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "None",
                },
            ],
        },
    ],
    target_ref: None,
    target_commit: None,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
    );

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    // It's now possible to use the old API with unborn repos.
    // This type can't really represent missing tips, but `null()` will do.
    snapbox::assert_data_eq!(
        stacks.to_debug(),
        snapbox::str![[r#"
[
    StackEntry {
        id: None,
        heads: [
            StackHeadInfo {
                name: "main",
                tip: Sha1(0000000000000000000000000000000000000000),
                review_id: None,
                is_checked_out: false,
            },
        ],
        tip: Sha1(0000000000000000000000000000000000000000),
        order: None,
        is_checked_out: false,
    },
]

"#]]
    );

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    // It's also possible to obtain details.
    snapbox::assert_data_eq!(
        details.to_debug(),
        snapbox::str![[r#"
StackDetails {
    derived_name: "main",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "main",
            reference: FullName(
                "refs/heads/main",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(0000000000000000000000000000000000000000),
            base_commit: Sha1(0000000000000000000000000000000000000000),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [],
            is_conflicted: false,
            commits: [],
            upstream_commits: [],
            is_remote_head: false,
        },
    ],
    is_conflicted: false,
}

"#]]
    );
    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("one-commit-detached")?;
    let info = head_info(&repo, &meta, ref_info::Options::default())?;
    let commit_id = repo.head_commit()?.id().detach();
    let [stack] = info.stacks.as_slice() else {
        panic!("the detached commit should still project as one stack")
    };
    let [segment] = stack.segments.as_slice() else {
        panic!("the stack should contain the reference node that names its commit")
    };
    assert_eq!(
        segment
            .ref_info
            .as_ref()
            .map(|info| info.ref_name.to_string()),
        Some("refs/heads/main".to_owned()),
        "a detached entrypoint can still be represented by a reference pointing at its commit"
    );
    assert_eq!(
        segment
            .commits
            .iter()
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        vec![commit_id]
    );

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    let [stack] = stacks.as_slice() else {
        panic!("the legacy listing should expose the named reference")
    };
    let [head] = stack.heads.as_slice() else {
        panic!("the legacy stack should contain main")
    };
    assert_eq!(head.name.to_str()?, "main");
    assert_eq!(head.tip, commit_id);

    let details = stack_details_v3(stack.id, &repo, &meta)?;
    assert_eq!(details.derived_name, "main");
    let [branch] = details.branch_details.as_slice() else {
        panic!("the legacy details should expose main")
    };
    assert_eq!(branch.name.to_str()?, "main");
    assert_eq!(branch.reference.to_string(), "refs/heads/main");
    assert_eq!(branch.tip, commit_id);
    assert_eq!(
        branch
            .commits
            .iter()
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        vec![commit_id],
        "legacy details now follow the reference node instead of rejecting detached HEAD"
    );
    Ok(())
}

#[test]
fn detached_with_ambiguous_local_refs_stays_anonymous() -> anyhow::Result<()> {
    let (_tmp, repo, meta) =
        named_writable_scenario_with_args("one-commit-detached", std::iter::empty::<String>())?;
    let commit_id = repo.head_commit()?.id().detach();
    repo.reference(
        "refs/heads/also-main",
        commit_id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "create an ambiguous detached-head name",
    )?;

    let info = head_info(&repo, &meta, ref_info::Options::default())?;
    let [stack] = info.stacks.as_slice() else {
        panic!("the detached commit should still project as one stack")
    };
    let [segment] = stack.segments.as_slice() else {
        panic!("the detached commit should still project as one segment")
    };
    assert!(
        segment.ref_info.is_none(),
        "the compatibility boundary must not choose between ambiguous local refs"
    );
    let [commit] = segment.commits.as_slice() else {
        panic!("the detached one-commit fixture should expose its commit")
    };
    let mut local_refs = commit
        .refs
        .iter()
        .filter(|info| info.ref_name.category() == Some(gix::refs::Category::LocalBranch))
        .map(|info| info.ref_name.to_string())
        .collect::<Vec<_>>();
    local_refs.sort();
    assert_eq!(local_refs, ["refs/heads/also-main", "refs/heads/main"]);
    Ok(())
}

#[test]
fn conflicted_in_local_branch() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("with-conflict")?;
    let info = head_info(&repo, &meta, ref_info::Options::default())?;
    // The conflict is detected in the local commit.
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
RefInfo {
    workspace_ref_info: Some(
        RefInfo {
            ref_name: FullName(
                "refs/heads/main",
            ),
            commit_id: Some(
                Sha1(84503317a1e1464381fcff65ece14bc1f4315b7c),
            ),
            worktree: Some(
                Worktree {
                    kind: Main,
                    owned_by_repo: true,
                },
            ),
        },
    ),
    symbolic_remote_names: {},
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: None,
            segments: [
                👉ref_info::ui::Segment {
                    id: 2,
                    ref_name: "►main[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(💥8450331, "GitButler WIP Commit\n\n\n", local),
                        LocalCommit(a047f81, "init\n", local),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "None",
                },
            ],
        },
    ],
    target_ref: None,
    target_commit: None,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
        .raw()
    );

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    snapbox::assert_data_eq!(
        stacks.to_debug(),
        snapbox::str![[r#"
[
    StackEntry {
        id: None,
        heads: [
            StackHeadInfo {
                name: "main",
                tip: Sha1(84503317a1e1464381fcff65ece14bc1f4315b7c),
                review_id: None,
                is_checked_out: true,
            },
        ],
        tip: Sha1(84503317a1e1464381fcff65ece14bc1f4315b7c),
        order: None,
        is_checked_out: true,
    },
]

"#]]
    );

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    // The conflict is visible here as well.
    snapbox::assert_data_eq!(
        details.to_debug(),
        snapbox::str![[r#"
StackDetails {
    derived_name: "main",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "main",
            reference: FullName(
                "refs/heads/main",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(84503317a1e1464381fcff65ece14bc1f4315b7c),
            base_commit: Sha1(0000000000000000000000000000000000000000),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                GitButler <gitbutler@gitbutler.com>,
                author <author@example.com>,
            ],
            is_conflicted: true,
            commits: [
                Commit(8450331, "GitButler WIP Commit", local),
                Commit(a047f81, "init", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
    ],
    is_conflicted: true,
}

"#]]
    );
    Ok(())
}

#[test]
fn single_branch() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits")?;
    let info = head_info(&repo, &meta, standard_options())?;

    assert_eq!(
        info.stacks[0].segments.len(),
        1,
        "a single branch, a single segment"
    );
    assert!(
        info.stacks[0].segments[0].id.is_some(),
        "a born branch segment retains its backing graph node"
    );
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
RefInfo {
    workspace_ref_info: Some(
        RefInfo {
            ref_name: FullName(
                "refs/heads/main",
            ),
            commit_id: Some(
                Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
            ),
            worktree: Some(
                Worktree {
                    kind: Main,
                    owned_by_repo: true,
                },
            ),
        },
    ),
    symbolic_remote_names: {},
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: None,
            segments: [
                👉ref_info::ui::Segment {
                    id: 10,
                    ref_name: "►main[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(b5743a3, "10\n", local),
                        LocalCommit(344e320, "9\n", local),
                        LocalCommit(599c271, "8\n", local),
                        LocalCommit(05f069b, "7\n", local),
                        LocalCommit(c4f2a35, "6\n", local),
                        LocalCommit(44c12ce, "5\n", local),
                        LocalCommit(c584dbe, "4\n", local),
                        LocalCommit(281da94, "3\n", local),
                        LocalCommit(12995d7, "2\n", local),
                        LocalCommit(3d57fc1, "1\n", local),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "None",
                },
            ],
        },
    ],
    target_ref: None,
    target_commit: None,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
        .raw()
    );

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    snapbox::assert_data_eq!(
        stacks.to_debug(),
        snapbox::str![[r#"
[
    StackEntry {
        id: None,
        heads: [
            StackHeadInfo {
                name: "main",
                tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                review_id: None,
                is_checked_out: true,
            },
        ],
        tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
        order: None,
        is_checked_out: true,
    },
]

"#]]
    );

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    snapbox::assert_data_eq!(
        details.to_debug(),
        snapbox::str![[r#"
StackDetails {
    derived_name: "main",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "main",
            reference: FullName(
                "refs/heads/main",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
            base_commit: Sha1(0000000000000000000000000000000000000000),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(b5743a3, "10", local),
                Commit(344e320, "9", local),
                Commit(599c271, "8", local),
                Commit(05f069b, "7", local),
                Commit(c4f2a35, "6", local),
                Commit(44c12ce, "5", local),
                Commit(c584dbe, "4", local),
                Commit(281da94, "3", local),
                Commit(12995d7, "2", local),
                Commit(3d57fc1, "1", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
    ],
    is_conflicted: false,
}

"#]]
    );
    Ok(())
}

#[test]
fn single_branch_multiple_segments() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits-multi-segment")?;
    let info = head_info(&repo, &meta, standard_options())?;

    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
RefInfo {
    workspace_ref_info: Some(
        RefInfo {
            ref_name: FullName(
                "refs/heads/main",
            ),
            commit_id: Some(
                Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
            ),
            worktree: Some(
                Worktree {
                    kind: Main,
                    owned_by_repo: true,
                },
            ),
        },
    ),
    symbolic_remote_names: {},
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: None,
            segments: [
                👉ref_info::ui::Segment {
                    id: 11,
                    ref_name: "►main[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(b5743a3, "10\n", local, ►above-10),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "344e320",
                },
                ref_info::ui::Segment {
                    id: 12,
                    ref_name: "►nine",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(344e320, "9\n", local),
                        LocalCommit(599c271, "8\n", local),
                        LocalCommit(05f069b, "7\n", local),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "c4f2a35",
                },
                ref_info::ui::Segment {
                    id: 13,
                    ref_name: "►six",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(c4f2a35, "6\n", local),
                        LocalCommit(44c12ce, "5\n", local),
                        LocalCommit(c584dbe, "4\n", local),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "281da94",
                },
                ref_info::ui::Segment {
                    id: 14,
                    ref_name: "►three",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(281da94, "3\n", local),
                        LocalCommit(12995d7, "2\n", local),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "3d57fc1",
                },
                ref_info::ui::Segment {
                    id: 15,
                    ref_name: "►one",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(3d57fc1, "1\n", local),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "None",
                },
            ],
        },
    ],
    target_ref: None,
    target_commit: None,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
        .raw()
    );

    assert_eq!(info.stacks[0].segments.len(), 5, "multiple segments");

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    snapbox::assert_data_eq!(
        stacks.to_debug(),
        snapbox::str![[r#"
[
    StackEntry {
        id: None,
        heads: [
            StackHeadInfo {
                name: "main",
                tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                review_id: None,
                is_checked_out: true,
            },
            StackHeadInfo {
                name: "above-10",
                tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "nine",
                tip: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "six",
                tip: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "three",
                tip: Sha1(281da9454d5b41844d28e453e80b24925a7c8c7a),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "one",
                tip: Sha1(3d57fc18d679a1ba45bc7f79e394a5e2606719ee),
                review_id: None,
                is_checked_out: false,
            },
        ],
        tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
        order: None,
        is_checked_out: true,
    },
]

"#]]
    );

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    // It also works with multiple segments.
    snapbox::assert_data_eq!(
        details.to_debug(),
        snapbox::str![[r#"
StackDetails {
    derived_name: "main",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "main",
            reference: FullName(
                "refs/heads/main",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
            base_commit: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(b5743a3, "10", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "nine",
            reference: FullName(
                "refs/heads/nine",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
            base_commit: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(344e320, "9", local),
                Commit(599c271, "8", local),
                Commit(05f069b, "7", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "six",
            reference: FullName(
                "refs/heads/six",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
            base_commit: Sha1(281da9454d5b41844d28e453e80b24925a7c8c7a),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(c4f2a35, "6", local),
                Commit(44c12ce, "5", local),
                Commit(c584dbe, "4", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "three",
            reference: FullName(
                "refs/heads/three",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(281da9454d5b41844d28e453e80b24925a7c8c7a),
            base_commit: Sha1(3d57fc18d679a1ba45bc7f79e394a5e2606719ee),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(281da94, "3", local),
                Commit(12995d7, "2", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "one",
            reference: FullName(
                "refs/heads/one",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(3d57fc18d679a1ba45bc7f79e394a5e2606719ee),
            base_commit: Sha1(0000000000000000000000000000000000000000),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(3d57fc1, "1", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
    ],
    is_conflicted: false,
}

"#]]
    );
    Ok(())
}

mod utils {
    use but_meta::VirtualBranchesTomlMetadata;
    use but_testsupport::gix_testtools::tempfile;
    use but_workspace::ref_info;

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        named_read_only_in_memory_scenario(name, "")
    }

    pub fn named_read_only_in_memory_scenario(
        script: &str,
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let repo = crate::utils::read_only_in_memory_scenario_named(script, name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, std::mem::ManuallyDrop::new(meta)))
    }

    pub fn named_writable_scenario_with_args(
        name: &str,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> anyhow::Result<(
        tempfile::TempDir,
        gix::Repository,
        VirtualBranchesTomlMetadata,
    )> {
        let (repo, tmp) = crate::utils::writable_scenario_with_args(name, args);
        let meta =
            VirtualBranchesTomlMetadata::from_path(repo.path().join("virtual-branches.toml"))?;
        Ok((tmp, repo, meta))
    }

    pub fn standard_options() -> but_workspace::ref_info::Options<'static> {
        ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        }
    }
}
