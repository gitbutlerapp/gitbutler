use std::borrow::Cow;

use bstr::ByteSlice;
use but_core::{RefMetadata, WORKSPACE_REF_NAME, ref_metadata::StackKind};
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::{
    CommandExt as _, git, gix_testtools::tempfile::TempDir, graph_workspace_determinisitcally,
    visualize_commit_graph_all,
};
use but_workspace::branch::apply::OutcomeStatus;

use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments, graph, named_writable_scenario,
    },
    utils::r,
};

#[derive(Default)]
struct Options {
    // Also include an unapplied branch "C".
    unapplied_branch: bool,
    // Also include an unapplied branch "conflicting", which conflicts with "A".
    unapplied_conflicting_branch: bool,
}

/// `A` and `B` are applied.
fn two_stacks(
    Options {
        unapplied_branch,
        unapplied_conflicting_branch,
    }: Options,
) -> anyhow::Result<(
    TempDir,
    gix::Repository,
    VirtualBranchesTomlMetadata,
    but_db::DbHandle,
)> {
    let (tmp, repo, mut meta, db) =
        named_writable_scenario("ws-two-stacks-with-unapplied-branches")?;
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    if !unapplied_branch {
        git(&repo).args(["branch", "-D", "C"]).run();
    }
    if !unapplied_conflicting_branch {
        git(&repo).args(["branch", "-D", "conflicting"]).run();
    }
    Ok((tmp, repo, meta, db))
}

fn workspace(
    repo: &gix::Repository,
    meta: &VirtualBranchesTomlMetadata,
    db: &mut but_db::DbHandle,
) -> anyhow::Result<but_graph::Workspace> {
    graph(repo, meta, db)?.into_workspace()
}

fn assert_worktree_files(repo: &gix::Repository, present: &[&str], absent: &[&str]) {
    for path in present {
        let path = repo.workdir_path(path).expect("non-bare");
        assert!(
            path.exists(),
            "{} should exist in the worktree",
            path.display()
        );
    }
    for path in absent {
        let path = repo.workdir_path(path).expect("non-bare");
        assert!(
            !path.exists(),
            "{} should not exist in the worktree",
            path.display()
        );
    }
}

/// The configuration of the local `branch`, empty if there is none.
fn branch_config(repo: &gix::Repository, branch: &str) -> anyhow::Result<String> {
    let output = but_testsupport::git(repo)
        .args(["config", "--get-regexp", &format!("^branch\\.{branch}\\.")])
        .output()?;
    Ok(output.stdout.to_str_lossy().into_owned())
}

/// The stacks the workspace metadata records as applied.
fn applied_stacks_in_metadata(meta: &VirtualBranchesTomlMetadata) -> anyhow::Result<Vec<String>> {
    Ok(meta
        .workspace(r(WORKSPACE_REF_NAME))?
        .stacks(StackKind::Applied)
        .filter_map(|stack| stack.name().map(|name| name.shorten().to_string()))
        .collect())
}

#[test]
fn apply_adds_a_branch_as_the_last_lane() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Options {
        unapplied_branch: true,
        ..Default::default()
    })?;

    assert_worktree_files(&repo, &["a", "b"], &["c"]);
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(r("refs/heads/C"), ws, &repo, &mut meta, &mut db)?;

    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "C wasn't applied before"
    );
    assert_eq!(
        out.applied_branches,
        [r("refs/heads/C").to_owned()],
        "only C is newly applied"
    );
    // C becomes the last parent of the workspace commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   ad018e0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ /  
| | * 9cbf901 (C) add c
| * | e16577c (B) add b
| |/  
* | dcc139a (A) add A2
* | a33af50 add A1
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733
├── ≡📙:A on 0dc3733 {1}
│   └── 📙:A
│       ├── ·dcc139a (🏘️)
│       └── ·a33af50 (🏘️)
├── ≡📙:B on 0dc3733 {2}
│   └── 📙:B
│       └── ·e16577c (🏘️)
└── ≡📙:C on 0dc3733 {3}
    └── 📙:C
        └── ·9cbf901 (🏘️)

"#]]
    );
    assert_worktree_files(&repo, &["a", "b", "c"], &[]);
    Ok(())
}

#[test]
fn apply_of_an_applied_branch_changes_nothing() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    let before = visualize_commit_graph_all(&repo)?;
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(r("refs/heads/A"), ws, &repo, &mut meta, &mut db)?;

    assert_eq!(
        out.status,
        OutcomeStatus::AlreadyApplied,
        "A is a parent of the workspace commit"
    );
    assert!(out.applied_branches.is_empty(), "nothing was newly applied");
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        before,
        "the repository is unchanged"
    );
    Ok(())
}

#[test]
fn apply_of_an_empty_branch_changes_nothing() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    let before = visualize_commit_graph_all(&repo)?;

    let ws = workspace(&repo, &meta, &mut db)?;
    let out =
        but_workspace::branch::apply_stack(r("refs/heads/main"), ws, &repo, &mut meta, &mut db)?;
    assert_eq!(
        out.status,
        OutcomeStatus::AlreadyApplied,
        "branch has no commits that aren't in the workspace already"
    );
    assert!(
        out.applied_branches.is_empty(),
        "branch isn't newly applied"
    );

    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        before,
        "the repository is unchanged"
    );
    Ok(())
}

#[test]
fn apply_refuses_an_overlapping_branch() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    git(&repo).args(["branch", "D", "A^"]).run();
    let before = visualize_commit_graph_all(&repo)?;
    let ws = workspace(&repo, &meta, &mut db)?;

    let err = but_workspace::branch::apply_stack(r("refs/heads/D"), ws, &repo, &mut meta, &mut db)
        .expect_err("D overlaps A");

    snapbox::assert_data_eq!(
        err.to_string(),
        snapbox::str!["Cannot apply 'D' as it overlaps with 'A'"]
    );
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        before,
        "the repository is unchanged"
    );
    Ok(())
}

#[test]
fn apply_of_a_remote_tracking_branch_applies_its_local_tracking_branch() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Options {
        unapplied_branch: true,
        ..Default::default()
    })?;
    git(&repo)
        .args(["update-ref", "refs/remotes/origin/C", "C"])
        .run();
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(
        r("refs/remotes/origin/C"),
        ws,
        &repo,
        &mut meta,
        &mut db,
    )?;

    assert_eq!(
        out.applied_branches,
        [r("refs/heads/C").to_owned()],
        "C is the local tracking branch of origin/C"
    );
    assert_eq!(
        branch_config(&repo, "C")?,
        "",
        "the existing local tracking branch is used as is"
    );
    Ok(())
}

#[test]
fn apply_of_a_remote_tracking_branch_creates_its_local_tracking_branch() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Options {
        unapplied_branch: true,
        ..Default::default()
    })?;
    git(&repo)
        .args(["update-ref", "refs/remotes/origin/remote-only", "C"])
        .run();
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(
        r("refs/remotes/origin/remote-only"),
        ws,
        &repo,
        &mut meta,
        &mut db,
    )?;

    assert_eq!(
        out.applied_branches,
        [r("refs/heads/remote-only").to_owned()],
        "the new local tracking branch is applied"
    );
    // `remote-only` is created at the commit of `origin/remote-only` and becomes the last lane.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   ad018e0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ /  
| | * 9cbf901 (origin/remote-only, remote-only, C) add c
| * | e16577c (B) add b
| |/  
* | dcc139a (A) add A2
* | a33af50 add A1
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    // It tracks `origin/remote-only`.
    snapbox::assert_data_eq!(
        branch_config(&repo, "remote-only")?,
        snapbox::str![[r#"
branch.remote-only.remote origin
branch.remote-only.merge refs/heads/remote-only

"#]]
    );
    Ok(())
}

#[test]
fn remote_tracking_uses_the_configured_local_branch() -> anyhow::Result<()> {
    for legacy_apply in [false, true] {
        let (_tmp, mut repo, mut meta, mut db) = two_stacks(Options {
            unapplied_branch: true,
            ..Default::default()
        })?;
        git(&repo)
            .args(["update-ref", "refs/remotes/origin/topic", "C"])
            .run();
        git(&repo).args(["branch", "topic", "main"]).run();
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value("branch.C.remote", "origin")?;
            config.set_raw_value("branch.C.merge", "refs/heads/topic")?;
            config.set_raw_value("branch.topic.remote", "origin")?;
            config.set_raw_value("branch.topic.merge", "refs/heads/elsewhere")?;
        }
        let before = visualize_commit_graph_all(&repo)?;
        assert_eq!(
            but_workspace::branch::local_tracking_branch(&repo, r("refs/remotes/origin/topic"))?,
            "refs/heads/C",
            "checkout selects the configured tracker, not the unrelated same-name branch"
        );
        assert_eq!(
            visualize_commit_graph_all(&repo)?,
            before,
            "looking up an existing tracker changes no refs"
        );
        let ws = workspace(&repo, &meta, &mut db)?;
        let out = if legacy_apply {
            but_workspace::branch::apply(
                r("refs/remotes/origin/topic"),
                ws,
                &repo,
                &mut meta,
                Default::default(),
            )?
        } else {
            but_workspace::branch::apply_stack(
                r("refs/remotes/origin/topic"),
                ws,
                &repo,
                &mut meta,
                &mut db,
            )?
        };
        assert_eq!(
            out.applied_branches,
            [r("refs/heads/C").to_owned()],
            "both apply paths select the configured tracker"
        );
        assert_eq!(
            repo.find_reference("refs/heads/topic")?.id(),
            repo.find_reference("refs/heads/main")?.id(),
            "the unrelated local branch is untouched"
        );
    }
    Ok(())
}

#[test]
fn remote_tracking_rejects_ambiguous_trackers_without_writing() -> anyhow::Result<()> {
    for legacy_apply in [false, true] {
        let (_tmp, mut repo, mut meta, mut db) = two_stacks(Options {
            unapplied_branch: true,
            ..Default::default()
        })?;
        git(&repo)
            .args(["update-ref", "refs/remotes/origin/topic", "C"])
            .run();
        git(&repo).args(["branch", "other", "C"]).run();
        {
            let mut config = repo.config_snapshot_mut();
            for branch in ["C", "other"] {
                config.set_raw_value(&format!("branch.{branch}.remote"), "origin")?;
                config.set_raw_value(&format!("branch.{branch}.merge"), "refs/heads/topic")?;
            }
        }
        let before = visualize_commit_graph_all(&repo)?;
        let config_before = std::fs::read(repo.common_dir().join("config"))?;
        assert!(
            but_workspace::branch::local_tracking_branch(&repo, r("refs/remotes/origin/topic"))
                .is_err(),
            "checkout must not select an arbitrary tracker"
        );
        let ws = workspace(&repo, &meta, &mut db)?;
        let result = if legacy_apply {
            but_workspace::branch::apply(
                r("refs/remotes/origin/topic"),
                ws,
                &repo,
                &mut meta,
                Default::default(),
            )
        } else {
            but_workspace::branch::apply_stack(
                r("refs/remotes/origin/topic"),
                ws,
                &repo,
                &mut meta,
                &mut db,
            )
        };
        assert!(
            result.is_err(),
            "apply must not select an arbitrary tracker"
        );
        assert_eq!(
            visualize_commit_graph_all(&repo)?,
            before,
            "ambiguity is rejected before references change"
        );
        assert_eq!(
            std::fs::read(repo.common_dir().join("config"))?,
            config_before,
            "ambiguity leaves tracking configuration unchanged"
        );
    }
    Ok(())
}

#[test]
fn remote_tracking_rejects_a_same_name_branch_tracking_elsewhere() -> anyhow::Result<()> {
    for legacy_apply in [false, true] {
        let (_tmp, mut repo, mut meta, mut db) = two_stacks(Options {
            unapplied_branch: true,
            ..Default::default()
        })?;
        git(&repo)
            .args(["update-ref", "refs/remotes/origin/topic", "C"])
            .run();
        git(&repo).args(["branch", "topic", "main"]).run();
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value("branch.topic.remote", "origin")?;
            config.set_raw_value("branch.topic.merge", "refs/heads/elsewhere")?;
        }
        let before = visualize_commit_graph_all(&repo)?;
        let expected = "Local branch refs/heads/topic tracks refs/remotes/origin/elsewhere; choose a local branch for refs/remotes/origin/topic";
        assert_eq!(
            but_workspace::branch::local_tracking_branch(&repo, r("refs/remotes/origin/topic"))
                .expect_err("an unrelated upstream prevents selecting the same-name branch")
                .to_string(),
            expected
        );
        let ws = workspace(&repo, &meta, &mut db)?;
        let result = if legacy_apply {
            but_workspace::branch::apply(
                r("refs/remotes/origin/topic"),
                ws,
                &repo,
                &mut meta,
                Default::default(),
            )
        } else {
            but_workspace::branch::apply_stack(
                r("refs/remotes/origin/topic"),
                ws,
                &repo,
                &mut meta,
                &mut db,
            )
        };
        assert_eq!(
            result
                .expect_err("apply rejects the same collision")
                .to_string(),
            expected
        );
        assert_eq!(
            visualize_commit_graph_all(&repo)?,
            before,
            "an unrelated tracking branch cannot change the workspace"
        );
        repo.config_snapshot_mut()
            .set_raw_value("branch.topic.remote", "missing-remote")?;
        assert!(
            but_workspace::branch::local_tracking_branch(&repo, r("refs/remotes/origin/topic"))
                .expect_err("a missing configured remote does not make a branch unconfigured")
                .to_string()
                .contains("cannot be resolved"),
            "complete but unresolved upstream configuration prevents fallback too"
        );
        assert_eq!(
            visualize_commit_graph_all(&repo)?,
            before,
            "unresolved upstreams leave references unchanged"
        );
    }
    Ok(())
}

#[test]
fn apply_of_a_conflicting_branch_returns_error_payload() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Options {
        unapplied_conflicting_branch: true,
        ..Default::default()
    })?;
    let before = visualize_commit_graph_all(&repo)?;
    let ws = workspace(&repo, &meta, &mut db)?;

    let err = but_workspace::branch::apply_stack(
        r("refs/heads/conflicting"),
        ws,
        &repo,
        &mut meta,
        &mut db,
    )
    .expect_err("conflicting changes the file A changes");

    let conflict = err
        .downcast_ref::<but_rebase::merge::ConflictErrorContext>()
        .expect("the error carries the conflict");
    assert_eq!(
        conflict.paths,
        ["shared"],
        "A and conflicting both change `shared`"
    );
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        before,
        "the repository is unchanged"
    );
    Ok(())
}

#[test]
fn apply_creates_a_missing_workspace_commit() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    git(&repo).args(["reset", "--hard", "main"]).run();
    // The workspace reference is checked out, but without a workspace commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* dcc139a (A) add A2
* a33af50 add A1
| * e16577c (B) add b
|/  
* 0dc3733 (HEAD -> gitbutler/workspace, origin/main, main) add M

"#]]
    );
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(r("refs/heads/A"), ws, &repo, &mut meta, &mut db)?;

    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "A wasn't applied before"
    );
    // The new workspace commit merges A.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e16577c (B) add b
| * 2aa02de (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * dcc139a (A) add A2
| * a33af50 add A1
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733
└── ≡📙:A on 0dc3733 {1}
    └── 📙:A
        ├── ·dcc139a (🏘️)
        └── ·a33af50 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn apply_switches_into_the_workspace_from_a_branch() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Options {
        unapplied_branch: true,
        ..Default::default()
    })?;
    git(&repo).args(["checkout", "C"]).run();
    let c_before = repo.rev_parse_single("C")?.detach();
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(r("refs/heads/B"), ws, &repo, &mut meta, &mut db)?;

    assert_eq!(
        out.applied_branches,
        [r("refs/heads/C").to_owned(), r("refs/heads/B").to_owned()],
        "the branch HEAD was on is applied as well"
    );
    assert_eq!(
        repo.head_name()?.expect("on a branch").as_bstr(),
        WORKSPACE_REF_NAME,
        "HEAD is on the workspace reference"
    );
    assert_eq!(
        repo.rev_parse_single("C")?.detach(),
        c_before,
        "the branch HEAD was on doesn't move"
    );
    // The new workspace commit merges only C, which HEAD was on, and B.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* dcc139a (A) add A2
* a33af50 add A1
| *   e62c042 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |/  
| | * e16577c (B) add b
| |/  
|/|   
| * 9cbf901 (C) add c
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733
├── ≡📙:C on 0dc3733 {1}
│   └── 📙:C
│       └── ·9cbf901 (🏘️)
└── ≡📙:B on 0dc3733 {2}
    └── 📙:B
        └── ·e16577c (🏘️)

"#]]
    );
    assert_worktree_files(&repo, &["b", "c"], &["a"]);
    assert_eq!(
        applied_stacks_in_metadata(&meta)?,
        ["C", "B"],
        "A, which was applied before, is left out"
    );
    Ok(())
}

#[test]
fn apply_of_the_branch_head_is_on_switches_into_a_workspace_with_just_that_branch()
-> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Options {
        unapplied_branch: true,
        ..Default::default()
    })?;
    git(&repo).args(["checkout", "C"]).run();
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(r("refs/heads/C"), ws, &repo, &mut meta, &mut db)?;

    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "C wasn't applied to the workspace before"
    );
    // The new workspace commit merges only C.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* dcc139a (A) add A2
* a33af50 add A1
| * e16577c (B) add b
|/  
| * df9c157 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 9cbf901 (C) add c
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733
└── ≡📙:C on 0dc3733 {1}
    └── 📙:C
        └── ·9cbf901 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn apply_switches_into_the_workspace_from_the_target() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Options {
        unapplied_branch: true,
        ..Default::default()
    })?;
    git(&repo).args(["checkout", "main"]).run();
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::apply_stack(r("refs/heads/C"), ws, &repo, &mut meta, &mut db)?;

    assert_eq!(
        out.applied_branches,
        [r("refs/heads/C").to_owned()],
        "the target HEAD was on does not count"
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* dcc139a (A) add A2
* a33af50 add A1
| * e16577c (B) add b
|/  
| * e8da0dc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 9cbf901 (C) add c
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733
└── ≡📙:C on 0dc3733 {1}
    └── 📙:C
        └── ·9cbf901 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn unapply_removes_a_lane() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    let ws = workspace(&repo, &meta, &mut db)?;

    let out =
        but_workspace::branch::unapply_stack(r("refs/heads/B"), &ws, &repo, &mut meta, &mut db)?;

    assert!(out.workspace_changed(), "B was applied");
    // Only A remains a parent of the workspace commit, and B keeps its commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e16577c (B) add b
| * 7aabe42 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * dcc139a (A) add A2
| * a33af50 add A1
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733
└── ≡📙:A on 0dc3733 {1}
    └── 📙:A
        ├── ·dcc139a (🏘️)
        └── ·a33af50 (🏘️)

"#]]
    );
    assert_worktree_files(&repo, &["a"], &["b"]);
    assert_eq!(
        applied_stacks_in_metadata(&meta)?,
        ["A"],
        "materializing records the remaining lanes in metadata"
    );
    Ok(())
}

#[test]
fn unapply_of_the_last_lane_puts_the_workspace_commit_on_the_target() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    for branch in ["refs/heads/A", "refs/heads/B"] {
        let ws = workspace(&repo, &meta, &mut db)?;
        but_workspace::branch::unapply_stack(r(branch), &ws, &repo, &mut meta, &mut db)?;
    }

    // The workspace reference stays checked out, with an empty workspace.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* dcc139a (A) add A2
* a33af50 add A1
| * e16577c (B) add b
|/  
| * 9fc91f8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* 0dc3733 (origin/main, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&workspace(&repo, &meta, &mut db)?).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733

"#]]
    );
    assert_worktree_files(&repo, &[], &["a", "b"]);
    Ok(())
}

#[test]
fn unapply_of_a_branch_that_is_not_a_lane_changes_nothing() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    git(&repo).args(["branch", "C", "A^"]).run();
    let before = visualize_commit_graph_all(&repo)?;
    let ws = workspace(&repo, &meta, &mut db)?;

    let out =
        but_workspace::branch::unapply_stack(r("refs/heads/C"), &ws, &repo, &mut meta, &mut db)?;
    assert!(matches!(out.workspace, Cow::Borrowed(_)), "C isn't a lane");
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        before,
        "the repository is unchanged"
    );
    Ok(())
}

#[test]
fn unapply_of_a_branch_at_the_tip_of_a_lane_removes_that_lane() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    git(&repo).args(["branch", "also-A", "A"]).run();
    let ws = workspace(&repo, &meta, &mut db)?;

    let out = but_workspace::branch::unapply_stack(
        r("refs/heads/also-A"),
        &ws,
        &repo,
        &mut meta,
        &mut db,
    )?;

    assert!(
        out.workspace_changed(),
        "lanes are matched by their tip commit, which also-A shares with A"
    );
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 0dc3733
└── ≡📙:B on 0dc3733 {1}
    └── 📙:B
        └── ·e16577c (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn unapply_keeps_conflicting_worktree_changes() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = two_stacks(Default::default())?;
    let ws = workspace(&repo, &meta, &mut db)?;

    std::fs::write(repo.workdir_path("b").expect("non-bare"), "local edit\n")?;
    let out =
        but_workspace::branch::unapply_stack(r("refs/heads/B"), &ws, &repo, &mut meta, &mut db)?;

    assert!(
        out.workspace_changed(),
        "B is unapplied regardless of the change to its file"
    );
    assert_eq!(
        std::fs::read_to_string(repo.workdir_path("b").expect("non-bare"))?,
        "local edit\n",
        "the locally modified file is kept"
    );
    // The change conflicts with B's file going away.
    snapbox::assert_data_eq!(
        but_testsupport::git(&repo)
            .args(["status", "--short"])
            .output()?
            .stdout
            .to_str_lossy()
            .into_owned(),
        snapbox::str![[r#"
UD b

"#]]
    );
    Ok(())
}
