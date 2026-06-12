//! These tests exercise the insert segment operation.
use anyhow::{Context, Result};
use bstr::ByteSlice;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, mutate};
use but_testsupport::{git_status, graph_tree, visualize_commit_graph, visualize_commit_graph_all};

use crate::utils::{fixture_writable, standard_options};

fn parent_subjects(repo: &gix::Repository, rev: &str) -> Result<Vec<String>> {
    let commit = repo.rev_parse_single(rev)?.object()?.peel_to_commit()?;
    commit
        .parent_ids()
        .map(|parent_id| {
            let parent = parent_id.object()?.peel_to_commit()?;
            let subject = parent
                .message_raw()?
                .as_bstr()
                .lines()
                .next()
                .unwrap_or_default()
                .to_str_lossy()
                .into_owned();
            Ok(subject)
        })
        .collect()
}
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
        └── ·ee7f107 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·69221b4 (⌂|1)
            │       ├── ►:3[2]:B
            │       │   ├── ·a748762 (⌂|1)
            │       │   └── ·62e05ba (⌂|1)
            │       │       └── ►:4[3]:anon:
            │       │           └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── →:4:
            └── ►:2[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:4:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   ee7f107 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\  
    | * 930563a (C) C: add another 10 lines to new file
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    * | 69221b4 (A) A: 10 lines on top
    |\| 
    * | a748762 (B) B: another 10 lines at the bottom
    * | 62e05ba B: 10 lines at the bottom
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
        └── ·b005f3c (⌂|1)
            ├── ►:1[2]:A
            │   └── ·7f0cc55 (⌂|1)
            │       ├── ►:4[3]:anon:
            │       │   └── ·62e05ba (⌂|1)
            │       │       └── ►:5[4]:anon:
            │       │           └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── →:5:
            ├── ►:2[1]:B
            │   └── ·a3301fe (⌂|1)
            │       └── →:1: (A)
            └── ►:3[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:5:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   b005f3c (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | a3301fe (B) B: another 10 lines at the bottom
    |/ /  
    * | 7f0cc55 (A) A: 10 lines on top
    |\| 
    * | 62e05ba B: 10 lines at the bottom
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
        └── ·61b2679 (⌂|1)
            ├── ►:1[1]:anon:
            │   └── ·758c8a3 (⌂|1) ►A, ►B
            │       └── ►:3[2]:anon:
            │           └── ·db40ffc (⌂|1)
            │               ├── ►:4[3]:anon:
            │               │   └── ·add59d2 (⌂|1)
            │               │       └── ►:5[4]:anon:
            │               │           └── 🏁·8f0d338 (⌂|1) ►tags/base
            │               └── →:5:
            └── ►:2[1]:C
                ├── ·930563a (⌂|1)
                ├── ·68a2fc3 (⌂|1)
                └── ·984fd1c (⌂|1)
                    └── →:5:
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   61b2679 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\  
    | * 930563a (C) C: add another 10 lines to new file
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    * | 758c8a3 (B, A) B: another 10 lines at the bottom
    * | db40ffc B: 10 lines at the bottom
    |\| 
    * | add59d2 A: 10 lines on top
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
        mutate::ParentReparentingOrder::Prepend,
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·cca953f (⌂|1)
            ├── ►:1[2]:A
            │   └── ·69221b4 (⌂|1)
            │       ├── ►:2[3]:B
            │       │   ├── ·a748762 (⌂|1)
            │       │   └── ·62e05ba (⌂|1)
            │       │       └── ►:4[4]:anon:
            │       │           └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── →:4:
            ├── →:2: (B)
            └── ►:3[1]:C
                └── ·76e2160 (⌂|1)
                    ├── ►:5[2]:anon:
                    │   ├── ·68a2fc3 (⌂|1)
                    │   └── ·984fd1c (⌂|1)
                    │       └── →:4:
                    └── →:1: (A)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   cca953f (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | *   76e2160 (C) C: add another 10 lines to new file
    | | |\  
    | |_|/  
    |/| |   
    * | |   69221b4 (A) A: 10 lines on top
    |\ \ \  
    | |/ /  
    |/| |   
    * | | a748762 (B) B: another 10 lines at the bottom
    * | | 62e05ba B: 10 lines at the bottom
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
        mutate::ParentReparentingOrder::Prepend,
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·54f9cab (⌂|1)
            ├── ►:1[1]:A
            │   └── ·9501727 (⌂|1)
            │       ├── ►:4[4]:anon:
            │       │   └── 🏁·8f0d338 (⌂|1) ►tags/base
            │       └── ►:2[2]:B
            │           └── ·347772f (⌂|1)
            │               ├── ►:3[3]:C
            │               │   ├── ·930563a (⌂|1)
            │               │   ├── ·68a2fc3 (⌂|1)
            │               │   └── ·984fd1c (⌂|1)
            │               │       └── →:4:
            │               └── ►:5[3]:anon:
            │                   └── ·62e05ba (⌂|1)
            │                       └── →:4:
            ├── →:2: (B)
            └── →:3: (C)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());
    assert_eq!(
        parent_subjects(&repo, "B")?,
        [
            "C: add another 10 lines to new file",
            "B: 10 lines at the bottom"
        ]
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   54f9cab (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    * | | 9501727 (A) A: 10 lines on top
    |\| | 
    | * |   347772f (B) B: another 10 lines at the bottom
    | |\ \  
    | | |/  
    | |/|   
    | | * 62e05ba B: 10 lines at the bottom
    | |/  
    |/|   
    | * 930563a (C) C: add another 10 lines to new file
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_single_node_segment_below_can_append_reparented_parent() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
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
        mutate::ParentReparentingOrder::Append,
    )?;

    editor.rebase()?.materialize()?;
    assert_eq!(
        parent_subjects(&repo, "B")?,
        [
            "B: 10 lines at the bottom",
            "C: add another 10 lines to new file"
        ]
    );

    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}
