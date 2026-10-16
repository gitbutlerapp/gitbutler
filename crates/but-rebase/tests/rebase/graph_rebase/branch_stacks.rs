use anyhow::Result;
use but_core::{RefMetadata as _, RepositoryExt as _, ref_metadata::ProjectMeta};
use but_graph::Graph;
use but_meta::{BranchOrderMetadata, VirtualBranchesTomlMetadata};
use but_rebase::graph_rebase::{Editor, Step, mutate::InsertSide, testing::Testing as _};
use but_testsupport::{StackState, graph_tree, invoke_bash};
use gix::prelude::ObjectIdExt as _;

use crate::{
    graph_rebase::add_stack_with_segments,
    utils::{fixture_writable, standard_options, target_meta},
};

fn render_stacks(stacks: &[Vec<gix::refs::FullName>]) -> String {
    stacks
        .iter()
        .map(|stack| {
            let names: Vec<_> = stack
                .iter()
                .map(|name| name.shorten().to_string())
                .collect();
            format!("  {}\n", names.join(" > "))
        })
        .collect()
}

fn branch_stacks(
    repo: &gix::Repository,
    meta: &mut VirtualBranchesTomlMetadata,
    db: &mut but_db::DbHandle,
    project_meta: ProjectMeta,
    worktrees: bool,
) -> Result<String> {
    if worktrees {
        db.worktree_meta_mut().mark_adopted()?;
    }
    let options = but_graph::init::Options {
        worktrees,
        ..standard_options()
    };
    let graph = Graph::from_head(repo, &*meta, project_meta, db, options)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let rebase = Editor::create(&mut ws, meta, repo, db)?.rebase()?;
    let stacks = rebase.branch_stacks()?;
    let workspace = match &stacks.workspace {
        Some(stacks) => format!("workspace\n{}", render_stacks(stacks)),
        None => String::new(),
    };
    Ok(format!(
        "{}\n{workspace}ad hoc\n{}",
        rebase.steps_ascii(),
        render_stacks(&stacks.ad_hoc)
    ))
}

#[test]
fn each_workspace_commit_parent_is_a_stack() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-two-stacks")?;
    for (idx, stack) in ["stack-a", "stack-b"].into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, stack, StackState::InWorkspace, &[]);
    }

    // `main` is the configured local target at the cutoff, so no stack owns it.
    snapbox::assert_data_eq!(
        branch_stacks(&repo, &mut meta, &mut db, target_meta(&repo), false)?,
        snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
●    1162583 GitButler Workspace Commit
├─╮
◎ │  refs/heads/stack-a
● │  49c06ff A2
● │  ff76d2f A1
│ ◎  refs/heads/stack-b
│ ●  afc3f8f B2
│ ●  b3ee99c B1
├─╯
│ ◎  refs/remotes/origin/main (immutable)
├─╯
◎  refs/heads/main
●  965998b base
workspace
  stack-a
  stack-b
ad hoc

"#]]
    );
    Ok(())
}

#[test]
fn a_branch_on_the_target_commit_joins_the_first_stack_without_a_configured_target() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-two-stacks")?;
    for (idx, stack) in ["stack-a", "stack-b"].into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, stack, StackState::InWorkspace, &[]);
    }
    let project_meta = ProjectMeta {
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        ..Default::default()
    };

    // Without a configured target, `main` is an ordinary branch and `stack-a` reaches it first.
    snapbox::assert_data_eq!(
        branch_stacks(&repo, &mut meta, &mut db, project_meta, false)?,
        snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
●    1162583 GitButler Workspace Commit
├─╮
◎ │  refs/heads/stack-a
● │  49c06ff A2
● │  ff76d2f A1
│ ◎  refs/heads/stack-b
│ ●  afc3f8f B2
│ ●  b3ee99c B1
├─╯
│ ◎  refs/remotes/origin/main (immutable)
├─╯
◎  refs/heads/main
●  965998b base
workspace
  stack-a > main
  stack-b
ad hoc

"#]]
    );
    Ok(())
}

#[test]
fn empty_stacks_stay_apart() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-with-three-empty-stacks")?;
    for (idx, stack) in ["stack-1", "stack-2", "stack-3"].into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, stack, StackState::InWorkspace, &[]);
    }

    // All three rest on the same commit, each beneath its own workspace commit parent.
    snapbox::assert_data_eq!(
        branch_stacks(&repo, &mut meta, &mut db, target_meta(&repo), false)?,
        snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
●      2bdc06b GitButler Workspace Commit
├─┬─╮
◎ │ │  refs/heads/stack-1
│ ◎ │  refs/heads/stack-2
├─╯ │
│   ◎  refs/heads/stack-3
├───╯
│ ◎  refs/remotes/origin/main (immutable)
│ ◎  refs/heads/main (immutable)
│ ●  1cf9cf4 Commit X
├─╯
●  fafd9d0 init
workspace
  stack-1
  stack-2
  stack-3
ad hoc

"#]]
    );
    Ok(())
}

#[test]
fn a_workspace_reference_without_a_workspace_commit_is_one_workspace_stack() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;

    // GitButler's own references never stack, which leaves `main` below `HEAD`.
    snapbox::assert_data_eq!(
        branch_stacks(&repo, &mut meta, &mut db, ProjectMeta::default(), false)?,
        snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
●  1b78c63 just a normal commit
│ ◎  refs/remotes/origin/main (immutable)
├─╯
◎  refs/heads/main
●  4d41a5c one
●  965998b base
workspace
  main
ad hoc

"#]]
    );
    Ok(())
}

#[test]
fn an_ad_hoc_stack_follows_first_parents() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("merge-in-the-middle")?;

    // `B` is merged in as a second parent, so it is not part of what `HEAD` stacks on.
    snapbox::assert_data_eq!(
        branch_stacks(&repo, &mut meta, &mut db, ProjectMeta::default(), false)?,
        snapbox::str![[r#"
◎  refs/heads/with-inner-merge
●  e8ee978 on top of inner merge
●    2fc288c Merge branch 'B' into with-inner-merge
├─╮
◎ │  refs/heads/A
● │  add59d2 A: 10 lines on top
│ ◎  refs/heads/B
│ ●  984fd1c C: new file with 10 lines
├─╯
◎  refs/heads/main
◎  refs/tags/base (immutable)
●  8f0d338 base
ad hoc
  with-inner-merge > A > main

"#]]
    );
    Ok(())
}

#[test]
fn a_detached_head_is_an_ad_hoc_stack() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("merge-in-the-middle")?;
    invoke_bash("git checkout --detach A", &repo);

    // The stack starts at the commit `HEAD` is detached at, beneath the references resting on it.
    snapbox::assert_data_eq!(
        branch_stacks(&repo, &mut meta, &mut db, ProjectMeta::default(), false)?,
        snapbox::str![[r#"
◎  refs/heads/A
●  add59d2 A: 10 lines on top
◎  refs/heads/main
◎  refs/tags/base (immutable)
●  8f0d338 base
ad hoc
  main

"#]]
    );
    Ok(())
}

#[test]
fn a_worktree_owns_the_branch_it_has_checked_out() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("worktree-checkout-heads")?;

    // `middle` is in `main`'s history, and still stacks with the worktree that has it checked out.
    snapbox::assert_data_eq!(
        branch_stacks(&repo, &mut meta, &mut db, ProjectMeta::default(), true)?,
        snapbox::str![[r#"
◎  refs/heads/main
●  a96434e b
│ ◎  refs/heads/middle
├─╯
●  d591dfe a
●  35b8235 base
ad hoc
  main
  middle

"#]]
    );
    Ok(())
}

fn local_target_workspace(
    repo: &gix::Repository,
    meta: &VirtualBranchesTomlMetadata,
    db: &mut but_db::DbHandle,
    project_meta: ProjectMeta,
) -> Result<but_graph::Workspace> {
    Graph::from_head(repo, meta, project_meta, db, standard_options())?
        .validated()?
        .into_workspace()
}

#[test]
fn upstream_cutoff_keeps_local_target_work_visible() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash("git branch -f main HEAD", &repo);
    let mut project_meta = target_meta(&repo);
    project_meta.target_commit_id = None;
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    let rebase = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?.rebase()?;
    // Main is ahead of its upstream and must remain a visible local branch.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        "  main\n"
    );
    Ok(())
}

#[test]
fn upstream_cutoff_preserves_divergent_local_target_work() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("diverged-local-target")?;
    let local_tip = repo.rev_parse_single("main")?.detach();
    let upstream_tip = repo.rev_parse_single("origin/main")?.detach();
    let merge_base = repo
        .merge_base(local_tip, upstream_tip)?
        .expect("shared base")
        .detach();
    assert_ne!(merge_base, local_tip, "local main has unique work");
    assert_ne!(merge_base, upstream_tip, "upstream has unique work");
    let mut project_meta = target_meta(&repo);
    project_meta.target_commit_id = None;
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    assert_eq!(
        ws.target_commit.as_ref().map(|target| target.commit_id),
        Some(upstream_tip),
        "without a stored cutoff the workspace uses the upstream tip, not local main"
    );
    let rebase = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?.rebase()?;
    let stacks = rebase.branch_stacks()?;
    // Main stays visible because its divergent work is not the upstream boundary.
    snapbox::assert_data_eq!(
        render_stacks(stacks.workspace.as_ref().expect("managed workspace")),
        "  main\n"
    );
    assert_eq!(
        rebase.reference_target("refs/heads/landed".try_into()?)?,
        upstream_tip,
        "the branch resting on the cutoff remains a ref, but has no stack lane"
    );
    Ok(())
}

#[test]
fn local_target_is_the_cutoff_without_an_upstream() -> Result<()> {
    let (mut repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash("git branch -f main HEAD", &repo);
    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("branch.main.remote", "missing")?;
    }
    let mut project_meta = target_meta(&repo);
    project_meta.target_commit_id = None;
    assert!(
        project_meta.remote_target_ref(&repo)?.is_none(),
        "the fixture has no resolvable upstream"
    );
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    let rebase = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?.rebase()?;
    // Main itself is the boundary, so it is not a stack branch.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        ""
    );
    Ok(())
}

#[test]
fn upstream_cutoff_survives_a_missing_reference_step() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash("git branch feature HEAD; git branch older main~1", &repo);
    let mut project_meta = target_meta(&repo);
    project_meta.target_commit_id = None;
    let cutoff = repo.rev_parse_single("origin/main")?.detach();
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let upstream = editor.select_reference("refs/remotes/origin/main".try_into()?)?;
    editor.replace_with_none(upstream)?;
    assert!(
        editor.try_select_commit(cutoff).is_some(),
        "only the upstream reference step was removed"
    );
    let rebase = editor.rebase()?;
    // The upstream commit still bounds traversal, excluding main and older history.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        "  feature\n"
    );
    Ok(())
}

#[test]
fn missing_upstream_reference_uses_the_rewritten_tip() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash("git branch feature HEAD; git branch older main~1", &repo);
    let mut project_meta = target_meta(&repo);
    project_meta.target_commit_id = None;
    let cutoff = repo.rev_parse_single("origin/main")?.detach();
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let upstream = editor.select_reference("refs/remotes/origin/main".try_into()?)?;
    editor.replace_with_none(upstream)?;
    let mut replacement = but_core::Commit::from_id(cutoff.attach(&repo))?;
    replacement.message = "rewritten upstream boundary".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    editor.amend_pick(cutoff, replacement)?;
    let rebase = editor.rebase()?;
    assert!(
        rebase.history.commit_mappings().contains_key(&cutoff),
        "the upstream tip was rewritten"
    );
    // Resolving the same upstream's repository tip must follow its rewrite mapping.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        "  feature\n"
    );
    Ok(())
}

#[test]
fn local_only_cutoff_follows_a_ref_only_move_in_the_editor() -> Result<()> {
    let (mut repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash("git branch feature HEAD", &repo);
    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("branch.main.remote", "missing")?;
    }
    let mut project_meta = target_meta(&repo);
    project_meta.target_commit_id = None;
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let main = editor.select_reference("refs/heads/main".try_into()?)?;
    editor.replace_with_none(main)?;
    let tip = editor.select_commit(repo.head_id()?.detach())?;
    editor.insert(
        tip,
        Step::new_reference("refs/heads/main".try_into()?),
        InsertSide::Above,
    )?;
    let rebase = editor.rebase()?;
    assert!(
        rebase.history.commit_mappings().is_empty(),
        "moving main must not require rewriting any commit"
    );
    // Main is the boundary at its new position, not a branch above the old repository tip.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        "  feature\n"
    );
    Ok(())
}

#[test]
fn another_local_branch_sharing_the_target_upstream_remains_visible() -> Result<()> {
    let (mut repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash("git branch other main", &repo);
    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("branch.other.remote", "origin")?;
        config.set_raw_value("branch.other.merge", "refs/heads/main")?;
    }
    let mut project_meta = target_meta(&repo);
    // Keep other above the cutoff: branches resting on it have no stack lane.
    project_meta.target_commit_id = Some(repo.rev_parse_single("main~1")?.detach());
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    let rebase = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?.rebase()?;
    // Only the configured local main is excluded as upstream, not other.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        "  other\n"
    );
    Ok(())
}

#[test]
fn rewritten_stored_cutoff_uses_its_commit_mapping() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash(
        "git branch boundary main; git branch older main~1; git branch -f main HEAD",
        &repo,
    );
    let project_meta = target_meta(&repo);
    let cutoff = project_meta
        .target_commit_id
        .expect("fixture has a stored cutoff");
    let mut ws = local_target_workspace(&repo, &meta, &mut db, project_meta)?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let mut replacement = but_core::Commit::from_id(cutoff.attach(&repo))?;
    replacement.message = "rewritten boundary".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    editor.amend_pick(cutoff, replacement)?;
    let rebase = editor.rebase()?;
    assert!(
        rebase.history.commit_mappings().contains_key(&cutoff),
        "the stored cutoff was rewritten, not excluded"
    );
    // The boundary reference is on a separate lane, while main reaches the rewritten cutoff.
    snapbox::assert_data_eq!(
        rebase.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
◎  refs/heads/main
●  92660d6 just a normal commit
│ ◎  refs/remotes/origin/main (immutable)
│ ◎  refs/heads/boundary (immutable)
├─╯
●  e47fc3a rewritten boundary
◎  refs/heads/older (immutable)
●  965998b base
"#]]
    );
    // Stop at the rewritten boundary, retaining main but excluding older history.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        "  main\n"
    );
    Ok(())
}

#[test]
fn excluded_stored_cutoff_does_not_fall_back_to_a_target_tip() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-without-managed-commit")?;
    invoke_bash(
        "git branch boundary main; git branch older main~1; git branch -f main HEAD",
        &repo,
    );
    let mut ws = local_target_workspace(&repo, &meta, &mut db, ProjectMeta::default())?;
    let mut excluded = but_core::Commit::from_id(repo.head_id()?)?;
    excluded.parents.clear();
    excluded.message = "excluded boundary".into();
    let cutoff = repo.write_object(excluded.inner)?.detach();
    // Supply a boundary outside this graph without invoking projection's own fallback policy.
    ws.graph.project_meta = target_meta(&repo);
    ws.graph.project_meta.target_commit_id = Some(cutoff);
    let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    assert!(
        editor.try_select_commit(cutoff).is_none(),
        "the excluded cutoff has no editor step"
    );
    let rebase = editor.rebase()?;
    assert!(
        !rebase.history.commit_mappings().contains_key(&cutoff),
        "an excluded cutoff has no rewrite mapping"
    );
    // No encountered commit is the cutoff; neither target tip may replace it.
    snapbox::assert_data_eq!(
        render_stacks(
            rebase
                .branch_stacks()?
                .workspace
                .as_ref()
                .expect("workspace reference")
        ),
        "  main > boundary > older\n"
    );
    Ok(())
}

#[test]
fn materializing_rebuilds_the_workspace_stacks() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-two-stacks")?;
    add_stack_with_segments(&mut meta, 0, "stack-b", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &["below"]);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;

    let outcome = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?
        .rebase()?
        .materialize(Default::default())?;

    let workspace = outcome
        .meta
        .workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    // `stack-b` keeps its id, `stack-a` is new to the metadata and precedes it as the first parent
    // of the workspace commit, and what isn't a parent of the workspace commit is gone.
    snapbox::assert_data_eq!(
        format!("{:#?}", workspace.stacks),
        snapbox::str![[r#"
[
    WorkspaceStack {
        id: [..],
        branches: [
            WorkspaceStackBranch {
                ref_name: "refs/heads/stack-a",
                archived: false,
            },
        ],
        workspacecommit_relation: Merged,
    },
    WorkspaceStack {
        id: 00000000-0000-0000-0000-000000000000,
        branches: [
            WorkspaceStackBranch {
                ref_name: "refs/heads/stack-b",
                archived: false,
            },
        ],
        workspacecommit_relation: Merged,
    },
]
"#]]
    );
    Ok(())
}

#[test]
fn materializing_persists_an_order_that_moves_no_reference() -> Result<()> {
    let (repo, _tmp, _meta, mut db) = fixture_writable("many-references")?;
    let mut meta = BranchOrderMetadata::from_paths(
        repo.path().join("virtual_branches.toml"),
        repo.gitbutler_storage_path()?,
    )?;
    let graph = Graph::from_head(
        &repo,
        &meta,
        ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;

    let x = editor.select_reference("refs/heads/X".try_into()?)?;
    editor.replace_with_none(x)?;
    let z = editor.select_reference("refs/heads/Z".try_into()?)?;
    editor.insert(
        z,
        Step::new_reference("refs/heads/X".try_into()?),
        InsertSide::Below,
    )?;
    let outcome = editor.rebase()?.materialize(Default::default())?;

    // `X`, `Y` and `Z` all rest on one commit, so only the metadata can tell `X` went to the bottom.
    snapbox::assert_data_eq!(
        graph_tree(&outcome.workspace.graph).to_string(),
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·120e3a9 (⌂)
    └── ·a96434e (⌂)
        └── ►:2[1]:Y
            └── ►:3[2]:Z
                └── ►:1[3]:X
                    ├── ·d591dfe (⌂)
                    └── 🏁·35b8235 (⌂)

"#]]
    );
    Ok(())
}
