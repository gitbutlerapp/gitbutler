use anyhow::Result;
use but_graph::{Graph, init::Tip};
use but_rebase::graph_rebase::{Editor, GraphEditorOptions, testing::Testing as _};
use but_testsupport::{StackState, graph_tree, visualize_commit_graph_all};
use snapbox::IntoData;

use crate::{
    graph_rebase::add_stack_with_segments,
    utils::{fixture, fixture_writable, standard_options, target_meta},
};

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·120e3a9 (⌂|1)
    ├── ·a96434e (⌂|1)
    ├── ·d591dfe (⌂|1) ►X, ►Y, ►Z
    └── 🏁·35b8235 (⌂|1)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

└── 👉►:0[0]:with-inner-merge[🌳]
    └── ·6ac5745 (⌂|1)
        └── ►:1[1]:anon:
            └── ·d20f547 (⌂|1)
                ├── ►:2[2]:A
                │   ├── ·198d2e4 (⌂|1)
                │   ├── ·7325853 (⌂|1)
                │   └── ·add59d2 (⌂|1)
                │       └── ►:4[3]:main
                │           └── 🏁·8f0d338 (⌂|1) ►tags/base
                └── ►:3[2]:B
                    └── ·984fd1c (⌂|1)
                        └── →:4: (main)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

└── 👉►:0[0]:with-inner-merge[🌳]
    └── ·a6775ea (⌂|1)
        └── ►:1[1]:anon:
            └── ·b85214b (⌂|1)
                ├── ►:2[2]:A
                │   └── ·add59d2 (⌂|1)
                │       └── ►:4[3]:main
                │           └── 🏁·8f0d338 (⌂|1) ►tags/base
                └── ►:3[2]:B
                    ├── ·f87f875 (⌂|1)
                    ├── ·cb181a0 (⌂|1)
                    └── ·984fd1c (⌂|1)
                        └── →:4: (main)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph = Graph::from_head(&repo, &*meta, target_meta(), standard_options())?.validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
│   └── ·74bcc92 (⌂|🏘|01)
│       ├── 📙►:3[1]:stack-1
│       │   ├── ·2169646 (⌂|🏘|01)
│       │   └── ·46ef828 (⌂|🏘|01)
│       │       └── ►:4[2]:anon:
│       │           ├── ·f555940 (⌂|🏘|✓|11)
│       │           ├── ·d664be0 (⌂|🏘|✓|11)
│       │           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
│       └── 📙►:5[1]:stack-2
│           └── →:4:
└── ►:1[0]:origin/main →:2:
    └── ►:2[1]:main <> origin/main →:1:
        └── ·a0f2ac5 (⌂|✓|10)
            └── →:4:

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph = Graph::from_head(&repo, &*meta, target_meta(), standard_options())?.validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
│   └── ·a26ae77 (⌂|🏘|01)
│       ├── 📙►:4[1]:stack-1
│       │   └── ►:3[2]:anon:
│       │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
│       ├── 📙►:5[1]:stack-2
│       │   └── →:3:
│       └── 📙►:6[1]:stack-3
│           └── →:3:
└── ►:1[0]:origin/main →:2:
    └── ►:2[1]:main <> origin/main →:1:
        └── ·1cf9cf4 (⌂|✓|10)
            └── →:3:

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    └── ·d70d863 (⌂|1)
        ├── ►:1[1]:anon:
        │   └── 🏁·35b8235 (⌂|1)
        └── →:1:

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

    {
        let graph =
            Graph::from_head(&repo, &*meta, target_meta(), standard_options())?.validated()?;
        let mut ws = graph.into_workspace()?;
        let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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
        let graph =
            Graph::from_head(&repo, &*meta, target_meta(), standard_options())?.validated()?;
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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

└── 👉►:0[0]:first-parent[🌳]
    └── ·738ea18 (⌂|1)
        └── ►:1[1]:anon:
            └── ·408ca26 (⌂|1)
                ├── ►:3[2]:anon:
                │   └── ·2854fa2 (⌂|1)
                │       └── ►:4[3]:main
                │           └── 🏁·793a434 (⌂|1) ►tags/base
                └── ►:2[2]:second-parent
                    ├── ·75369b0 (⌂|1)
                    ├── ·553bbf7 (⌂|1)
                    └── ·72614bb (⌂|1)
                        └── →:4: (main)

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

    let graph = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(
                repo.rev_parse_single("refs/heads/implicit-mut")?.detach(),
                Some("refs/heads/implicit-mut".try_into()?),
            ),
            Tip::reachable(
                repo.rev_parse_single("refs/heads/explicit-const")?.detach(),
                Some("refs/heads/explicit-const".try_into()?),
            ),
            Tip::reachable(
                repo.rev_parse_single("refs/heads/explicit-const-2")?
                    .detach(),
                Some("refs/heads/explicit-const-2".try_into()?),
            ),
        ],
        &*meta,
        Default::default(),
        standard_options(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"

├── ►:0[0]:explicit-const
│   └── ·be4ae80 (⌂) ►main
│       └── ►:3[1]:implicit-const
│           └── ·120e3a9 (⌂)
│               └── ►:6[2]:explicit-mut
│                   └── ·a96434e (⌂)
│                       └── ►:5[3]:foo
│                           ├── ·d591dfe (⌂|1)
│                           └── 🏁·35b8235 (⌂|1)
└── ►:1[0]:explicit-const-2
    └── ·d9fa122 (⌂)
        └── ►:4[1]:implicit-const-2
            └── ·85bccf0 (⌂)
                └── 👉►:2[2]:implicit-mut
                    └── ·c8dd361 (⌂|1)
                        └── →:5: (foo)

"#]]
    );

    let mut ws = graph.into_workspace()?;
    let opts = GraphEditorOptions {
        extra_mutable_refs: vec!["refs/heads/explicit-mut".try_into()?],
        ..Default::default()
    };
    let editor = Editor::create_with_opts(&mut ws, &mut *meta, &repo, &opts)?;

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
