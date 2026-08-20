use snapbox::prelude::*;
use std::borrow::Cow;

use but_core::WORKSPACE_REF_NAME;
use but_workspace::ref_info;
use gix::prelude::ObjectIdExt;

use crate::ref_info::utils::{read_only_in_memory_scenario, standard_options};

/// All tests that use a workspace commit for a fully managed, explicit workspace.
pub(crate) mod with_workspace_commit;

pub fn head_info(
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
    db: &mut but_db::DbHandle,
    mut opts: but_workspace::ref_info::Options,
) -> anyhow::Result<but_workspace::RefInfo> {
    if opts.project_meta == Default::default() {
        opts.project_meta = project_meta(repo)?;
    }
    but_workspace::head_info(repo, meta, db, opts)
}

fn project_meta(repo: &gix::Repository) -> anyhow::Result<but_core::ref_metadata::ProjectMeta> {
    let project_meta = but_core::ref_metadata::ProjectMeta::resolve(repo)?;
    if project_meta == Default::default() && repo.try_find_reference(WORKSPACE_REF_NAME)?.is_some()
    {
        with_workspace_commit::utils::project_meta(repo)
    } else {
        Ok(project_meta)
    }
}

fn first_commit(info: &but_workspace::RefInfo) -> &but_workspace::ref_info::LocalCommit {
    &info.stacks[0].segments[0].commits[0]
}

#[test]
fn commit_change_id_derives_fallback_for_headerless_commit() -> anyhow::Result<()> {
    let (repo, _meta, _db) = read_only_in_memory_scenario("single-branch-10-commits")?;
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
    let (repo, meta, mut db) =
        crate::ref_info::with_workspace_commit::utils::named_read_only_in_memory_scenario(
            "journey03",
            "01-with-local-amended-after-integration",
        )?;
    let commit_id = repo.find_reference("A")?.peel_to_id()?.detach();
    let header_change_id = but_core::Commit::from_id(commit_id.attach(&repo))?
        .headers()
        .and_then(|headers| headers.change_id)
        .expect("fixture commit has change id in header");
    let info = but_workspace::ref_info(
        repo.find_reference("A")?,
        &*meta,
        &mut db,
        standard_options(),
    )?;
    let commit = first_commit(&info);

    assert_eq!(commit.id, commit_id);
    assert_eq!(commit.change_id, Some(header_change_id));

    Ok(())
}

#[test]
fn commit_change_id_prefers_stored_header_value() -> anyhow::Result<()> {
    let (repo, _meta, _db) =
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
    let (repo, meta, mut db) = read_only_in_memory_scenario("unborn-untracked")?;
    let info = head_info(&repo, &meta, &mut db, standard_options())?;
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
                    id: NodeIndex(0),
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
    is_target_current: false,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
    );

    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("one-commit-detached")?;
    let info = head_info(&repo, &meta, &mut db, ref_info::Options::default())?;
    // As the workspace name is derived from the first segment, it's empty as well.
    // We do know that `main` is pointing at the local commit though, despite the unnamed segment owning it.
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
RefInfo {
    workspace_ref_info: None,
    symbolic_remote_names: {},
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: None,
            segments: [
                ref_info::ui::Segment {
                    id: NodeIndex(0),
                    ref_name: "None",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(15bcd1b, "init\n", local, ►main),
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
    is_target_current: false,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn conflicted_in_local_branch() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("with-conflict")?;
    let info = head_info(&repo, &meta, &mut db, ref_info::Options::default())?;
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
                ref_info::ui::Segment {
                    id: NodeIndex(0),
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
    is_target_current: false,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn single_branch() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("single-branch-10-commits")?;
    let info = head_info(&repo, &meta, &mut db, standard_options())?;

    assert_eq!(
        info.stacks[0].segments.len(),
        1,
        "a single branch, a single segment"
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
                ref_info::ui::Segment {
                    id: NodeIndex(0),
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
    is_target_current: false,
    lower_bound: None,
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn single_branch_multiple_segments() -> anyhow::Result<()> {
    let (repo, meta, mut db) =
        read_only_in_memory_scenario("single-branch-10-commits-multi-segment")?;
    let info = head_info(&repo, &meta, &mut db, standard_options())?;

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
                ref_info::ui::Segment {
                    id: NodeIndex(0),
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
                    id: NodeIndex(1),
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
                    id: NodeIndex(2),
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
                    id: NodeIndex(3),
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
                    id: NodeIndex(4),
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
    is_target_current: false,
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
        but_db::DbHandle,
    )> {
        named_read_only_in_memory_scenario(name, "")
    }

    pub fn named_read_only_in_memory_scenario(
        script: &str,
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
        but_db::DbHandle,
    )> {
        let repo = crate::utils::read_only_in_memory_scenario_named(script, name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        // The fixture is shared and read-only, so its database cannot live on disk.
        let db = but_testsupport::in_memory_db();
        Ok((repo, std::mem::ManuallyDrop::new(meta), db))
    }

    pub fn named_writable_scenario_with_args(
        name: &str,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> anyhow::Result<(
        tempfile::TempDir,
        gix::Repository,
        VirtualBranchesTomlMetadata,
        but_db::DbHandle,
    )> {
        let (repo, tmp) = crate::utils::writable_scenario_with_args(name, args);
        let meta =
            VirtualBranchesTomlMetadata::from_path(repo.path().join("virtual-branches.toml"))?;
        let db = but_testsupport::project_db(&repo)?;
        Ok((tmp, repo, meta, db))
    }

    pub fn standard_options() -> but_workspace::ref_info::Options<'static> {
        ref_info::Options {
            expensive_commit_info: true,
            traversal: Default::default(),
            ..Default::default()
        }
    }
}
