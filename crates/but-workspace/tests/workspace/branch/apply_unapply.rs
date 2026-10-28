use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments, blank_project_meta_graph, graph,
        graph_with_cutoff_only, named_read_only_in_memory_scenario, named_writable_scenario,
        project_meta,
    },
    utils::r,
};
use bstr::ByteSlice;
use but_core::{
    RefMetadata, WORKSPACE_REF_NAME, ref_metadata,
    ref_metadata::{StackId, StackKind, WorkspaceCommitRelation::Outside},
};
use but_graph::{Graph, init::Tip, workspace::WorkspaceKind};
use but_testsupport::{
    CommandExt, InMemoryRefMetadata, git, graph_workspace, graph_workspace_determinisitcally,
    id_at, id_by_rev, sanitize_uuids_and_timestamps, visualize_commit_graph_all,
    visualize_disk_tree_with_hashes_skip_dot_git,
};
use but_workspace::branch::{apply::OutcomeStatus, unapply::WorkspaceDisposition};
use gix::refs::{Category, transaction::PreviousValue};
use snapbox::prelude::*;

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

#[test]
fn operation_denied_on_improper_workspace() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-ws-commit-one-stack-ws-advanced")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0d01196 (HEAD -> gitbutler/workspace) O1
* 4979833 GitButler Workspace Commit
* 3183e43 (main, B, A) M1

"#]]
    );
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43
└── ≡:anon: on 3183e43
    └── :anon:
        ├── ·0d01196 (🏘️)
        └── ·4979833 (🏘️)

"#]]
    );

    let branch_b = r("refs/heads/B");
    let err = but_workspace::branch::apply(branch_b, ws.clone(), &repo, &mut meta, apply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to work on workspace whose workspace commit isn't at the top",
        "cannot apply on a workspace that isn't proper"
    );

    let err =
        but_workspace::branch::apply(r("HEAD"), ws.clone(), &repo, &mut meta, apply_options())
            .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply symbolic ref 'HEAD' due to potential ambiguity"
    );

    let err = but_workspace::branch::unapply(branch_b, &ws, &repo, &mut meta, unapply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to work on workspace whose workspace commit isn't at the top",
        "cannot unapply on a workspace that isn't proper"
    );

    let err = but_workspace::branch::unapply(r("HEAD"), &ws, &repo, &mut meta, unapply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to unapply symbolic ref 'HEAD' due to potential ambiguity"
    );

    Ok(())
}

#[test]
fn unapply_ad_hoc_tip_checks_out_target_only_when_disposition_allows_it() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
    git(&repo).args(["checkout", "A"]).run();
    let branch_before = id_at(&repo, "A");
    let workspace_before = id_at(&repo, "gitbutler/workspace");
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;

    let err = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect_err("keeping the workspace must not permit switching away from an ad-hoc branch");
    assert_eq!(
        err.to_string(),
        "Cannot unapply branch 'A' from an ad-hoc workspace because the workspace cannot be empty",
        "callers that disable single-branch mode retain the existing rejection"
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
        ),
    )?;
    assert_eq!(
        out.checked_out.as_ref().map(|name| name.as_ref()),
        Some(r("refs/heads/main")),
        "report the checkout destination"
    );
    assert_eq!(
        repo.head()?.referent_name(),
        Some(r("refs/heads/main")),
        "HEAD must be attached to the local target"
    );
    // Only HEAD changes: neither the unapplied branch nor an existing workspace ref is deleted.
    assert_eq!(
        id_at(&repo, "A"),
        branch_before,
        "the unapplied branch is preserved"
    );
    assert_eq!(
        id_at(&repo, "gitbutler/workspace"),
        workspace_before,
        "an existing workspace ref is preserved"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
    );
    Ok(())
}

#[test]
fn unapply_ad_hoc_tip_checks_out_local_target_without_upstream() -> anyhow::Result<()> {
    let (_tmp, mut repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
    git(&repo).args(["checkout", "A"]).run();
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));
    assert!(
        project_meta(&repo)?.remote_target_ref(&repo)?.is_none(),
        "local target has no configured upstream"
    );
    let local_tip = id_by_rev(&repo, "main").detach();
    let branch_before = id_at(&repo, "A");
    let workspace_before = id_at(&repo, "gitbutler/workspace");
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
        ),
    )?;
    assert_eq!(
        out.checked_out.as_ref().map(|name| name.as_ref()),
        Some(r("refs/heads/main")),
        "unapply does not require a target upstream"
    );
    assert_eq!(
        repo.head()?.referent_name(),
        Some(r("refs/heads/main")),
        "HEAD is attached to local main"
    );
    assert_eq!(
        repo.head_id()?.detach(),
        local_tip,
        "checkout uses the local target tip"
    );
    assert_eq!(
        id_at(&repo, "A"),
        branch_before,
        "the unapplied branch stays intact"
    );
    assert_eq!(
        id_at(&repo, "gitbutler/workspace"),
        workspace_before,
        "ad-hoc unapply preserves the existing workspace ref"
    );
    Ok(())
}

#[test]
fn unapply_workspace_checks_out_diverged_local_target_not_upstream_or_cutoff() -> anyhow::Result<()>
{
    let (_tmp, mut repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
    let mut project_meta = project_meta(&repo)?;
    let cutoff = id_by_rev(&repo, "main").detach();
    let local_tip = repo
        .commit(
            "refs/heads/main",
            "local target divergence",
            repo.find_commit(id_by_rev(&repo, "B"))?.tree_id()?,
            [cutoff],
        )?
        .detach();
    let upstream_tip = repo
        .commit(
            "refs/remotes/origin/trunk",
            "upstream divergence",
            repo.find_commit(id_by_rev(&repo, "A"))?.tree_id()?,
            [cutoff],
        )?
        .detach();
    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("remote.origin.url", "./remote.git")?;
        config.set_raw_value("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*")?;
        config.set_raw_value("branch.main.remote", "origin")?;
        config.set_raw_value("branch.main.merge", "refs/heads/trunk")?;
    }
    project_meta.target_commit_id = Some(cutoff);
    assert_eq!(
        project_meta.remote_target_ref(&repo)?,
        Some("refs/remotes/origin/trunk".try_into()?),
        "tracking names need not match the local target"
    );
    assert_ne!(local_tip, upstream_tip, "the tips differ");
    assert_eq!(
        repo.merge_base(local_tip, upstream_tip)?
            .map(gix::Id::detach),
        Some(cutoff),
        "local and upstream tips diverge above the stored cutoff"
    );
    let ws = Graph::from_head(
        &repo,
        &meta,
        project_meta,
        &mut db,
        but_graph::init::Options::limited(),
    )?
    .into_workspace()?;
    let out = but_workspace::branch::unapply(
        r(WORKSPACE_REF_NAME),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
        ),
    )?;
    assert_eq!(
        out.checked_out.as_ref().map(|name| name.as_ref()),
        Some(r("refs/heads/main")),
        "checkout selects the configured local target"
    );
    assert_eq!(
        repo.head()?.referent_name(),
        Some(r("refs/heads/main")),
        "HEAD is attached to local main"
    );
    assert_eq!(
        repo.head_id()?.detach(),
        local_tip,
        "checkout must not substitute upstream or the stored cutoff"
    );
    assert_eq!(
        id_by_rev(&repo, "main").detach(),
        local_tip,
        "local divergence is preserved"
    );
    assert_eq!(
        id_by_rev(&repo, "origin/trunk").detach(),
        upstream_tip,
        "upstream is untouched"
    );
    assert_eq!(
        out.workspace.graph.project_meta.target_commit_id,
        Some(cutoff),
        "checkout does not move the recorded boundary"
    );
    assert!(
        repo.try_find_reference(WORKSPACE_REF_NAME)?.is_none(),
        "the unapplied workspace ref is removed"
    );
    Ok(())
}

#[test]
fn unapply_ad_hoc_target_itself_is_rejected() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
    git(&repo).args(["checkout", "main"]).run();
    let before = visualize_commit_graph_all(&repo)?;
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;

    let err = but_workspace::branch::unapply(
        r("refs/heads/main"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
        ),
    )
    .expect_err("checking out the branch being unapplied would not unapply it");
    assert_eq!(
        err.to_string(),
        "Cannot unapply the local target branch",
        "reject a checkout to the same branch"
    );
    snapbox::assert_data_eq!(visualize_commit_graph_all(&repo)?, before.into_data().raw());
    Ok(())
}

#[test]
fn unapply_tip_of_ad_hoc_branch_is_an_error() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-with-3-commits")?;
    // fixture starts with a single local main branch
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 281da94 (HEAD -> main) 3
* 12995d7 2
* 3d57fc1 1

"#]]
    );

    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    // a normal checked-out branch is still an ad-hoc workspace without workspace metadata
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
    );

    let err = but_workspace::branch::unapply(
        r("refs/heads/main"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )
    .expect_err("unapplied workspace should be empty, but this one can't be");
    assert_eq!(
        err.to_string(),
        "Cannot unapply branch 'main' from an ad-hoc workspace because the workspace cannot be empty"
    );
    Ok(())
}

#[test]
fn unapply_branch_from_named_ad_hoc_workspace_affects_metadata() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-stack-two-segments")?;
    // fixture starts with a single local main branch
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 53ad0c2 (unrelated) add U1
|/  
* 3183e43 (HEAD -> main, origin/main) M1

"#]]
    );

    let a2_ref = r("refs/heads/A2");
    let a2_tip = repo.rev_parse_single("A2")?;
    let ws = but_graph::Graph::from_commit_traversal(
        a2_tip,
        a2_ref.to_owned(),
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        but_graph::init::Options::default(),
    )?
    .into_workspace()?;
    // a normal checked-out branch is still an ad-hoc workspace without workspace metadata
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A2 <> ✓!
└── ≡:A2 {1}
    ├── :A2
    │   └── ·f1889e7
    ├── :A1
    │   └── ·7de99e1
    └── :main[🌳] <> origin/main
        └── ❄️3183e43

"#]]
    );

    let branch = Category::LocalBranch.to_full_name("on-A1")?;
    let first_id = id_by_rev(&repo, "A1");
    repo.reference(
        branch.as_ref(),
        first_id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "on-A1 with branch metadata",
    )?;
    let mut branch_md = meta.branch(branch.as_ref())?;
    branch_md.update_times(true);
    meta.set_branch(&branch_md)?;
    let ws = but_graph::Graph::from_commit_traversal(
        a2_tip,
        a2_ref.to_owned(),
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        but_graph::init::Options::default(),
    )?
    .into_workspace()?;
    // a branch with its own ref-metadata gives unapply a visible segment to reject
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A2 <> ✓!
└── ≡:A2 {1}
    ├── :A2
    │   └── ·f1889e7
    ├── 📙:on-A1
    │   └── ·7de99e1 ►A1
    └── :main[🌳] <> origin/main
        └── ❄️3183e43

"#]]
    );

    let out = but_workspace::branch::unapply(
        branch.as_ref(),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )
    .expect("this works because `on-A1` is only present thanks to ref-metadata which we removed");

    // on-A1 no longer has a metadata-disambiguated segment
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
⌂:A2 <> ✓!
└── ≡:A2 {1}
    ├── :A2
    │   ├── ·f1889e7
    │   └── ·7de99e1 ►A1, ►on-A1
    └── :main[🌳] <> origin/main
        └── ❄️3183e43

"#]]
    );

    let err = but_workspace::branch::unapply(
        r("refs/heads/main"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )
    .expect_err("without project target metadata in the graph, main is a visible non-tip segment");
    assert_eq!(
        err.to_string(),
        "Cannot unapply branch 'main' from an ad-hoc workspace because non-tip branches can only disappear if their now removed metadata disambiguated them"
    );
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_two_virtual_stacks_on_same_commit_apply_dependent_first()
-> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-no-ws-commit-one-stack-one-branch")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::Inactive, &["B"]);
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    // We know a stack, but nothing is actually in the workspace.
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Put "B" into the workspace, even though it's the dependent branch of A.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:B on e5d0542 {1}
    └── 📙:B

"#]]
    );

    // Applying A is always a new stack then.
    but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    // the workspace ref still points to the base e5d0542
    snapbox::assert_data_eq!(
        graph_workspace(&graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?)
            .to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:B on e5d0542 {1}
│   └── 📙:B
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
    );

    // It's all virtual.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 5ecf7a4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* e5d0542 (main, B, A) A

"#]]
    );

    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // A was removed with metadata only
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:B on e5d0542 {1}
    └── 📙:B

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // virtual stacks don't cause checkouts.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    // no stacks are left
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );
    // no workspace commit is present
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    Ok(())
}

mod workspace_disposition {
    use super::*;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_testsupport::gix_testtools::tempfile::TempDir;

    #[test]
    fn keep_workspace_commit() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = workspace_with_virtual_base()?;
        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::KeepWorkspaceCommit),
        )?;
        assert!(
            out.workspace.kind.has_managed_commit(),
            "KeepWorkspaceCommit must preserve the checked-out managed workspace commit"
        );
        // A was removed
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:virtual-base on 85efbe4 {1}
│   └── 📙:virtual-base
└── ≡📙:B on 85efbe4 {3}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
        );
        // the workspace commit is still there with two parents (even though there is no need)
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 09d8e52 (A) A
| * 3b77768 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/| 
| * c813d8d (B) B
|/  
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
        );

        Ok(())
    }

    #[test]
    fn keep_workspace_reference() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = workspace_with_virtual_base()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::KeepWorkspaceReference),
        )?;
        assert!(
            out.workspace.kind.has_managed_commit(),
            "KeepWorkspaceReference should keep a workspace commit when a virtual stack remains next to a real stack"
        );
        // There still is a workspace merge commit, we have two stacks, virtual or not
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:virtual-base on 85efbe4 {1}
│   └── 📙:virtual-base
└── ≡📙:B on 85efbe4 {3}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
        );
        // the virtual stack counts as real stack and it merges the base to account for it
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 09d8e52 (A) A
| * 3b77768 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/| 
| * c813d8d (B) B
|/  
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
        );

        Ok(())
    }

    #[test]
    fn prevent_unnecessary_workspace_reference_checks_out_last_real_stack() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
        add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
        )?;
        // with one real stack left, the workspace ref is deleted and the remaining stack is checked out
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/B",
    ),
}

"#]]
        );
        // the workspace ref is deleted and HEAD points to the last real stack
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 09d8e52 (A) A
| * c813d8d (HEAD -> B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        );
        // the projection is the checked-out remaining stack
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:B[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B[🌳] on 85efbe4 {1}
    └── 📙:B[🌳]
        └── ·c813d8d

"#]]
        );

        Ok(())
    }

    #[test]
    fn prevent_unnecessary_workspace_references_keep_workspace_commit_keeps_merge_when_ref_remains()
    -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = workspace_with_virtual_base()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/virtual-base"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(
                WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
            ),
        )?;
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
        );
        assert!(
            out.workspace.kind.has_managed_commit(),
            "compatibility disposition should preserve the managed workspace commit when the workspace ref remains"
        );
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:A on 85efbe4 {2}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {3}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   483c8bd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * c813d8d (B) B
* | 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
            .raw()
        );

        Ok(())
    }

    #[test]
    fn allow_workspace_reference_deletion() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("no-ws-ref-no-ws-commit-two-branches")?;

        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on e5d0542
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
        );

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
        )?;
        // deleting the workspace reference should switch to the local target branch
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
        );

        // everything is dissolved, there is no gitbutler/workspace ref either
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on e5d0542

"#]]
        );

        Ok(())
    }

    #[test]
    fn compatibility_mode_deletes_workspace_reference_when_possible() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("no-ws-ref-no-ws-commit-two-branches")?;

        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(
                WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
            ),
        )?;
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
        );

        // the compatibility mode still removes the whole workspace when it can
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on e5d0542

"#]]
        );

        Ok(())
    }

    #[test]
    fn unapply_workspace_ref_requires_disposition_that_allows_switching() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("no-ws-ref-no-ws-commit-two-branches")?;

        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
        // the workspace ref is checked out
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on e5d0542
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
        );

        let refs_before = visualize_commit_graph_all(&repo)?;
        let err = but_workspace::branch::unapply(
            r("refs/heads/gitbutler/workspace"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )
        .expect_err("unapplying the workspace ref requires checking out another ref");
        assert_eq!(
            err.to_string(),
            "Cannot unapply workspace reference 'gitbutler/workspace' without switching away from it"
        );

        // the workspace is unchanged
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on e5d0542
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
        );
        assert_eq!(
            visualize_commit_graph_all(&repo)?,
            refs_before,
            "failing before checkout must leave refs unchanged"
        );

        Ok(())
    }

    #[test]
    fn keep_workspace_commit_with_last_stack_removed() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
        add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* d69fe94 (B) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
        );

        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    │   └── ·d69fe94 (🏘️)
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
        );

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::KeepWorkspaceCommit),
        )?;
        // the workspace is empty
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
        );
        // the workspace commit is kept as no-op, it's legacy behaviour that will be removed
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* d69fe94 (B) B
* 09d8e52 (A) A
| * bde8ed6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* 85efbe4 (origin/main, main) M

"#]]
        );

        Ok(())
    }

    fn workspace_with_virtual_base() -> anyhow::Result<(
        TempDir,
        gix::Repository,
        VirtualBranchesTomlMetadata,
        but_graph::Workspace,
    )> {
        let (tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
        let base_id = repo
            .find_reference("refs/heads/main")?
            .peel_to_id()?
            .detach();
        repo.reference(
            r("refs/heads/virtual-base"),
            base_id,
            PreviousValue::Any,
            "create a metadata-only virtual stack on the workspace base",
        )?;
        add_stack_with_segments(&mut meta, 1, "virtual-base", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 2, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 3, "B", StackState::InWorkspace, &[]);
        let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
        assert!(
            ws.kind.has_managed_commit(),
            "fixture starts with a managed workspace commit"
        );

        let repo_log = visualize_commit_graph_all(&repo)?;

        snapbox::assert_data_eq!(
            repo_log,
            snapbox::str![[r#"
*   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
            .raw()
        );
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:virtual-base on 85efbe4 {1}
│   └── 📙:virtual-base
├── ≡📙:A on 85efbe4 {2}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {3}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
        );

        Ok((tmp, repo, meta, ws))
    }
}

#[test]
fn main_with_advanced_remote_tracking_branch() -> anyhow::Result<()> {
    let (_tmp, mut repo, _meta, mut db) = named_writable_scenario("main-with-advanced-remote")?;
    // Because VirtualBranchesTomlMetadata does not support remotes, use InMemoryRefMetadata for
    // this test instead.
    let mut meta = InMemoryRefMetadata::default();

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6b40b15 (origin/feature) without-local-tracking
| * 552e7dc (origin/main) only-on-remote
|/  
* 3183e43 (HEAD -> main) M1

"#]]
    );

    // Applying origin/feature must use the fetch refspec to create review/feature and set its
    // upstream merge ref to refs/heads/review/feature, instead of guessing feature from the name.
    // Replace the wildcard mapping so it cannot mask this renamed mapping, while keeping main
    // mapped for the already-tracked branch checked below.
    but_core::git_config::edit_repo_config(&repo, gix::config::Source::Local, |config| {
        but_core::git_config::set_config_value(
            config,
            "remote.origin.fetch",
            "+refs/heads/main:refs/remotes/origin/main",
        )?;
        but_core::git_config::ensure_config_value(
            config,
            "remote.origin.fetch",
            "+refs/heads/review/feature:refs/remotes/origin/feature",
        )?;
        Ok(())
    })?;
    repo.reload()?;

    meta.workspaces.push((
        "refs/heads/gitbutler/workspace".try_into()?,
        ref_metadata::Workspace::default(),
    ));
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    // Without a project target, the ad-hoc view uses main's configured upstream.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡:main[🌳] <> origin/main⇣1 on 3183e43 {1}
    └── :main[🌳] <> origin/main⇣1
        └── 🟣552e7dc

"#]]
    );

    // We cannot apply remote tracking branches directly, but it resolves automatically to local tracking branches.
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/main"),
        ws.clone(),
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // nothing was actually applied as the `main` branch is already in the workspace
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    applied_branches: "[]",
}

"#]]
    );

    // Set up an automatic tracking of origin/feature, as remote tracking branches can't be in the workspace.
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/feature"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/main, refs/heads/review/feature]",
}

"#]]
    );
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    // both branches, main and feature, are available in the newly created workspace ref.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:review/feature {5ad}
    ├── 📙:review/feature
    │   └── ·6b40b15 (🏘️)
    └── 📙:main <> origin/main⇣1
        ├── 🟣552e7dc
        └── ❄️3183e43 (🏘️)

"#]]
    );

    // the new local tracking ref actually exists, and is in the right spot.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 95baf27 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 6b40b15 (origin/feature, review/feature) without-local-tracking
| * 552e7dc (origin/main) only-on-remote
|/  
* 3183e43 (main) M1

"#]]
    );

    repo.reload()?;
    let config = repo.config_snapshot();
    let section = config.section("branch", Some("review/feature".into()))?;
    snapbox::assert_data_eq!(
        section.to_bstring().to_string(),
        snapbox::str![[r#"
[branch "review/feature"]
	remote = origin
	merge = refs/heads/review/feature

"#]]
    );

    Ok(())
}

#[test]
fn unapply_remotely_tracked_tip_of_multi_segment_stack_can_delete_workspace_ref()
-> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("no-ws-ref-stack-and-dependent-branch")?;
    for branch in ["A", "B", "C"] {
        let local_name = Category::LocalBranch.to_full_name(branch)?;
        let remote_name: gix::refs::FullName = format!("refs/remotes/origin/{branch}")
            .as_str()
            .try_into()?;
        let commit_id = repo
            .find_reference(local_name.as_ref())?
            .peel_to_id()?
            .detach();
        repo.reference(
            remote_name,
            commit_id,
            PreviousValue::Any,
            "create remote-tracking ref for stack branch",
        )?;
    }

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B <> origin/B on 85efbe4 {42}
    └── 📙:B <> origin/B
        ├── ❄️f084d61 (🏘️) ►A, ►C
        └── ❄️7076dee (🏘️) ►D, ►E

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
        ),
    )?;
    // deleting the workspace ref uses the target fallback instead of the unapplied stack
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
    );
    // the managed workspace ref is deleted and the local target branch is checked out
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (origin/C, origin/B, origin/A, C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    // an ad-hoc workspace remains on the local target branch
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
    );
    Ok(())
}

#[test]
fn workspace_with_out_of_ws_ref_and_anon_stack() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("advanced-stack-and-unnamed-stack-in-workspace")?;
    add_stack_with_segments(&mut meta, 1, "outside", StackState::InWorkspace, &[]);
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d03b217 (feature) F1
| *   dd3b979 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |\  
| * | d6bdeab missing-name
|/ /  
| | * 5121eb9 (outside) advanced-outside
| |/  
| * 67c6397 advanced-inside
|/  
* 3183e43 (origin/main, main) M1

"#]]
        .raw()
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡:anon: on 3183e43
│   └── :anon:
│       └── ·d6bdeab (🏘️)
└── ≡:anon: on 3183e43
    └── :anon:
        └── ·67c6397 (🏘️)

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/feature"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/feature]",
}

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph(&repo, &meta, &mut db)?.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡:anon: on 3183e43
│   └── :anon:
│       └── ·d6bdeab (🏘️)
├── ≡:anon: on 3183e43
│   └── :anon:
│       └── ·67c6397 (🏘️)
└── ≡📙:feature on 3183e43 {2ec}
    └── 📙:feature
        └── ·d03b217 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_two_stacks_on_same_commit() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-no-ws-commit-one-stack-one-branch")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Put "A" into the workspace, yielding a single branch.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
    );
    // If the user is checked out on gitbutler/workspace but is missing a workspace commit, we
    // always create one.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6277161 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* e5d0542 (main, B, A) A

"#]]
    );

    let branch_b = r("refs/heads/B");
    let out = but_workspace::branch::apply(branch_b, ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    // Note how it will create a new stack (to keep it simple),
    // in theory we could also add B as dependent branch.
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:A on e5d0542 {41}
│   └── 📙:A
└── ≡📙:B on e5d0542 {42}
    └── 📙:B

"#]]
    );

    // If the user is checked out on gitbutler/workspace but is missing a workspace commit, we
    // always create one.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 452772e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* e5d0542 (main, B, A) A

"#]]
    );

    let out = but_workspace::branch::unapply(branch_b, &ws, &repo, &mut meta, unapply_options())?;
    let ws = out.workspace.into_owned();
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    Ok(())
}

#[test]
fn unapply_natural_stack_with_partial_workspace_metadata() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
    add_stack_with_segments(
        &mut meta,
        1,
        "B",
        StackState::InWorkspace,
        &["not-in-graph"],
    );
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    // A is naturally visible, and B has metadata that matches it with an extra segment in metadata that's not there
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:B on 85efbe4 {1}
│   └── 📙:B
│       └── ·c813d8d (🏘️)
└── ≡:A on 85efbe4
    └── :A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;

    // A is removed and B is left
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let ws_md = out.workspace.metadata.as_ref().expect("present");
    // the extra segment is still present in metadata to help with stack identities
    snapbox::assert_data_eq!(
        ws_md.to_debug(),
        snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
    stacks: [
        WorkspaceStack {
            id: 00000000-0000-0000-0000-000000000001,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/B",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/not-in-graph",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ],
}

"#]]
    );
    Ok(())
}

#[test]
fn unapply_natural_stack_branch_without_workspace_metadata() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-ws-commit-single-stack-double-stack-files")?;
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    // a single and multi-branch stack are visible without any workspace metadata
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 893d602
├── ≡:C on 893d602
│   ├── :C
│   │   └── ·356de85 (🏘️)
│   └── :B
│       └── ·f25f65c (🏘️)
└── ≡:A on 893d602
    └── :A
        └── ·26e45af (🏘️)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;

    // C was unapplied, and the workspace commit removed
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 893d602
└── ≡📙:A on 893d602 {1}
    └── 📙:A
        └── ·26e45af (🏘️)

"#]]
    );
    // the workspace commit isn't retained due to the workspace disposition
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 26e45af (HEAD -> gitbutler/workspace, A) A
| * 356de85 (C) C
| * f25f65c (B) B
|/  
* 893d602 (origin/main, main) M

"#]]
    );

    let ws_md = out.workspace.metadata.as_ref().expect("present");
    // the metadata matches the real workspace now
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{ws_md:#?}")),
        snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
    stacks: [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/C",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/B",
                    archived: false,
                },
            ],
            workspacecommit_relation: Outside,
        },
        WorkspaceStack {
            id: 2,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/A",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ],
}
"#]]
    );
    Ok(())
}

#[test]
fn no_ws_ref_no_ws_commit_two_stacks_on_same_commit_ad_hoc_workspace_with_target()
-> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("no-ws-ref-no-ws-commit-two-branches")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> main, origin/main, B, A) A

"#]]
    );
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on e5d0542

"#]]
    );

    // Put "A" into the workspace, creating the workspace ref, but never put a branch related to the target in as well,
    // which is currently checked out with `main`.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on e5d0542
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
    );

    // If the user is checked out on gitbutler/workspace but is missing a workspace commit, we
    // always create one.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6277161 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* e5d0542 (origin/main, main, B, A) A

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on e5d0542
├── ≡📙:A on e5d0542 {41}
│   └── 📙:A
└── ≡📙:B on e5d0542 {42}
    └── 📙:B

"#]]
    );

    // If the user is checked out on gitbutler/workspace but is missing a workspace commit, we
    // always create one.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 452772e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* e5d0542 (origin/main, main, B, A) A

"#]]
    );

    // Cannot put the configured local target branch into its workspace.
    for branch in ["refs/heads/main", "refs/remotes/origin/main"] {
        let err =
            but_workspace::branch::apply(r(branch), ws.clone(), &repo, &mut meta, apply_options())
                .unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("Cannot add the target '{branch}' branch to its own workspace")
        );

        let out =
            but_workspace::branch::unapply(r(branch), &ws, &repo, &mut meta, unapply_options())?;
        assert!(
            !out.workspace_changed(),
            "target refs are never applied, so unapplying them is always fulfilled after the call (i.e. they aren't applied)"
        );
    }

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    let ws = out.workspace.into_owned();
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on e5d0542
└── ≡📙:A on e5d0542 {41}
    └── 📙:A

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )?;
    // the local target branch is checked out
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on e5d0542

"#]]
    );
    // no workspace ref is present anymore
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> main, origin/main, B, A) A

"#]]
    );

    Ok(())
}

#[test]
fn apply_after_switching_out_of_workspace_drops_stale_stacks() -> anyhow::Result<()> {
    // A managed workspace exists with `outside` marked in-workspace. The user then `git switch`es
    // onto `feature`, a branch outside the workspace, leaving the metadata stale.
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("advanced-stack-and-unnamed-stack-in-workspace")?;
    add_stack_with_segments(&mut meta, 1, "outside", StackState::InWorkspace, &[]);
    git(&repo).args(["branch", "applied", "feature"]).run();
    git(&repo).args(["checkout", "feature"]).run();

    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;

    // Apply a different branch from the adhoc `feature` checkout. The new workspace should hold the
    // current branch plus the applied one, while the stale `outside` stack is demoted.
    let out = but_workspace::branch::apply(
        r("refs/heads/applied"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    assert_eq!(out.status, OutcomeStatus::Applied);

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(
        in_workspace("outside"),
        Some(false),
        "the stale stack from the abandoned workspace is demoted to outside"
    );
    assert_eq!(
        in_workspace("feature"),
        Some(true),
        "the foreign branch we switched to joins the new workspace"
    );
    assert_eq!(
        in_workspace("applied"),
        Some(true),
        "the newly applied branch is in the new workspace"
    );
    Ok(())
}

#[test]
fn apply_in_managed_workspace_drops_stack_whose_ref_disappeared() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("managed-workspace-with-missing-applied-branch")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 863775d (C) add C
| *   12c0f48 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |/  
| | * 239dcaf add A
| |/  
|/|   
| * 208b2ad (B) add B
|/  
* 893d602 (origin/main, main) M

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    assert!(matches!(ws.kind, WorkspaceKind::Managed { .. }));
    assert!(!ws.refname_is_segment(r("refs/heads/A")));
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 893d602
├── ≡📙:B on 893d602 {2}
│   └── 📙:B
│       └── ·208b2ad (🏘️)
└── ≡:anon: on 893d602
    └── :anon:
        └── ·239dcaf (🏘️)

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(out.status, OutcomeStatus::Applied);

    snapbox::assert_data_eq!(
        graph_workspace(&graph(&repo, &meta, &mut db)?.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 893d602
├── ≡:anon: on 893d602
│   └── :anon:
│       └── ·239dcaf (🏘️)
├── ≡📙:B on 893d602 {2}
│   └── 📙:B
│       └── ·208b2ad (🏘️)
└── ≡📙:C on 893d602 {43}
    └── 📙:C
        └── ·863775d (🏘️)

"#]]
    );

    let ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        ws_md
            .find_branch(r("refs/heads/A"), StackKind::AppliedAndUnapplied)
            .is_some(),
        "the disappeared stack configuration is retained"
    );
    assert!(
        ws_md
            .find_branch(r("refs/heads/A"), StackKind::Applied)
            .is_none(),
        "the stack whose branch disappeared is no longer applied"
    );
    assert!(
        ws_md
            .find_branch(r("refs/heads/B"), StackKind::Applied)
            .is_some()
    );
    assert!(
        ws_md
            .find_branch(r("refs/heads/C"), StackKind::Applied)
            .is_some()
    );
    Ok(())
}

#[test]
fn apply_from_enclosed_adhoc_workspace_rebuilds_around_current_and_applied() -> anyhow::Result<()> {
    // A managed workspace has two live stacks, then HEAD is moved to one of its branches. Applying
    // a third branch from that enclosed AdHoc checkout rebuilds the workspace around the checked-out
    // branch and the newly applied branch.
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-ws-commit-two-file-stacks")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
    // initial workspace has A and B applied
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   4fce3a1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * ccf539c (A) A
* | 53c254d (B) B
|/  
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["A", "B"], &["C"]);
    // workspace checkout contains A and B files
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── A:100644:f70f10e4db19068f79bc43844b49f3eece45c4e8
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    git(&repo).args(["checkout", "-b", "C", "main"]).run();
    std::fs::write(repo.workdir_path("C").expect("non-bare"), "C\n")?;
    git(&repo).args(["add", "C"]).run();
    git(&repo).args(["commit", "-m", "add C"]).run();
    git(&repo).args(["checkout", "B"]).run();
    // direct checkout of B leaves C outside the workspace
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 863775d (C) add C
| *   4fce3a1 (gitbutler/workspace) GitButler Workspace Commit
| |\  
| | * ccf539c (A) A
| |/  
|/|   
| * 53c254d (HEAD -> B) B
|/  
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["B"], &["A", "C"]);
    // direct checkout of B contains only B's file
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    assert!(
        matches!(ws.kind, WorkspaceKind::AdHoc),
        "direct checkout of B projects as a plain branch"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:B[🌳] <> ✓refs/heads/main on 893d602
└── ≡📙:B[🌳] on 893d602 {1}
    └── 📙:B[🌳]
        └── ·53c254d (🏘️)

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(out.status, OutcomeStatus::Applied);
    // applying C rebuilds the workspace around B and C
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* ccf539c (A) A
| *   7ed2c6c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |\  
| | * 863775d (C) add C
| |/  
|/|   
| * 53c254d (B) B
|/  
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["B", "C"], &["A"]);
    // workspace checkout contains B and C files after apply
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
├── C:100644:3cc58df83752123644fef39faab2393af643b1d2
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );
    assert_eq!(
        repo.head_name()?.as_ref().map(|rn| rn.as_bstr()),
        Some(r("refs/heads/gitbutler/workspace").as_bstr()),
        "applying from an ad-hoc checkout should switch back to the managed workspace"
    );

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(
        in_workspace("A"),
        Some(false),
        "previously visible stack A should be left behind"
    );
    assert_eq!(
        in_workspace("B"),
        Some(true),
        "checked-out enclosed stack B should stay in the workspace"
    );
    assert_eq!(
        in_workspace("C"),
        Some(true),
        "the newly applied branch joins the workspace"
    );
    Ok(())
}

#[test]
fn apply_from_adhoc_checkout_rebuilds_around_current_and_applied() -> anyhow::Result<()> {
    // A managed workspace has two live stacks, then HEAD is moved to a third branch. Applying one
    // of the previously applied branches rebuilds the workspace around the checked-out branch and
    // the branch being applied.
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-ws-commit-two-file-stacks")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
    // initial workspace has A and B applied
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   4fce3a1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * ccf539c (A) A
* | 53c254d (B) B
|/  
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["A", "B"], &["C"]);
    // workspace checkout contains A and B files
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── A:100644:f70f10e4db19068f79bc43844b49f3eece45c4e8
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    git(&repo).args(["checkout", "-b", "C", "main"]).run();
    std::fs::write(repo.workdir_path("C").expect("non-bare"), "C\n")?;
    git(&repo).args(["add", "C"]).run();
    git(&repo).args(["commit", "-m", "add C"]).run();
    // direct checkout of C leaves A and B in the old workspace
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 863775d (HEAD -> C) add C
| *   4fce3a1 (gitbutler/workspace) GitButler Workspace Commit
| |\  
| | * ccf539c (A) A
| |/  
|/|   
| * 53c254d (B) B
|/  
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["C"], &["A", "B"]);
    // direct checkout of C contains only C's file
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── C:100644:3cc58df83752123644fef39faab2393af643b1d2
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    // direct checkout of C is an ad-hoc workspace next to the existing managed workspace
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓refs/heads/main on 893d602
└── ≡:C[🌳] on 893d602 {1}
    └── :C[🌳]
        └── ·863775d

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(out.status, OutcomeStatus::Applied);
    // applying A rebuilds the workspace around C and A
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 53c254d (B) B
| *   dbb9910 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |\  
| | * 863775d (C) add C
| |/  
|/|   
| * ccf539c (A) A
|/  
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["A", "C"], &["B"]);
    // workspace checkout contains C and A files after apply
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── A:100644:f70f10e4db19068f79bc43844b49f3eece45c4e8
├── C:100644:3cc58df83752123644fef39faab2393af643b1d2
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );
    assert_eq!(
        repo.head_name()?.as_ref().map(|rn| rn.as_bstr()),
        Some(r("refs/heads/gitbutler/workspace").as_bstr()),
        "applying from an ad-hoc checkout should switch back to the managed workspace"
    );

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(
        in_workspace("A"),
        Some(true),
        "the newly applied branch joins the workspace"
    );
    assert_eq!(
        in_workspace("B"),
        Some(false),
        "previously visible stack B should be left behind"
    );
    assert_eq!(
        in_workspace("C"),
        Some(true),
        "the checked-out branch joins the workspace"
    );
    Ok(())
}

#[test]
fn apply_already_applied_branch_from_adhoc_checkout_excludes_other_applied_stacks()
-> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-ws-commit-three-file-stacks")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "C", StackState::InWorkspace, &[]);
    assert_worktree_files(&repo, &["A", "B", "C"], &[]);

    let a_tip_before_apply = id_by_rev(&repo, "A");
    git(&repo).args(["checkout", "A"]).run();
    assert_worktree_files(&repo, &["A"], &["B", "C"]);

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;

    assert_eq!(out.status, OutcomeStatus::Applied);
    assert_eq!(
        id_by_rev(&repo, "A"),
        a_tip_before_apply,
        "applying from A must not move A to the workspace commit"
    );
    assert_eq!(
        repo.head_name()?.as_ref().map(|rn| rn.as_bstr()),
        Some(r("refs/heads/gitbutler/workspace").as_bstr()),
        "applying from an ad-hoc checkout should switch back to the managed workspace"
    );
    assert_worktree_files(&repo, &["A", "C"], &["B"]);

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(in_workspace("A"), Some(true));
    assert_eq!(in_workspace("B"), Some(false));
    assert_eq!(in_workspace("C"), Some(true));
    Ok(())
}

#[test]
fn new_workspace_exists_elsewhere_and_to_be_applied_branch_exists_there() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-no-ws-commit-one-stack-one-branch")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    // The workspace is empty at the stored cutoff, without a configured target branch.
    snapbox::assert_data_eq!(
        graph_workspace(&graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?)
            .to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Pretend "B" is checked out (it's at the right state independently of that)
    let (b_id, b_ref) = id_at(&repo, "B");
    let ws = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        but_graph::init::Options::default(),
    )?
    .into_workspace()?;
    // The existing workspace isn't projected without its ProjectMeta, but its ref stays visible.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:B <> ✓!
└── ≡:B {1}
    └── :B
        └── ·e5d0542 ►A, ►gitbutler/workspace[🌳], ►main

"#]]
    );

    // Put "A" into the workspace, hence we want "A" and "B" in it.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B, refs/heads/A]",
}

"#]]
    );

    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    // This apply brings both branches into the existing workspace, and it's where HEAD points to.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
├── ≡📙:B {42}
│   └── 📙:B
│       └── ·e5d0542 (🏘️) ►main
└── ≡📙:A {41}
    └── 📙:A
        └── ·e5d0542 (🏘️) ►main

"#]]
    );

    // HEAD must now point to the workspace (that already existed)
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 5ecf7a4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* e5d0542 (main, B, A) A

"#]]
    );

    Ok(())
}

mod unapply_checked_out {
    use but_testsupport::gix_testtools::tempfile::TempDir;

    use super::*;

    type Scenario = (
        TempDir,
        gix::Repository,
        but_meta::VirtualBranchesTomlMetadata,
        but_graph::Workspace,
    );

    fn virtual_stack_tip_checked_out() -> anyhow::Result<Scenario> {
        let (tmp, repo, mut meta, mut db) =
            named_writable_scenario("ws-ref-no-ws-commit-one-stack-one-branch")?;
        add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
        let initial_graph = visualize_commit_graph_all(&repo)?;

        snapbox::assert_data_eq!(
            initial_graph,
            snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
        );

        git(&repo).args(["checkout", "B"]).run();
        let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;

        // B is checked out directly, and its stack is virtual
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:B[🌳] <> ✓!
└── ≡:B[🌳] {1}
    └── :B[🌳]
        └── ·e5d0542 (🏘️) ►A, ►main

"#]]
        );

        Ok((tmp, repo, meta, ws))
    }

    fn real_stack_tip_checked_out() -> anyhow::Result<Scenario> {
        let (tmp, repo, mut meta, mut db) =
            named_writable_scenario("detached-with-multiple-branches")?;
        let initial_graph = visualize_commit_graph_all(&repo)?;

        // the initial checkout is detached, so the workspace has no target ref to fall back to
        snapbox::assert_data_eq!(
            initial_graph,
            snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/  
| * aaa195b (HEAD, C) C1
|/  
* 3183e43 (main) M1

"#]]
        );

        let mut ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
        for branch_to_apply in ["C", "B"] {
            but_workspace::branch::apply(
                Category::LocalBranch
                    .to_full_name(branch_to_apply)?
                    .as_ref(),
                ws,
                &repo,
                &mut meta,
                apply_options(),
            )?;
            ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
        }

        // C and B are real stacks in the managed workspace
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:C on 3183e43 {43}
│   └── 📙:C
│       └── ·aaa195b (🏘️)
└── ≡📙:B on 3183e43 {42}
    └── 📙:B
        └── ·f57c528 (🏘️)

"#]]
        );

        git(&repo).args(["checkout", "B"]).run();
        let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;

        // B is checked out directly, and its stack has a real commit
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:B[🌳] <> ✓!
└── ≡📙:B[🌳] {1}
    ├── 📙:B[🌳]
    │   └── ·f57c528 (🏘️)
    └── :main
        └── ·3183e43 (🏘️)

"#]]
        );

        Ok((tmp, repo, meta, ws))
    }

    #[test]
    fn checked_out_stack_tip_cannot_be_unapplied() -> anyhow::Result<()> {
        for scenario in [virtual_stack_tip_checked_out, real_stack_tip_checked_out] {
            let (_tmp, repo, mut meta, ws) = scenario()?;
            let err = but_workspace::branch::unapply(
                r("refs/heads/B"),
                &ws,
                &repo,
                &mut meta,
                unapply_options(),
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "Cannot unapply branch 'B' from an ad-hoc workspace because the workspace cannot be empty",
                "a checked out branch projects on its own, so there is no workspace to unapply it from"
            );
        }
        Ok(())
    }

    #[test]
    fn virtual_stack_tip_with_indirect_entrypoint() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("ws-ref-no-ws-commit-one-stack-one-branch")?;
        add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &["A"]);

        git(&repo).args(["checkout", "A"]).run();
        let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
        // A is checked out as a lower segment of the B stack
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:A[🌳] <> ✓! on e5d0542
└── ≡:A[🌳] on e5d0542 {1}
    └── :A[🌳]

"#]]
        );

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // the workspace is checked out as the current branch's stack was unapplied
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
        );
        // the returned workspace is projected from the managed workspace ref
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:A[🌳] <> ✓! on e5d0542
└── ≡:A[🌳] on e5d0542 {1}
    └── :A[🌳]

"#]]
        );
        // HEAD switches back to the managed workspace when the checked-out stack is unapplied
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* e5d0542 (HEAD -> A, main, gitbutler/workspace, B) A

"#]]
        );
        Ok(())
    }

    #[test]
    fn virtual_unrelated_stack_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = virtual_stack_tip_checked_out()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // no change in what's checked out
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
        );

        // the returned workspace preserves the unrelated checked-out entrypoint
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:B[🌳] <> ✓!
└── ≡:B[🌳] {1}
    └── :B[🌳]
        └── ·e5d0542 (🏘️) ►A, ►main

"#]]
        );
        // HEAD remains on B while the managed workspace also contains B
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* e5d0542 (HEAD -> B, main, gitbutler/workspace, A) A

"#]]
        );
        Ok(())
    }

    #[test]
    fn real_unrelated_stack_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = real_stack_tip_checked_out()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/C"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // the checked out branch isn't changed
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
        );

        // the returned workspace preserves the unrelated checked-out entrypoint
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:B[🌳] <> ✓!
└── ≡📙:B[🌳] {1}
    ├── 📙:B[🌳]
    │   └── ·f57c528 (🏘️)
    └── :main
        └── ·3183e43 (🏘️)

"#]]
        );
        // HEAD remains on B while the managed workspace also contains B
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 49d4b34 (A) A1
| *   fdec130 (gitbutler/workspace) GitButler Workspace Commit
| |/  
| | * f57c528 (HEAD -> B) B1
| |/  
|/|   
| * aaa195b (C) C1
|/  
* 3183e43 (main) M1

"#]]
        );
        Ok(())
    }
}

#[test]
fn apply_multiple_without_target_or_metadata_or_base() -> anyhow::Result<()> {
    let (_tmp, mut repo, mut meta, mut db) = named_writable_scenario("one-fork")?;
    // No project target and no branch upstream should supply a base.
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bf53300 (A) add A
| * b1540e5 (HEAD -> main) M
|/  
| * 0e391b2 (origin/B) add B
|/  
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );

    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
        ├── ·b1540e5
        └── ·e31e6ca

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/main, refs/heads/A]",
}

"#]]
    );

    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
├── ≡📙:main {1a5}
│   └── 📙:main
│       ├── ·b1540e5 (🏘️)
│       └── ·e31e6ca (🏘️)
└── ≡📙:A {41}
    └── 📙:A
        ├── ·bf53300 (🏘️)
        └── ·e31e6ca (🏘️)

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   d87b903 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * bf53300 (A) add A
* | b1540e5 (main) M
|/  
| * 0e391b2 (origin/B) add B
|/  
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
        .raw()
    );

    let branch_b_rt = r("refs/remotes/origin/B");
    let out = but_workspace::branch::apply(branch_b_rt, ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );

    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
├── ≡📙:main {1a5}
│   └── 📙:main
│       ├── ·b1540e5 (🏘️)
│       └── ·e31e6ca (🏘️)
├── ≡📙:A {41}
│   └── 📙:A
│       ├── ·bf53300 (🏘️)
│       └── ·e31e6ca (🏘️)
└── ≡📙:B {42}
    └── 📙:B
        ├── ·0e391b2 (🏘️)
        └── ·e31e6ca (🏘️)

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   7bcf528 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 0e391b2 (origin/B, B) add B
| * | bf53300 (A) add A
| |/  
* / b1540e5 (main) M
|/  
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
        .raw()
    );

    let out =
        but_workspace::branch::unapply(branch_b_rt, &ws, &repo, &mut meta, unapply_options())?;
    // remote tracking branches can't be unapplied, as they aren't applied in the first place
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // the local tracking branch, howeer, will be unapplied
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
├── ≡📙:main {1a5}
│   └── 📙:main
│       ├── ·b1540e5 (🏘️)
│       └── ·e31e6ca (🏘️)
└── ≡📙:A {41}
    └── 📙:A
        ├── ·bf53300 (🏘️)
        └── ·e31e6ca (🏘️)

"#]]
    );
    assert!(
        !repo.workdir_path("B").expect("non-bare").exists(),
        "unapplying B updates the checked-out workspace tree"
    );
    assert_eq!(
        std::fs::read_to_string(repo.workdir_path("A").expect("non-bare"))?,
        "A\n"
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // A is removed and main is checked out
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    // By default, we keep the workspace, and `main` is in there as a target is missing
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:main {1a5}
    └── 📙:main
        ├── ·b1540e5 (🏘️)
        └── ·e31e6ca (🏘️)

"#]]
    );
    assert!(
        !repo.workdir_path("A").expect("non-bare").exists(),
        "unapplying A updates the checked-out workspace tree"
    );
    assert_eq!(
        std::fs::read_to_string(repo.workdir_path("init").expect("non-bare"))?,
        "init\n"
    );
    Ok(())
}

#[test]
fn unapply_dirty_worktree_abort_keeps_refs_and_metadata() -> anyhow::Result<()> {
    let (_tmp, mut repo, mut meta, mut db) = named_writable_scenario("one-fork")?;
    // Keep this ad-hoc workspace unbounded by a target or branch upstream.
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bf53300 (A) add A
| * b1540e5 (HEAD -> main) M
|/  
| * 0e391b2 (origin/B) add B
|/  
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
        ├── ·b1540e5
        └── ·e31e6ca

"#]]
    );
    but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    but_workspace::branch::apply(
        r("refs/remotes/origin/B"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
├── ≡📙:main {1a5}
│   └── 📙:main
│       ├── ·b1540e5 (🏘️)
│       └── ·e31e6ca (🏘️)
├── ≡📙:A {41}
│   └── 📙:A
│       ├── ·bf53300 (🏘️)
│       └── ·e31e6ca (🏘️)
└── ≡📙:B {42}
    └── 📙:B
        ├── ·0e391b2 (🏘️)
        └── ·e31e6ca (🏘️)

"#]]
    );

    let ws_before = graph_workspace(&ws).to_string();
    let refs_before = visualize_commit_graph_all(&repo)?;
    let metadata_before = sanitize_uuids_and_timestamps(format!(
        "{:#?}",
        ws.metadata
            .as_ref()
            .expect("managed workspace has metadata")
    ));

    std::fs::write(repo.workdir_path("B").expect("non-bare"), "local edit\n")?;
    let worktree_before =
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string();
    let err =
        but_workspace::branch::unapply(r("refs/heads/B"), &ws, &repo, &mut meta, unapply_options())
            .unwrap_err();
    snapbox::assert_data_eq!(
        err.to_debug(),
        snapbox::str![[r#"
Context {
    code: PreconditionFailed,
    message: Some(
        "Uncommitted files would be overwritten by checkout: \"B\"",
    ),
}

"#]]
        .raw()
    );

    assert_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        worktree_before,
        "the locally modified file wasn't changed"
    );
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        refs_before,
        "refs must not move when dirty worktree checkout aborts"
    );
    let ws_after = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    assert_eq!(graph_workspace(&ws_after).to_string(), ws_before);
    assert_eq!(
        sanitize_uuids_and_timestamps(format!(
            "{:#?}",
            ws_after
                .metadata
                .as_ref()
                .expect("managed workspace has metadata")
        )),
        metadata_before
    );
    Ok(())
}

#[test]
fn apply_repairs_stale_outside_metadata_for_reachable_branch() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    assert!(
        ws.refname_is_segment(r("refs/heads/B")),
        "fixture must start with B visible in the cached workspace graph"
    );

    let mut ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    for stack in &mut ws_md.stacks {
        stack.workspacecommit_relation = Outside;
    }
    meta.set_workspace(&ws_md)?;

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "reachable-but-unapplied metadata must be repaired instead of reported as a no-op"
    );
    assert_eq!(out.applied_branches, [r("refs/heads/B").to_owned()]);

    let ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        ws_md
            .find_branch(r("refs/heads/B"), StackKind::Applied)
            .is_some(),
        "apply should put the reachable branch back into the applied metadata set"
    );

    Ok(())
}

#[test]
fn apply_multiple_segments_of_stack_in_order_merge_if_needed() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-stack-two-segments")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 53ad0c2 (unrelated) add U1
|/  
* 3183e43 (HEAD -> main, origin/main) M1

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on 3183e43

"#]]
    );

    // Add another stack to be sure we correctly handle the removal of existing stacks later (i.e. don't get the index wrong)
    let out = but_workspace::branch::apply(
        r("refs/heads/unrelated"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/unrelated]",
}

"#]]
    );
    // TODO: should this not avoid creating a workspace commit? Yes.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 6848743 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 53ad0c2 (unrelated) add U1
|/  
* 3183e43 (origin/main, main) M1

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;

    let out =
        but_workspace::branch::apply(r("refs/heads/A1"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/A1]",
}

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:unrelated on 3183e43 {3c4}
│   └── 📙:unrelated
│       └── ·53ad0c2 (🏘️)
└── ≡📙:A1 on 3183e43 {72}
    └── 📙:A1
        └── ·7de99e1 (🏘️)

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/A2"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/A2]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:unrelated on 3183e43 {3c4}
│   └── 📙:unrelated
│       └── ·53ad0c2 (🏘️)
└── ≡📙:A2 on 3183e43 {73}
    ├── 📙:A2
    │   └── ·f1889e7 (🏘️)
    └── 📙:A1
        └── ·7de99e1 (🏘️)

"#]]
    );

    // The metadata is in sync, and A1 is outside the workspace.
    snapbox::assert_data_eq!(
        ws.metadata.to_debug(),
        snapbox::str![[r#"
Some(
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 00000000-0000-0000-0000-0000000003c4,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/unrelated",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000072,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/A1",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Outside,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000073,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/A2",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
        ],
    },
)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A2"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // A2 is removed, along with A1
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:unrelated on 3183e43 {3c4}
    └── 📙:unrelated
        └── ·53ad0c2 (🏘️)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A1"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // this is a no-op, A1 was removed with A2 prior
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // nothing changed, this was a no-op
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:unrelated on 3183e43 {3c4}
    └── 📙:unrelated
        └── ·53ad0c2 (🏘️)

"#]]
    );

    let opts = unapply_options();
    assert!(
        matches!(
            opts.workspace_disposition,
            WorkspaceDisposition::KeepWorkspaceReference
        ),
        "this should make the workspace commit disappear, but keep the workspace reference"
    );
    let out =
        but_workspace::branch::unapply(r("refs/heads/unrelated"), &ws, &repo, &mut meta, opts)?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43

"#]]
    );

    //
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 53ad0c2 (unrelated) add U1
|/  
* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main) M1

"#]]
    );
    Ok(())
}

#[test]
fn unapply_existing_branch_outside_detached_ad_hoc_workspace_is_noop() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("detached-with-multiple-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/  
| * aaa195b (HEAD, C) C1
|/  
* 3183e43 (main) M1

"#]]
    );
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:DETACHED <> ✓! on 3183e43
└── ≡:anon: on 3183e43 {1}
    └── :anon:
        └── ·aaa195b ►C

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // it's a noop, the workspace didn't change, the branch was never applied
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );
    Ok(())
}

#[test]
fn unapply_branch_from_detached_ad_hoc_workspace_is_an_error() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-stack-two-segments")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 53ad0c2 (unrelated) add U1
|/  
* 3183e43 (HEAD -> main, origin/main) M1

"#]]
    );

    let a2_id = repo.rev_parse_single("A2")?.detach();
    let ws = Graph::from_commit_traversal_tips(
        &repo,
        [Tip::detached_entrypoint(a2_id)],
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_traversal_options(),
    )?
    .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:DETACHED <> ✓!
└── ≡:anon: {1}
    ├── :anon:
    │   └── ·f1889e7 ►A2
    ├── :A1
    │   └── ·7de99e1
    └── :main[🌳] <> origin/main
        └── ❄️3183e43

"#]]
    );

    let unapply_this = r("refs/heads/A1");
    let err =
        but_workspace::branch::unapply(unapply_this, &ws, &repo, &mut meta, unapply_options())
            .expect_err("detached ad-hoc workspaces cannot unapply their contained branch");
    assert_eq!(
        err.to_string(),
        "Cannot unapply a branch from an ad-hoc detached workspace"
    );
    Ok(())
}

#[test]
fn unapply_workspace_ref_refuses_conflicted_named_stack_checkout() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("with-conflict")?;
    // the fixture starts on a conflicted main commit
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 8450331 (HEAD -> main, tag: conflicted) GitButler WIP Commit
* a047f81 (tag: normal) init

"#]]
    );

    git(&repo)
        .args(["branch", "tip-conflicted", "conflicted"])
        .run();
    git(&repo).args(["reset", "--hard", "normal"]).run();

    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    but_workspace::branch::apply(
        r("refs/heads/tip-conflicted"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    let ws = blank_project_meta_graph(&repo, &meta, &mut db)?.into_workspace()?;
    // the target-less workspace can contain a conflicted named stack
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:tip-conflicted {595}
    ├── 📙:tip-conflicted
    │   └── ·8450331 (🏘️)
    └── 📙:main
        └── ·a047f81 (🏘️)

"#]]
    );

    let err = but_workspace::branch::unapply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect_err("workspace ref unapply must not check out conflicted stack tips");
    assert!(
        but_workspace::branch::dissolve_unnecessary_workspace(
            &ws,
            &repo,
            &meta,
            Default::default()
        )?
        .is_none(),
        "a workspace whose only stack has a conflicted tip stays, as that tip can't be checked out"
    );
    assert_eq!(
        err.to_string(),
        "Cannot unapply workspace reference by checking out conflicted commit at 'tip-conflicted'"
    );
    // the failed unapply leaves HEAD and the workspace ref unchanged
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 8a0bbba (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 8450331 (tag: conflicted, tip-conflicted) GitButler WIP Commit
* a047f81 (tag: normal, main) init

"#]]
    );
    Ok(())
}

#[test]
fn apply_two_ambiguous_stacks_with_target_with_dependent_branch() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("no-ws-ref-stack-and-dependent-branch")?;
    add_stack_with_segments(&mut meta, 1, "C", StackState::Inactive, &["E"]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::Inactive, &["D"]);
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
    );

    // Apply the dependent branch, to bring in only the dependent branch
    let out =
        but_workspace::branch::apply(r("refs/heads/E"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/E]",
}

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:E on 85efbe4 {1}
    └── 📙:E
        └── ·7076dee (🏘️) ►D

"#]]
    );

    // Apply the former tip of the stack, to create a new stack. Note how it won't double-list the
    // other stack.
    but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:E on 85efbe4 {1}
│   └── 📙:E
│       └── ·7076dee (🏘️) ►D
└── ≡📙:C on 85efbe4 {43}
    ├── 📙:C
    │   └── ·f084d61 (🏘️) ►A, ►B
    └── 📙:E
        └── ·7076dee (🏘️) ►D

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   78f3659 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f084d61 (C, B, A) A2
|/  
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    // Adding `B` as tip of an unapplied stack brings in the whole stack.
    // BUT: Currently it overrides the previous stack C, which points to the same commit, and avoids any merge!
    // Accepting this behaviour for now as it's quite rare to have such ambiguity, even though I'd love if one day
    // for this to just work as people might intuitively want, even if that means the same commit is used multiple times.
    but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:E on 85efbe4 {1}
│   └── 📙:E
│       └── ·7076dee (🏘️) ►D
└── ≡📙:B on 85efbe4 {2}
    ├── 📙:B
    │   └── ·f084d61 (🏘️) ►A, ►C
    └── 📙:E
        └── ·7076dee (🏘️) ►D

"#]]
    );

    // Applying C again… works, but it's creating a dependent stack.
    // This is what happens because we notice that C can't be applied as independent stack due to the graph algorithm,
    // and then it tries it a dependent stack, which should always work.
    but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options()).unwrap();
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:E on 85efbe4 {1}
│   └── 📙:E
│       └── ·7076dee (🏘️) ►D
└── ≡📙:C on 85efbe4 {2}
    ├── 📙:C
    ├── 📙:B
    │   └── ·f084d61 (🏘️) ►A
    └── 📙:E
        └── ·7076dee (🏘️) ►D

"#]]
    );

    Ok(())
}

#[test]
fn apply_two_ambiguous_stacks_with_target() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("no-ws-ref-stack-and-dependent-branch")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
    );

    // Apply `A` first.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:A on 85efbe4 {41}
    └── 📙:A
        ├── ·f084d61 (🏘️) ►B, ►C
        └── ·7076dee (🏘️) ►D, ►E

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 773e030 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    // Apply `B` - the only sane way is to make it its own stack, but allow it to diverge.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {41}
    ├── 📙:B
    └── 📙:A
        ├── ·f084d61 (🏘️) ►C
        └── ·7076dee (🏘️) ►D, ►E

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    // What follows is a bit wonky, but for now is here to document what happens in a complex scenario.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    // applying C succeeds and updates the workspace metadata
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/C]",
}

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    // C is recorded as another segment at the same tip in the B/A stack
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {41}
    ├── 📙:B
    ├── 📙:C
    └── 📙:A
        ├── ·f084d61 (🏘️)
        └── ·7076dee (🏘️) ►D, ►E

"#]]
    );
    // adding C only changes workspace metadata, not Git refs or objects
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/D"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    // applying D succeeds and updates the workspace metadata
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/D]",
}

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    // D becomes a lower segment in the same projected stack, leaving E as the remaining alternate ref at that commit
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {41}
    ├── 📙:B
    ├── 📙:C
    ├── 📙:A
    │   └── ·f084d61 (🏘️)
    └── 📙:D
        └── ·7076dee (🏘️) ►E

"#]]
    );
    // adding D also remains metadata-only even though the stack presentation changes
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/E"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    // applying E forces the lower same-commit branch pair into its own stack
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/E]",
}

"#]]
    );

    let ws = out.workspace;
    // applying all ambiguous dependent branches ends with B/C/A and E/D split into two stacks
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:B on 85efbe4 {41}
│   ├── 📙:B
│   ├── 📙:C
│   ├── 📙:A
│   │   └── ·f084d61 (🏘️)
│   └── 📙:D
│       └── ·7076dee (🏘️)
└── ≡📙:E on 85efbe4 {44}
    ├── 📙:E
    └── 📙:D
        └── ·7076dee (🏘️)

"#]]
    );
    // future improvement: it should most definitely not create a merge if both tips have shared in-workspace commits in their ancestry
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   2c125a1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | f084d61 (C, B, A) A2
|/  
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/E"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // unapplying E removes the E/D stack and keeps the B/C/A stack applied
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    assert!(
        out.workspace.kind.has_managed_commit(),
        "legacy mode should rebuild the workspace commit after removing E/D"
    );

    let ws = out.workspace.into_owned();
    // after unapplying E, legacy mode keeps a workspace commit
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {41}
    ├── 📙:B
    ├── 📙:C
    ├── 📙:A
    │   └── ·f084d61 (🏘️)
    └── 📙:D
        └── ·7076dee (🏘️)

"#]]
    );
    // after unapplying E, the workspace commit contains only the remaining B/C/A stack
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/D"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // unapplying D after E is already gone is a no-op
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // the B/C/A stack stays applied after the D no-op
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {41}
    ├── 📙:B
    ├── 📙:C
    ├── 📙:A
    │   └── ·f084d61 (🏘️)
    └── :E
        └── ·7076dee (🏘️)

"#]]
    );
    // the Git graph is unchanged after the D no-op
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // unapplying C removes that middle metadata segment from B/C/A
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // after unapplying C, B/A/E remains applied with D only as an ambiguous ref on E's tip
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {41}
    ├── 📙:B
    ├── 📙:A
    │   └── ·f084d61 (🏘️)
    └── 📙:E
        └── ·7076dee (🏘️) ►D

"#]]
    );
    // the Git graph is unchanged after removing C from metadata
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // unapplying B removes the remaining B/A applied stack
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // after unapplying B, no applied stacks remain
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
    );
    // after unapplying B, there are no mergeable stacks left so legacy mode keeps an empty workspace commit
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
| * bde8ed6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // unapplying A after B removed the remaining stack is a no-op
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // the workspace stays empty after the A no-op
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
    );
    // the Git graph is unchanged after the A no-op
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
| * bde8ed6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect("workspace ref can be unapplied by checking out a named target");
    // unapplying the workspace ref switches to the target's local branch when the disposition allows deleting the workspace ref
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // the projection is the checked-out target branch
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/heads/main on 85efbe4

"#]]
    );
    // the managed workspace ref is deleted and HEAD points to main
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    Ok(())
}

#[test]
fn apply_with_conflicts_shows_exact_conflict_info() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("various-heads-for-multi-line-merge-conflict")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d3cce74 (clean-A) add A
| * 115e41b (clean-B) add B
|/  
| * 34c4591 (clean-C) add C
|/  
| * bf09eae (conflict-F1) add F1
|/  
| * f2ce66d (conflict-F2) add F2
|/  
| * 4bbb93c (HEAD -> conflict-hero) add conflicting-F2
| * 98519e9 add conflicting-F1
|/  
* 85efbe4 (main, gitbutler/workspace) M

"#]]
    );

    git(&repo).args(["checkout", "main"]).run();
    git(&repo)
        .args(["branch", "-d", "gitbutler/workspace"])
        .run();
    // The fixture helper created `graph` while `HEAD` still pointed to `conflict-hero`.
    // Replaying that graph would correctly keep using `conflict-hero` as the traversal
    // entrypoint, even though the test just checked out `main`. Build the graph from
    // the current repository state so the workspace under test starts at `main`.
    let mut ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;

    for branch_to_apply in [
        "clean-A",
        "conflict-F1",
        "clean-B",
        "conflict-F2",
        "clean-C",
    ] {
        but_workspace::branch::apply(
            Category::LocalBranch
                .to_full_name(branch_to_apply)?
                .as_ref(),
            ws,
            &repo,
            &mut meta,
            apply_options(),
        )
        .unwrap_or_else(|err| panic!("{branch_to_apply}: {err}"));
        ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    }

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on 85efbe4
├── ≡📙:main on 85efbe4 {1a5}
│   └── 📙:main
├── ≡📙:clean-A on 85efbe4 {271}
│   └── 📙:clean-A
│       └── ·d3cce74 (🏘️)
├── ≡📙:conflict-F1 on 85efbe4 {3f6}
│   └── 📙:conflict-F1
│       └── ·bf09eae (🏘️)
├── ≡📙:clean-B on 85efbe4 {272}
│   └── 📙:clean-B
│       └── ·115e41b (🏘️)
├── ≡📙:conflict-F2 on 85efbe4 {3f7}
│   └── 📙:conflict-F2
│       └── ·f2ce66d (🏘️)
└── ≡📙:clean-C on 85efbe4 {273}
    └── 📙:clean-C
        └── ·34c4591 (🏘️)

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4bbb93c (conflict-hero) add conflicting-F2
* 98519e9 add conflicting-F1
| *-----.   e13e11a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/|\ \ \ \  
| | | | | * 34c4591 (clean-C) add C
| |_|_|_|/  
|/| | | |   
| | | | * f2ce66d (conflict-F2) add F2
| |_|_|/  
|/| | |   
| | | * 115e41b (clean-B) add B
| |_|/  
|/| |   
| | * bf09eae (conflict-F1) add F1
| |/  
|/|   
| * d3cce74 (clean-A) add A
|/  
* 85efbe4 (main) M

"#]]
        .raw()
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/conflict-hero"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{out:#?}")),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[]",
    conflicting_stacks: [
        ConflictingStack {
            id: 1,
            ref_name: "refs/heads/conflict-F1",
        },
        ConflictingStack {
            id: 2,
            ref_name: "refs/heads/conflict-F2",
        },
    ],
}
"#]]
    );
    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    // By default, it fails and just reports the conflicting stacks, so it's the same as it was before.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on 85efbe4
├── ≡📙:main on 85efbe4 {1a5}
│   └── 📙:main
├── ≡📙:clean-A on 85efbe4 {271}
│   └── 📙:clean-A
│       └── ·d3cce74 (🏘️)
├── ≡📙:conflict-F1 on 85efbe4 {3f6}
│   └── 📙:conflict-F1
│       └── ·bf09eae (🏘️)
├── ≡📙:clean-B on 85efbe4 {272}
│   └── 📙:clean-B
│       └── ·115e41b (🏘️)
├── ≡📙:conflict-F2 on 85efbe4 {3f7}
│   └── 📙:conflict-F2
│       └── ·f2ce66d (🏘️)
└── ≡📙:clean-C on 85efbe4 {273}
    └── 📙:clean-C
        └── ·34c4591 (🏘️)

"#]]
    );
    let conflicting_stacks = out
        .conflicting_stacks
        .iter()
        .map(|stack| stack.ref_name.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        conflicting_stacks,
        ["refs/heads/conflict-F1", "refs/heads/conflict-F2"]
    );

    Ok(())
}

#[test]
fn conflicting_apply_reports_no_applied_branches_and_names_conflicting_stacks() -> anyhow::Result<()>
{
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("one-fork-with-conflicting-sibling")?;
    let graph_snapshot = visualize_commit_graph_all(&repo)?.replace("|/  ", "|/");
    snapbox::assert_data_eq!(
        graph_snapshot,
        snapbox::str![[r#"
* bf53300 (A) add A
| * 543911c (add-A-too) add a different A
| * b1540e5 (HEAD -> main) M
|/
| * 0e391b2 (origin/B) add B
|/
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    // A is applied before trying the conflicting sibling branch
    // The header describes local main; incoming history still comes from its upstream.
    assert_eq!(
        ws.graph
            .project_meta
            .remote_target_ref(&repo)?
            .map(|name| name.to_string()),
        Some("refs/remotes/origin/main".to_owned()),
        "the fixture must configure the target upstream"
    );
    assert_eq!(
        ws.incoming_target_commit_ids(&repo)?,
        Vec::<gix::ObjectId>::new(),
        "incoming upstream commits must remain visible independently of the local target count"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main⇣1 on e31e6ca
└── ≡📙:A on e31e6ca {41}
    └── 📙:A
        └── ·bf53300 (🏘️)

"#]]
    );
    let refs_before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &refs_before,
        snapbox::str![[r#"
* 543911c (add-A-too) add a different A
* b1540e5 (main) M
| * 9f5b797 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * bf53300 (A) add A
|/  
| * 0e391b2 (origin/B) add B
|/  
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/add-A-too"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    assert_eq!(
        out.status,
        OutcomeStatus::ConflictAborted,
        "conflicted apply should be classified as an aborted apply"
    );
    // The workspace is changed, notably, as it always passes the most recent projection.
    // a conflict-aborted apply must not report branches as applied
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{out:#?}")),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[]",
    conflicting_stacks: [
        ConflictingStack {
            id: 1,
            ref_name: "refs/heads/A",
        },
    ],
}
"#]]
    );
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        refs_before,
        "an aborting conflict must leave refs unchanged"
    );

    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_flat() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("ws-ref-no-ws-commit-one-stack-one-branch")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let ws = graph_with_cutoff_only(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:A on e5d0542 {1}
│   └── 📙:A
└── ≡📙:B on e5d0542 {2}
    └── 📙:B

"#]]
    );

    // Apply the workspace ref itself, it's a no-op
    let out = but_workspace::branch::apply(
        r("refs/heads/gitbutler/workspace"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // nothing actually changed, so nothing is mentioned
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    applied_branches: "[]",
}

"#]]
    );

    let (b_id, b_ref) = id_at(&repo, "B");
    let ws = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_traversal_options(),
    )?
    .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:B <> ✓!
└── ≡:B {1}
    └── :B
        └── ·e5d0542 (🏘️) ►A, ►main

"#]]
    );
    // Already applied (the HEAD points to it).
    let out = but_workspace::branch::apply(
        b_ref.as_ref(),
        ws.clone(),
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // no-ops aren't listing the already applied branches
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    applied_branches: "[]",
}

"#]]
    );

    // Simulate stale metadata: the cached graph still contains A, but metadata says it is outside.
    let mut ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    let (stack_idx, _) = ws_md
        .find_owner_indexes_by_name(r("refs/heads/A"), StackKind::AppliedAndUnapplied)
        .expect("A is in metadata");
    ws_md.stacks[stack_idx].workspacecommit_relation = Outside;
    meta.set_workspace(&ws_md)?;

    // To apply A, we checkout the surrounding workspace and repair the stale metadata.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "enclosed branches should report a successful apply"
    );
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B, refs/heads/A]",
}

"#]]
    );
    let ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        ws_md
            .find_branch(r("refs/heads/A"), StackKind::Applied)
            .is_some(),
        "apply should repair stale enclosing-branch metadata"
    );

    // Now the workspace ref itself is checked out.
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
├── ≡📙:A {1}
│   └── 📙:A
│       └── ·e5d0542 (🏘️) ►main
└── ≡📙:B {2}
    └── 📙:B
        └── ·e5d0542 (🏘️) ►main

"#]]
    );
    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_with_commits() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:A on 85efbe4 {1}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {2}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    // Apply the workspace ref itself, it's a no-op
    let ws_ref = r("refs/heads/gitbutler/workspace");
    let out = but_workspace::branch::apply(ws_ref, ws, &repo, &mut meta, apply_options())?;
    // the workspace ref itself counts as no-op as well
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    applied_branches: "[]",
}

"#]]
    );

    let (b_id, b_ref) = id_at(&repo, "B");
    let ws = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        project_meta(&repo)?,
        &mut db,
        but_graph::init::Options::default(),
    )?
    .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:B <> ✓refs/heads/main on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    // Already applied (the HEAD points to it, it literally IS the workspace).
    let out = but_workspace::branch::apply(
        b_ref.as_ref(),
        ws.clone(),
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // already applied branches are a no-op, even when a stack segment is checked out
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    applied_branches: "[]",
}

"#]]
    );

    let err = but_workspace::branch::apply(ws_ref, ws.clone(), &repo, &mut meta, apply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply a reference that already is a workspace: 'gitbutler/workspace'",
        "it's never good to merge one managed workspace into another, and we just disallow it.\
         Note that we could also check it out."
    );

    // To apply, we just checkout the surrounding workspace.
    let b_tip_before_apply = id_by_rev(&repo, "B");
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "enclosed branches with commits should report a successful apply"
    );
    assert_eq!(
        id_by_rev(&repo, "B"),
        b_tip_before_apply,
        "applying an enclosed branch must not move the previously checked-out branch"
    );
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    applied_branches: "[refs/heads/B, refs/heads/A]",
}

"#]]
    );

    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
├── ≡📙:A on 85efbe4 {1}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {2}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn apply_nonexisting_branch_failure() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) =
        named_read_only_in_memory_scenario("ws-ref-no-ws-commit-one-stack-one-branch", "")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let graph = graph_with_cutoff_only(&repo, &meta, &mut db)?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    let err = but_workspace::branch::apply(
        r("refs/heads/does-not-exist"),
        ws,
        &repo,
        &mut *meta,
        apply_options(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot apply non-existing branch 'does-not-exist'"
    );

    // Nothing should be changed
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    Ok(())
}

#[test]
fn unapply_nonexisting_branch() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) =
        named_read_only_in_memory_scenario("ws-ref-no-ws-commit-one-stack-one-branch", "")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let graph = graph_with_cutoff_only(&repo, &meta, &mut db)?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    let err = but_workspace::branch::unapply(
        r("refs/heads/does-not-exist"),
        &ws,
        &repo,
        &mut *meta,
        unapply_options(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot unapply non-existing branch 'does-not-exist'"
    );

    // Nothing should be changed
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    Ok(())
}

#[test]
fn unborn_apply_needs_base() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) =
        named_read_only_in_memory_scenario("unborn-empty-detached-remote", "unborn")?;
    // Depending on the Git version it produces`* 3183e43 (orphan/main, orphan/HEAD) M1` on CI,
    // so a comment is used as reference.
    // snapbox::assert_data_eq!(visualize_commit_graph_all(&repo)?, snapbox::str!["* 3183e43 (orphan/main) M1"]);

    let graph = blank_project_meta_graph(&repo, &*meta, &mut db)?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]

"#]]
    );

    // Idempotency in ad-hoc workspace
    let out = but_workspace::branch::apply(
        r("refs/heads/main"),
        ws.clone(),
        &repo,
        &mut *meta,
        apply_options(),
    )?;
    // the HEAD is already at 'main', so nothing changes
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    applied_branches: "[]",
}

"#]]
    );

    // Cannot apply branch without a base,
    // but since remote is transformed into a local tracking branch, it's a noop.
    let out = but_workspace::branch::apply(
        r("refs/remotes/orphan/main"),
        ws,
        &repo,
        &mut *meta,
        apply_options(),
    )?;
    // this won't happen (often) in the real world, but it's a no-op
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    applied_branches: "[]",
}

"#]]
    );

    let ws = blank_project_meta_graph(&repo, &*meta, &mut db)?.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]

"#]]
    );

    Ok(())
}

#[test]
fn apply_from_single_branch_mode_does_not_report_already_applied_head_branch() -> anyhow::Result<()>
{
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
    let ws = graph(&repo, &meta, &mut db)?.into_workspace()?;

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )?;
    // HEAD is on B directly, while metadata still records B as applied.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 09d8e52 (A) A
| * c813d8d (HEAD -> B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        out.workspace.into_owned(),
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // B was applied before, so only A is reported.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    Ok(())
}

fn apply_options() -> but_workspace::branch::apply::Options {
    but_workspace::branch::apply::Options {
        new_stack_id: Some(stack_id_for_name),
        allow_applying_already_applied_branch_when_outside_workspace: false,
    }
}

fn unapply_options() -> but_workspace::branch::unapply::Options {
    but_workspace::branch::unapply::Options {
        // TODO: make this the Default once it's KeepWorkspaceReference.
        workspace_disposition: WorkspaceDisposition::KeepWorkspaceReference,
    }
}

/// Legacy because we plan to only have the commits we really need.
fn legacy_unapply_options() -> but_workspace::branch::unapply::Options {
    unapply_options_with(WorkspaceDisposition::KeepWorkspaceCommit)
}

fn unapply_options_with(
    disposition: WorkspaceDisposition,
) -> but_workspace::branch::unapply::Options {
    but_workspace::branch::unapply::Options {
        workspace_disposition: disposition,
    }
}

fn stack_id_for_name(rn: &gix::refs::FullNameRef) -> StackId {
    StackId::from_number_for_testing(rn.shorten().chars().map(|c| c as u128).sum())
}

mod utils {
    pub fn standard_traversal_options() -> but_graph::init::Options {
        but_graph::init::Options {
            collect_tags: true,
            commits_limit_hint: None,
            commits_limit_recharge_location: vec![],
            hard_limit: None,
            dangerously_skip_postprocessing_for_debugging: false,
            worktrees: false,
        }
    }
}
use utils::standard_traversal_options;
