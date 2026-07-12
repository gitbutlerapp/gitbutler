use anyhow::Result;
use but_graph::{Workspace, walk::Seed};
use but_rebase::graph_rebase::{Editor, GraphEditorOptions, testing::Testing as _};
use but_testsupport::{StackState, graph_dag, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::{
    graph_rebase::add_stack_with_segments,
    utils::{fixture, fixture_writable, standard_options},
};

fn project_meta(meta: &impl but_core::RefMetadata) -> but_core::ref_metadata::ProjectMeta {
    meta.workspace(
        but_core::WORKSPACE_REF_NAME
            .try_into()
            .expect("valid workspace ref"),
    )
    .map(|workspace| workspace.project_meta())
    .unwrap_or_default()
}

#[test]
fn four_commits() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
●  120e3a9 c
●  a96434e b
●  d591dfe a
●  35b8235 base
"#]]
    );

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, mut meta) = fixture("merge-in-the-middle")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e8ee978 (HEAD -> with-inner-merge) on top of inner merge
*   2fc288c Merge branch 'B' into with-inner-merge
|\  
| * 984fd1c (B) C: new file with 10 lines
* | add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
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
◎  refs/tags/base
●  8f0d338 base
"#]]
    );

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, mut meta) = fixture("three-branches-merged")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
●      1348870 Merge branches 'A', 'B' and 'C'
├─┬─╮
◎ │ │  refs/heads/A
● │ │  add59d2 A: 10 lines on top
│ ◎ │  refs/heads/B
│ ● │  a748762 B: another 10 lines at the bottom
│ ● │  62e05ba B: 10 lines at the bottom
├─╯ │
│   ◎  refs/heads/C
│   ●  930563a C: add another 10 lines to new file
│   ●  68a2fc3 C: add 10 lines to new file
│   ●  984fd1c C: new file with 10 lines
├───╯
◎  refs/tags/base
●  8f0d338 base
"#]]
    );

    Ok(())
}

#[test]
fn many_references() -> Result<()> {
    let (repo, mut meta) = fixture("many-references")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe (Z, Y, X) a
* 35b8235 base

"#]]
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·120e3a9 (⌂) ►main[🌳]
*  ·a96434e (⌂)
*  ·d591dfe (⌂) ►X, ►Y, ►Z
*  🏁·35b8235 (⌂)
"#]]
    );
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
●  120e3a9 c
●  a96434e b
◎  refs/heads/X
◎  refs/heads/Y
◎  refs/heads/Z
●  d591dfe a
●  35b8235 base
"#]]
    );

    Ok(())
}

#[test]
fn first_parent_leg_long() -> Result<()> {
    let (repo, mut meta) = fixture("first-parent-leg-long")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6ac5745 (HEAD -> with-inner-merge) on top of inner merge
*   d20f547 Merge branch 'B' into with-inner-merge
|\  
| * 984fd1c (B) C: new file with 10 lines
* | 198d2e4 (A) A: 10 more more lines on top
* | 7325853 A: 10 more lines on top
* | add59d2 A: 10 lines on top
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·6ac5745 (⌂) ►with-inner-merge[🌳]
*    ·d20f547 (⌂)
├─╮
* │  ·198d2e4 (⌂) ►A
* │  ·7325853 (⌂)
* │  ·add59d2 (⌂)
│ *  ·984fd1c (⌂) ►B
├─╯
*  🏁·8f0d338 (⌂) ►main, ►tags/base
"#]]
    );
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/with-inner-merge
●  6ac5745 on top of inner merge
●    d20f547 Merge branch 'B' into with-inner-merge
├─╮
◎ │  refs/heads/A
● │  198d2e4 A: 10 more more lines on top
● │  7325853 A: 10 more lines on top
● │  add59d2 A: 10 lines on top
│ ◎  refs/heads/B
│ ●  984fd1c C: new file with 10 lines
├─╯
◎  refs/heads/main
◎  refs/tags/base
●  8f0d338 base
"#]]
    );

    Ok(())
}

#[test]
fn second_parent_leg_long() -> Result<()> {
    let (repo, mut meta) = fixture("second-parent-leg-long")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a6775ea (HEAD -> with-inner-merge) on top of inner merge
*   b85214b Merge branch 'B' into with-inner-merge
|\  
| * f87f875 (B) C: 10 more more lines on top
| * cb181a0 C: 10 more lines on top
| * 984fd1c C: new file with 10 lines
* | add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a6775ea (⌂) ►with-inner-merge[🌳]
*    ·b85214b (⌂)
├─╮
* │  ·add59d2 (⌂) ►A
│ *  ·f87f875 (⌂) ►B
│ *  ·cb181a0 (⌂)
│ *  ·984fd1c (⌂)
├─╯
*  🏁·8f0d338 (⌂) ►main, ►tags/base
"#]]
    );
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/with-inner-merge
●  a6775ea on top of inner merge
●    b85214b Merge branch 'B' into with-inner-merge
├─╮
◎ │  refs/heads/A
● │  add59d2 A: 10 lines on top
│ ◎  refs/heads/B
│ ●  f87f875 C: 10 more more lines on top
│ ●  cb181a0 C: 10 more lines on top
│ ●  984fd1c C: new file with 10 lines
├─╯
◎  refs/heads/main
◎  refs/tags/base
●  8f0d338 base
"#]]
    );

    Ok(())
}

#[test]
fn workspace_with_empty_stack() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-with-empty-stack")?;

    add_stack_with_segments(&mut meta, 1, "stack-1", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-2", StackState::InWorkspace, &[]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   74bcc92 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | 2169646 (stack-1) Commit D
* | 46ef828 Commit C
|/  
| * a0f2ac5 (origin/main, main) Commit X
|/  
* f555940 (stack-2) Commit A
* d664be0 Commit B
* fafd9d0 init

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·a0f2ac5 (⌂|✓) ►main, ►origin/main <> origin/main
│ *  👉·74bcc92 (⌂|🏘)
╭─┤
│ *  ·2169646 (⌂|🏘) ►stack-1
│ *  ·46ef828 (⌂|🏘)
├─╯
*  ·f555940 (⌂|🏘|✓) ►stack-2
*  ·d664be0 (⌂|🏘|✓)
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: 74bcc92: 2169646 f555940
  empty chain anchors: 2169646^ f555940
"#]]
    );
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
●    74bcc92 GitButler Workspace Commit
├─╮
◎ │  refs/heads/stack-1
● │  2169646 Commit D
● │  46ef828 Commit C
│ ◎  refs/heads/stack-2
├─╯
│ ◎  refs/remotes/origin/main (immutable)
│ ◎  refs/heads/main (immutable)
│ ●  a0f2ac5 Commit X
├─╯
●  f555940 Commit A
●  d664be0 Commit B
●  fafd9d0 init
"#]]
    );

    Ok(())
}

#[test]
fn workspace_with_three_empty_stacks() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-with-three-empty-stacks")?;

    add_stack_with_segments(&mut meta, 1, "stack-1", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-2", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "stack-3", StackState::InWorkspace, &[]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 1cf9cf4 (origin/main, main) Commit X
|/  
* fafd9d0 (stack-3, stack-2, stack-1) init

"#]]
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·1cf9cf4 (⌂|✓) ►main, ►origin/main <> origin/main
│ *  👉·a26ae77 (⌂|🏘)
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►stack-1, ►stack-2, ►stack-3
layout:
  materialized parents: a26ae77: fafd9d0 fafd9d0 fafd9d0
  empty chain anchors: fafd9d0 fafd9d0 fafd9d0
"#]]
    );
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
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
"#]]
    );

    Ok(())
}

/// The most common production shape: the target `origin/main` sits at the BASE of the
/// stacks, squarely on the mutability walk from HEAD. Remote-tracking refs must stay
/// immutable regardless of reachability — only push/fetch may move them.
#[test]
fn workspace_with_target_at_stack_base() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-two-stacks")?;

    add_stack_with_segments(&mut meta, 1, "stack-a", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-b", StackState::InWorkspace, &[]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   1162583 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * afc3f8f (stack-b) B2
| * b3ee99c B1
* | 49c06ff (stack-a) A2
* | ff76d2f A1
|/  
* 965998b (origin/main, main) base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
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
◎  refs/heads/main
│ ◎  refs/remotes/origin/main (immutable)
├─╯
●  965998b base
"#]]
    );

    Ok(())
}

/// Same shape but WITHOUT a local `main`: the target `origin/main` itself names the
/// segment owning the integrated base history, placing it directly on the mutability
/// walk from HEAD. It must still come out immutable.
#[test]
fn workspace_with_target_at_stack_base_no_local_main() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-two-stacks")?;
    repo.find_reference("refs/heads/main")?.delete()?;

    add_stack_with_segments(&mut meta, 1, "stack-a", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-b", StackState::InWorkspace, &[]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   1162583 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * afc3f8f (stack-b) B2
| * b3ee99c B1
* | 49c06ff (stack-a) A2
* | ff76d2f A1
|/  
* 965998b (origin/main) base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
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
◎  refs/remotes/origin/main (immutable)
●  965998b base
"#]]
    );

    Ok(())
}

/// Ops that would move, rename, delete, or unhook an immutable reference fail loudly
/// instead of succeeding session-only (materialization would refuse the write anyway).
#[test]
fn ops_on_immutable_refs_fail() -> Result<()> {
    use but_rebase::graph_rebase::{
        Step,
        mutate::InsertSide,
        selector::{SelectorSet, StepRange},
    };

    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-two-stacks")?;
    repo.find_reference("refs/heads/main")?.delete()?;
    add_stack_with_segments(&mut meta, 1, "stack-a", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-b", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let remote = editor.select_reference("refs/remotes/origin/main".try_into()?)?;
    let stack_a = editor.select_reference("refs/heads/stack-a".try_into()?)?;
    let expected = "reference refs/remotes/origin/main is immutable \
                    and cannot be moved, renamed, or deleted";

    // Delete and rename (replace on a ref entry).
    snapbox::assert_data_eq!(
        editor.replace(remote, Step::None).unwrap_err().to_string(),
        snapbox::str![
            "reference refs/remotes/origin/main is immutable and cannot be moved, renamed, or deleted"
        ]
    );
    // Re-point (an edge FROM a reference is its downward link).
    assert_eq!(
        editor
            .insert_edge(remote, stack_a, 0)
            .unwrap_err()
            .to_string(),
        expected
    );
    // Unhook (disconnecting the lone-reference segment).
    assert_eq!(
        editor
            .disconnect_range_from(
                StepRange {
                    child: remote,
                    parent: remote,
                },
                SelectorSet::All,
                SelectorSet::All,
                false,
            )
            .unwrap_err()
            .to_string(),
        expected
    );
    // Move (inserting the lone-reference segment elsewhere).
    assert_eq!(
        editor
            .insert_range(
                stack_a,
                StepRange {
                    child: remote,
                    parent: remote,
                },
                InsertSide::Above,
            )
            .unwrap_err()
            .to_string(),
        expected
    );
    // Splitting the reference off its pick (a pick inserted below re-points it).
    let new_commit = {
        let base = repo.rev_parse_single("refs/remotes/origin/main")?;
        let base = base.object()?.into_commit();
        repo.write_object(gix::objs::Commit {
            tree: base.tree_id()?.detach(),
            parents: Default::default(),
            author: base.author()?.into(),
            committer: base.committer()?.into(),
            encoding: None,
            message: "detached".into(),
            extra_headers: vec![],
        })?
        .detach()
    };
    assert_eq!(
        editor
            .insert(remote, Step::new_pick(new_commit), InsertSide::Below)
            .unwrap_err()
            .to_string(),
        expected
    );

    // A mutable ref standing beside the immutable one is untouched by the guard.
    editor.replace(stack_a, Step::None)?;

    Ok(())
}

#[test]
fn commit_with_two_parents() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("single-commit")?;

    let base = repo.rev_parse_single("HEAD")?;
    let base = base.object()?.into_commit();
    repo.commit("HEAD", "a", base.tree_id()?, vec![base.id(), base.id()])?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d70d863 (HEAD -> main) a
|\
* 35b8235 base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·d70d863 (⌂) ►main[🌳]
*  🏁·35b8235 (⌂)
"#]]
    );
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
●  d70d863 a
●  35b8235 base
"#]]
    );

    Ok(())
}

#[test]
fn includes_extra_refs_in_editor_creation() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-with-empty-stack")?;
    add_stack_with_segments(&mut meta, 1, "stack-1", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-2", StackState::InWorkspace, &[]);

    let main_ref = gix::refs::FullName::try_from("refs/heads/main")?;

    {
        let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

        snapbox::assert_data_eq!(
            editor.steps_ascii(),
            snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
●    74bcc92 GitButler Workspace Commit
├─╮
◎ │  refs/heads/stack-1
● │  2169646 Commit D
● │  46ef828 Commit C
│ ◎  refs/heads/stack-2
├─╯
│ ◎  refs/remotes/origin/main (immutable)
│ ◎  refs/heads/main (immutable)
│ ●  a0f2ac5 Commit X
├─╯
●  f555940 Commit A
●  d664be0 Commit B
●  fafd9d0 init
"#]]
        );
    }

    {
        let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let editor = Editor::create_with_opts(
            ws.commit_graph(),
            ws.project_meta(),
            &mut *meta,
            &repo,
            &GraphEditorOptions {
                extra_mutable_refs: vec![main_ref.clone()],
                ..<_>::default()
            },
        )?;

        snapbox::assert_data_eq!(
            editor.steps_ascii(),
            snapbox::str![[r#"
◎  refs/heads/gitbutler/workspace
●    74bcc92 GitButler Workspace Commit
├─╮
◎ │  refs/heads/stack-1
● │  2169646 Commit D
● │  46ef828 Commit C
│ ◎  refs/heads/stack-2
├─╯
│ ◎  refs/remotes/origin/main (immutable)
│ ◎  refs/heads/main
│ ●  a0f2ac5 Commit X
├─╯
●  f555940 Commit A
●  d664be0 Commit B
●  fafd9d0 init
"#]]
        );
    }

    Ok(())
}

/// When the first parent of a merge has an earlier committer timestamp
/// than the second parent, the but-graph traversal queue sort processes
/// the second parent first. This causes edges to be created in an order
/// that doesn't match parent_ids, which the editor must correct.
#[test]
fn merge_first_parent_older_than_second() -> Result<()> {
    let (repo, mut meta) = fixture("merge-first-parent-older")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 738ea18 (HEAD -> first-parent) commit on top of merge
*   408ca26 merge second-parent into first-parent
|\  
| * 75369b0 (second-parent) new commit 3 on second-parent
| * 553bbf7 new commit 2 on second-parent
| * 72614bb new commit 1 on second-parent
* | 2854fa2 old commit on first-parent
|/  
* 793a434 (tag: base, main) base

"#]]
        .raw()
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·738ea18 (⌂) ►first-parent[🌳]
*    ·408ca26 (⌂)
├─╮
* │  ·2854fa2 (⌂)
│ *  ·75369b0 (⌂) ►second-parent
│ *  ·553bbf7 (⌂)
│ *  ·72614bb (⌂)
├─╯
*  🏁·793a434 (⌂) ►main, ►tags/base
"#]]
    );
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/first-parent
●  738ea18 commit on top of merge
●    408ca26 merge second-parent into first-parent
├─╮
● │  2854fa2 old commit on first-parent
│ ◎  refs/heads/second-parent
│ ●  75369b0 new commit 3 on second-parent
│ ●  553bbf7 new commit 2 on second-parent
│ ●  72614bb new commit 1 on second-parent
├─╯
◎  refs/heads/main
◎  refs/tags/base
●  793a434 base
"#]]
    );

    Ok(())
}

#[test]
fn immutable_entrypoints_propogate_until_mutable_entrypoints() -> Result<()> {
    let (repo, mut meta) = fixture("extra-refs-to-include")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* be4ae80 (main, explicit-const) d
* 120e3a9 (implicit-const) c
* a96434e (explicit-mut) b
| * d9fa122 (explicit-const-2) g
| * 85bccf0 (implicit-const-2) f
| * c8dd361 (HEAD, implicit-mut) e
|/  
* d591dfe (foo) a
* 35b8235 base

"#]]
    );

    let ws = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(
                repo.rev_parse_single("refs/heads/implicit-mut")?.detach(),
                Some("refs/heads/implicit-mut".try_into()?),
            ),
            Seed::reachable(
                repo.rev_parse_single("refs/heads/explicit-const")?.detach(),
                Some("refs/heads/explicit-const".try_into()?),
            ),
            Seed::reachable(
                repo.rev_parse_single("refs/heads/explicit-const-2")?
                    .detach(),
                Some("refs/heads/explicit-const-2".try_into()?),
            ),
        ],
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·be4ae80 (⌂) ►explicit-const, ►main
*  ·120e3a9 (⌂) ►implicit-const
*  ·a96434e (⌂) ►explicit-mut
│ *  ·d9fa122 (⌂) ►explicit-const-2
│ *  ·85bccf0 (⌂) ►implicit-const-2
│ *  👉·c8dd361 (⌂) ►implicit-mut
├─╯
*  ·d591dfe (⌂) ►foo
*  🏁·35b8235 (⌂)
"#]]
    );

    let opts = GraphEditorOptions {
        extra_mutable_refs: vec!["refs/heads/explicit-mut".try_into()?],
        ..Default::default()
    };
    let editor = Editor::create_with_opts(
        ws.commit_graph(),
        ws.project_meta(),
        &mut *meta,
        &repo,
        &opts,
    )?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/explicit-const (immutable)
◎  refs/heads/main (immutable)
●  be4ae80 d
◎  refs/heads/implicit-const (immutable)
●  120e3a9 c
◎  refs/heads/explicit-mut
●  a96434e b
│ ◎  refs/heads/explicit-const-2 (immutable)
│ ●  d9fa122 g
│ ◎  refs/heads/implicit-const-2 (immutable)
│ ●  85bccf0 f
│ ◎  refs/heads/implicit-mut
│ ●  c8dd361 e
├─╯
◎  refs/heads/foo
●  d591dfe a
●  35b8235 base
"#]]
    );

    Ok(())
}
