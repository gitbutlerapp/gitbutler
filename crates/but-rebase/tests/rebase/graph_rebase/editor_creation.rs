use anyhow::Result;
use but_graph::{Graph, init::Overlay};
use but_rebase::graph_rebase::{
    Editor, GraphEditorOptions, LookupStep, Step, testing::Testing as _,
};
use but_testsupport::{StackState, graph_tree, visualize_commit_graph_all};
use snapbox::IntoData;

use crate::{
    graph_rebase::add_stack_with_segments,
    utils::{fixture, fixture_writable},
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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
│ ◎  refs/heads/with-inner-merge
│ ●  e8ee978 on top of inner merge
│ ●    2fc288c Merge branch 'B' into with-inner-merge
│ ├─╮
│ ◎ │  refs/heads/A
│ ● │  add59d2 A: 10 lines on top
├─╯ │
│   ◎  refs/heads/B
│   ●  984fd1c C: new file with 10 lines
├───╯
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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  X
│ ◎  Y
├─╯
│ ◎  Z
├─╯
│ ◎  main[🌳]
│ ●  👉·120e3a9 (→)
│ ●  ·a96434e (→)
├─╯
●  ·d591dfe (→)
●  🏁·35b8235 (→)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/X
│ ◎  refs/heads/Y
├─╯
│ ◎  refs/heads/Z
├─╯
│ ◎  refs/heads/main
│ ●  120e3a9 c
│ ●  a96434e b
├─╯
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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main
│ ◎  with-inner-merge[🌳]
│ ●  👉·6ac5745 (→)
│ ●    ·d20f547 (→)
│ ├─╮
│ ◎ │  A
│ ● │  ·198d2e4 (→)
│ ● │  ·7325853 (→)
│ ● │  ·add59d2 (→)
├─╯ │
│   ◎  B
│   ●  ·984fd1c (→)
├───╯
●  🏁·8f0d338 (→)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
│ ◎  refs/heads/with-inner-merge
│ ●  6ac5745 on top of inner merge
│ ●    d20f547 Merge branch 'B' into with-inner-merge
│ ├─╮
│ ◎ │  refs/heads/A
│ ● │  198d2e4 A: 10 more more lines on top
│ ● │  7325853 A: 10 more lines on top
│ ● │  add59d2 A: 10 lines on top
├─╯ │
│   ◎  refs/heads/B
│   ●  984fd1c C: new file with 10 lines
├───╯
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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main
│ ◎  with-inner-merge[🌳]
│ ●  👉·a6775ea (→)
│ ●    ·b85214b (→)
│ ├─╮
│ ◎ │  A
│ ● │  ·add59d2 (→)
├─╯ │
│   ◎  B
│   ●  ·f87f875 (→)
│   ●  ·cb181a0 (→)
│   ●  ·984fd1c (→)
├───╯
●  🏁·8f0d338 (→)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
│ ◎  refs/heads/with-inner-merge
│ ●  a6775ea on top of inner merge
│ ●    b85214b Merge branch 'B' into with-inner-merge
│ ├─╮
│ ◎ │  refs/heads/A
│ ● │  add59d2 A: 10 lines on top
├─╯ │
│   ◎  refs/heads/B
│   ●  f87f875 C: 10 more more lines on top
│   ●  cb181a0 C: 10 more lines on top
│   ●  984fd1c C: new file with 10 lines
├───╯
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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  👉·74bcc92 (→)
╭─┬─╯
◎ │  📙stack-1
● │  ·2169646 (→)
● │  ·46ef828 (→)
│ ◎  📙stack-2
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
│ ●  ·a0f2ac5 (→|←)
├─╯
●  ✂·f555940 (→|←)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎      refs/heads/gitbutler/workspace
├─┬─╮
│ │ ●  74bcc92 GitButler Workspace Commit
╭─┬─╯
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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎        📕gitbutler/workspace[🌳]
├─┬─┬─╮
│ ◎ │ │  📙stack-2
│ │ ◎ │  📙stack-3
│ ├─╯ │
│ │   ●  👉·a26ae77 (→)
├─────╯
◎ │  📙stack-1
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
│ ●  ·1cf9cf4 (→|←)
├─╯
●  🏁·fafd9d0 (→|←)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎        refs/heads/gitbutler/workspace
├─┬─┬─╮
│ ◎ │ │  refs/heads/stack-2
│ │ ◎ │  refs/heads/stack-3
│ ├─╯ │
│ │   ●  a26ae77 GitButler Workspace Commit
├─────╯
◎ │  refs/heads/stack-1
├─╯
│ ◎  refs/remotes/origin/main (immutable)
│ ◎  refs/heads/main (immutable)
│ ●  1cf9cf4 Commit X
├─╯
●  fafd9d0 init
"#]]
    );

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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
●  👉·d70d863 (→)
●  🏁·35b8235 (→)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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
    let workspace_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;
    let workspace_commit = repo.rev_parse_single(workspace_ref.as_ref())?.detach();

    {
        let graph = but_graph::Graph::from_repo(
            &repo,
            &*meta,
            project_meta(&*meta),
            but_graph::init::Overlay::default(),
        )?
        .validated()?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

        snapbox::assert_data_eq!(
            editor.steps_ascii(),
            snapbox::str![[r#"
◎      refs/heads/gitbutler/workspace
├─┬─╮
│ │ ●  74bcc92 GitButler Workspace Commit
╭─┬─╯
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
"#]]
        );
        let (_, target) = editor.find_reference_target(workspace_ref.as_ref())?;
        assert_eq!(
            target.id, workspace_commit,
            "workspace overlays must not replace the workspace ref's Git target"
        );
    }

    {
        let graph = but_graph::Graph::from_repo(
            &repo,
            &*meta,
            project_meta(&*meta),
            but_graph::init::Overlay::default(),
        )?
        .validated()?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create_with_opts(
            &mut ws,
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
◎      refs/heads/gitbutler/workspace
├─┬─╮
│ │ ●  74bcc92 GitButler Workspace Commit
╭─┬─╯
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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  first-parent[🌳]
●  👉·738ea18 (→)
●    ·408ca26 (→)
├─╮
● │  ·2854fa2 (→)
│ ◎  second-parent
│ ●  ·75369b0 (→)
│ ●  ·553bbf7 (→)
│ ●  ·72614bb (→)
├─╯
│ ◎  main
├─╯
●  🏁·793a434 (→)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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
│ ◎  refs/heads/main
├─╯
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

    let entrypoint_id = repo.rev_parse_single("refs/heads/implicit-mut")?.detach();
    let entrypoint_ref = "refs/heads/implicit-mut".try_into()?;
    let second_immutable_id = repo
        .rev_parse_single("refs/heads/explicit-const-2")?
        .detach();
    let target_ref: gix::refs::FullName = "refs/remotes/origin/explicit-const-2".try_into()?;
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some(target_ref.clone()),
        target_commit_id: Some(repo.rev_parse_single("refs/heads/explicit-const")?.detach()),
        ..project_meta(&*meta)
    };
    let graph = Graph::from_repo(
        &repo,
        &*meta,
        project_meta,
        Overlay::default()
            .with_references([gix::refs::Reference {
                name: target_ref,
                target: gix::refs::Target::Object(second_immutable_id),
                peeled: Some(second_immutable_id),
            }])
            .with_entrypoint(entrypoint_id, Some(entrypoint_ref)),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let opts = GraphEditorOptions {
        extra_mutable_refs: vec!["refs/heads/explicit-mut".try_into()?],
        ..Default::default()
    };
    let editor = Editor::create_with_opts(&mut ws, &mut *meta, &repo, &opts)?;

    for (name, expected_mutable) in [
        ("refs/heads/implicit-mut", true),
        ("refs/heads/explicit-mut", true),
        ("refs/heads/foo", true),
        ("refs/heads/main", false),
        ("refs/heads/implicit-const", false),
        ("refs/heads/explicit-const", false),
        ("refs/heads/implicit-const-2", false),
        ("refs/heads/explicit-const-2", false),
        ("refs/remotes/origin/explicit-const-2", false),
    ] {
        let name: gix::refs::FullName = name.try_into()?;
        let selector = editor.select_reference(name.as_ref())?;
        let Step::Reference { mutable, .. } = editor.lookup_step(selector)? else {
            unreachable!("selected a reference")
        };
        assert_eq!(mutable, expected_mutable, "mutability of {name}");
    }

    Ok(())
}

#[test]
fn unborn_head_is_a_single_mutable_reference() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = gix::init(tmp.path())?;
    let mut meta = std::mem::ManuallyDrop::new(but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?);
    let head_name = repo.head_name()?.expect("unborn HEAD is symbolic");

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        Default::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    assert_eq!(editor.steps_ascii(), format!("◎  {head_name}"));
    Ok(())
}

/// A metadata-backed workspace reference whose tip is a plain commit rather
/// than a managed workspace commit still creates an editor: the workspace ref
/// is an ordinary mutable reference above its actual target.
#[test]
fn workspace_reference_without_managed_commit() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-without-managed-commit")?;
    add_stack_with_segments(&mut meta, 1, "main", StackState::InWorkspace, &[]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 1b78c63 (HEAD -> gitbutler/workspace) just a normal commit
* 4d41a5c (origin/main, main) one
* 965998b base

"#]]
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎    refs/heads/gitbutler/workspace
├─╮
│ ●  1b78c63 just a normal commit
├─╯
│ ◎  refs/remotes/origin/main (immutable)
├─╯
◎  refs/heads/main
●  4d41a5c one
●  965998b base
"#]]
    );
    let workspace_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;
    let (_, target) = editor.find_reference_target(workspace_ref.as_ref())?;
    assert_eq!(
        target.id,
        repo.rev_parse_single("gitbutler/workspace")?.detach(),
        "the workspace ref targets its actual commit, not a stack overlay"
    );

    Ok(())
}

/// A remote-tracking reference placed inline on the HEAD ancestry (its remote
/// is behind the local branch) is reachable from the mutable entrypoint, but
/// must never become mutable itself.
#[test]
fn inline_remote_ref_on_head_ancestry_stays_immutable() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
    let behind = repo.rev_parse_single("main~2")?.detach();
    repo.reference(
        "refs/remotes/origin/main",
        behind,
        gix::refs::transaction::PreviousValue::Any,
        "remote behind local",
    )?;

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
●  120e3a9 c
●  a96434e b
◎  refs/remotes/origin/main (immutable)
●  d591dfe a
●  35b8235 base
"#]]
    );

    Ok(())
}
