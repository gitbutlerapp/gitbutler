//! These tests demonstrate that if none of the steps are changed, the same
//! graphs are returned.

use anyhow::Result;
use but_graph::Workspace;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{branch_tree, graph_workspace, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn four_commits() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.clone();
    let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let outcome = editor.rebase()?;
    let overlayed = branch_tree(&outcome.overlayed_workspace()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉:0:►main
    ├── ·120e3a9 (⌂)
    ├── ·a96434e (⌂)
    ├── ·d591dfe (⌂)
    └── 🏁·35b8235 (⌂)

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, branch_tree(outcome.workspace).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn four_commits_with_short_traversal() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    let options = standard_options().with_hard_limit(4);
    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        options,
    )?
    .validated()?;
    let mut ws = graph.clone();

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
        ├── ·120e3a9
        ├── ·a96434e
        ├── ·d591dfe
        └── ·35b8235

"#]]
    );
    let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let outcome = editor.rebase()?;
    let overlayed = branch_tree(&outcome.overlayed_workspace()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉:0:►main
    ├── ·120e3a9 (⌂)
    ├── ·a96434e (⌂)
    ├── ·d591dfe (⌂)
    └── 🏁·35b8235 (⌂)

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, branch_tree(outcome.workspace).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("merge-in-the-middle")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
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

    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.clone();
    let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let outcome = editor.rebase()?;
    let overlayed = branch_tree(&outcome.overlayed_workspace()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉:0:►with-inner-merge
    ├── ·e8ee978 (⌂)
    └── :1:►anon:
        ├── ·2fc288c (⌂)
        ├── :2:►A
        │   ├── ·add59d2 (⌂)
        │   └── :4:►main
        │       └── 🏁·8f0d338 (⌂) ►base
        └── :3:►B
            ├── ·984fd1c (⌂)
            └── →:4:►main

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, branch_tree(outcome.workspace).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("three-branches-merged")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
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

    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.clone();
    let editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let outcome = editor.rebase()?;
    let overlayed = branch_tree(&outcome.overlayed_workspace()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉:0:►main
    ├── ·1348870 (⌂)
    ├── :1:►A
    │   ├── ·add59d2 (⌂)
    │   └── :4:►anon:
    │       └── 🏁·8f0d338 (⌂) ►base
    ├── :2:►B
    │   ├── ·a748762 (⌂)
    │   ├── ·62e05ba (⌂)
    │   └── →:4:►anon:
    └── :3:►C
        ├── ·930563a (⌂)
        ├── ·68a2fc3 (⌂)
        ├── ·984fd1c (⌂)
        └── →:4:►anon:

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, branch_tree(outcome.workspace).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}
