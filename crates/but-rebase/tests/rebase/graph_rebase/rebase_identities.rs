/// These tests demonstrate that if none of the steps are changed, the same
/// graphs are returned.
use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_tree, graph_workspace, visualize_commit_graph_all};

use crate::utils::{fixture_writable, standard_options};

#[test]
fn four_commits() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut ws = graph.clone().into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·120e3a9 (⌂|1)
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1)
        └── ·35b8235 (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"{}");

    Ok(())
}

#[test]
fn four_commits_with_short_traversal() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let options = standard_options().with_hard_limit(4);
    let graph = Graph::from_head(&repo, &*meta, options)?.validated()?;
    let mut ws = graph.clone().into_workspace()?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            ├── ·120e3a9
            └── ❌·a96434e
    ");

    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·120e3a9 (⌂|1)
        └── ❌·a96434e (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"{}");

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut ws = graph.clone().into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:with-inner-merge[🌳]
        └── ·e8ee978 (⌂|1)
            └── ►:1[1]:anon:
                └── ·2fc288c (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·add59d2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── ·8f0d338 (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·984fd1c (⌂|1)
                            └── →:4: (main)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"{}");

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("three-branches-merged")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
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
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut ws = graph.clone().into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·1348870 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·add59d2 (⌂|1)
            │       └── ►:4[2]:anon:
            │           └── ·8f0d338 (⌂|1) ►tags/base
            ├── ►:2[1]:B
            │   ├── ·a748762 (⌂|1)
            │   └── ·62e05ba (⌂|1)
            │       └── →:4:
            └── ►:3[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:4:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"{}");

    Ok(())
}
