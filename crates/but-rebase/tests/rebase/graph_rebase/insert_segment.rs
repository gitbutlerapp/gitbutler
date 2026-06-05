//! These tests exercise the insert segment operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, mutate};
use but_testsupport::{git_status, graph_tree, visualize_commit_graph, visualize_commit_graph_all};

use crate::utils::{fixture_writable, standard_options};

#[test]
fn insert_single_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = "refs/heads/B".try_into()?;
    let b_selector = editor
        .select_reference(b)
        .context("Failed to find reference b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment(b_selector, delimiter, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·b78a484 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·706b5e8 (⌂|1)
            │       ├── ►:3[3]:anon:
            │       │   └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── ►:4[2]:B
            │           ├── ·a748762 (⌂|1)
            │           └── ·62e05ba (⌂|1)
            │               └── →:3:
            └── ►:2[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:3:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   b78a484 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\  
    | * 930563a (C) C: add another 10 lines to new file
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    * |   706b5e8 (A) A: 10 lines on top
    |\ \  
    | |/  
    |/|   
    | * a748762 (B) B: another 10 lines at the bottom
    | * 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_single_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment(b_selector, delimiter, mutate::InsertSide::Below)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·32e9d3a (⌂|1)
            ├── ►:1[2]:A
            │   └── ·032ef08 (⌂|1)
            │       ├── ►:4[4]:anon:
            │       │   └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── ►:5[3]:anon:
            │           └── ·62e05ba (⌂|1)
            │               └── →:4:
            ├── ►:2[1]:B
            │   └── ·2d43620 (⌂|1)
            │       └── →:1: (A)
            └── ►:3[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:4:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   32e9d3a (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | 2d43620 (B) B: another 10 lines at the bottom
    |/ /  
    * |   032ef08 (A) A: 10 lines on top
    |\ \  
    | |/  
    |/|   
    | * 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_multi_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_segment(a_selector, delimiter, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·85677e6 (⌂|1)
            ├── ►:1[1]:anon:
            │   └── ·9da738d (⌂|1) ►A, ►B
            │       └── ►:3[2]:anon:
            │           └── ·8e18b4e (⌂|1)
            │               ├── ►:4[4]:anon:
            │               │   └── 🏁·8f0d338 (⌂|1) ►tags/base
            │               └── ►:5[3]:anon:
            │                   └── ·add59d2 (⌂|1)
            │                       └── →:4:
            └── ►:2[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:4:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   85677e6 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\  
    | * 930563a (C) C: add another 10 lines to new file
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    * | 9da738d (B, A) B: another 10 lines at the bottom
    * |   8e18b4e B: 10 lines at the bottom
    |\ \  
    | |/  
    |/|   
    | * add59d2 A: 10 lines on top
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_multi_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_segment(a_selector, delimiter, mutate::InsertSide::Below)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·4db28a9 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·71dfc8f (⌂|1)
            │       └── ►:2[2]:B
            │           ├── ·a748762 (⌂|1)
            │           └── ·62e05ba (⌂|1)
            │               └── ►:4[3]:anon:
            │                   └── 🏁·8f0d338 (⌂|1) ►tags/base
            ├── →:2: (B)
            └── ►:3[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:4:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   4db28a9 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    * | | 71dfc8f (A) A: 10 lines on top
    |/ /  
    * | a748762 (B) B: another 10 lines at the bottom
    * | 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_single_node_segment_above_with_explicit_children() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment_into(
        b_selector,
        delimiter,
        mutate::InsertSide::Above,
        Some(mutate::SomeSelectors::new(vec![c_selector])?),
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·caf3957 (⌂|1)
            ├── ►:1[2]:A
            │   └── ·706b5e8 (⌂|1)
            │       ├── ►:4[4]:anon:
            │       │   └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── ►:2[3]:B
            │           ├── ·a748762 (⌂|1)
            │           └── ·62e05ba (⌂|1)
            │               └── →:4:
            ├── →:2: (B)
            └── ►:3[1]:C
                └── ·23b76e7 (⌂|1)
                    ├── ►:5[2]:anon:
                    │   ├── ·68a2fc3 (⌂|1)
                    │   └── ·984fd1c (⌂|1)
                    │       └── →:4:
                    └── →:1: (A)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   caf3957 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | *   23b76e7 (C) C: add another 10 lines to new file
    | | |\  
    | |_|/  
    |/| |   
    * | | 706b5e8 (A) A: 10 lines on top
    |\| | 
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    |/ /  
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_single_node_segment_below_with_explicit_parents() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_selector,
    };

    editor.insert_segment_into(
        a_selector,
        delimiter,
        mutate::InsertSide::Below,
        Some(mutate::SomeSelectors::new(vec![c_selector])?),
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·c1cb047 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·275b149 (⌂|1)
            │       ├── ►:4[4]:anon:
            │       │   └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── ►:2[2]:B
            │           └── ·9c9c689 (⌂|1)
            │               ├── ►:5[3]:anon:
            │               │   └── ·62e05ba (⌂|1)
            │               │       └── →:4:
            │               └── ►:3[3]:C
            │                   ├── ·930563a (⌂|1)
            │                   ├── ·68a2fc3 (⌂|1)
            │                   └── ·984fd1c (⌂|1)
            │                       └── →:4:
            ├── →:2: (B)
            └── →:3: (C)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   c1cb047 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    * | | 275b149 (A) A: 10 lines on top
    |\| | 
    | * | 9c9c689 (B) B: another 10 lines at the bottom
    | |\| 
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | |/  
    |/|   
    | * 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}
