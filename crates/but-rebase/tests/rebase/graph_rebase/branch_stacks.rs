use anyhow::Result;
use but_core::{RefMetadata as _, RepositoryExt as _, ref_metadata::ProjectMeta};
use but_graph::Graph;
use but_meta::{BranchOrderMetadata, VirtualBranchesTomlMetadata};
use but_rebase::graph_rebase::{Editor, Step, mutate::InsertSide, testing::Testing as _};
use but_testsupport::{StackState, graph_tree, invoke_bash};

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

    // `main` rests on the target commit and tracks the target, so no stack owns it.
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
fn a_branch_on_the_target_commit_joins_the_first_stack_unless_it_tracks_the_target() -> Result<()> {
    let (repo, _tmp, mut meta, mut db) = fixture_writable("workspace-two-stacks")?;
    for (idx, stack) in ["stack-a", "stack-b"].into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, stack, StackState::InWorkspace, &[]);
    }
    let project_meta = ProjectMeta {
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        ..Default::default()
    };

    // Without a target ref nothing tracks the target, and `stack-a` reaches `main` first.
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
●      a26ae77 GitButler Workspace Commit
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
fn a_workspace_reference_without_a_workspace_commit_is_one_ad_hoc_stack() -> Result<()> {
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
ad hoc
  main

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
