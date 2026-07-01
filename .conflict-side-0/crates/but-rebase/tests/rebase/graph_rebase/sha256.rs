//! Tests key graph rebase operations against a SHA-256 repository.

use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{Editor, Step, mutate::InsertSide},
};
use but_testsupport::{git_status, graph_tree, visualize_commit_graph_all};

use crate::utils::{fixture_writable, standard_options};

const FIXTURE: &str = "sha256-merge-in-the-middle";

#[test]
fn inserting_a_step_rewrites_sha256_commits() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable(FIXTURE)?;
    insta::assert_debug_snapshot!(repo.object_hash(), @"Sha256");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0f01c77 (HEAD -> with-inner-merge) on top of inner merge
    *   8ab779b Merge branch 'B' into with-inner-merge
    |\  
    | * 8f04e4a (B) C: new file with 10 lines
    * | 2ff29ff (A) A: 10 lines on top
    |/  
    * 8dcf66f (tag: base, main) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let merge_id = editor.repo().rev_parse_single("HEAD~")?.detach();
    let (selector, mut merge_obj) = editor.find_selectable_commit(merge_id)?;
    merge_obj.message = "Commit below the merge commit in SHA-256".into();
    merge_obj.parents = vec![].into();
    let new_commit = editor.new_commit_untracked(merge_obj, DateMode::CommitterKeepAuthorKeep)?;

    editor.insert(selector, Step::new_pick(new_commit), InsertSide::Below)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:with-inner-merge[🌳]
        ├── ·f6ce7ad (⌂|1)
        └── ·174eb34 (⌂|1)
            └── ►:1[1]:anon:
                └── ·3a9910d (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·2ff29ff (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── 🏁·8dcf66f (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·8f04e4a (⌂|1)
                            └── →:4: (main)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f6ce7ad (HEAD -> with-inner-merge) on top of inner merge
    * 174eb34 Merge branch 'B' into with-inner-merge
    *   3a9910d Commit below the merge commit in SHA-256
    |\  
    | * 8f04e4a (B) C: new file with 10 lines
    * | 2ff29ff (A) A: 10 lines on top
    |/  
    * 8dcf66f (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha256(0f01c778cee8081dcfd51d010dfcf6dca150fad6cd58bc269b9aed22587b97fb): Sha256(f6ce7ad57ab7b0c17f7bfc1da6c4edf4aba97cb7f1e67b9d15d92d0f985d6833),
        Sha256(49d2a9677ccb3458bef260578f9e49d55402d9f80e4db5f2aae26e17b6a9c4f7): Sha256(3a9910d979f9822da26e98e36907138b7b92ef1722e7fc45d4e4fd66cfb669d2),
        Sha256(8ab779b6d7ff461b6a09a86724e26c18a4d9d66f733a1cf3c927dc98e6eecbec): Sha256(174eb34a99092bf10f71677a4fc5bba1552cbb452585db415cef91768ba58104),
    }
    ");

    Ok(())
}

#[test]
fn replacing_a_step_rewrites_sha256_descendants() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable(FIXTURE)?;
    insta::assert_debug_snapshot!(repo.object_hash(), @"Sha256");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0f01c77 (HEAD -> with-inner-merge) on top of inner merge
    *   8ab779b Merge branch 'B' into with-inner-merge
    |\  
    | * 8f04e4a (B) C: new file with 10 lines
    * | 2ff29ff (A) A: 10 lines on top
    |/  
    * 8dcf66f (tag: base, main) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = editor.repo().rev_parse_single("A")?.detach();
    let (a_selector, mut a_obj) = editor
        .find_selectable_commit(a)
        .context("failed to select branch A commit")?;
    a_obj.message = "A: SHA-256 reworded".into();
    let a_new = editor.new_commit_untracked(a_obj, DateMode::CommitterKeepAuthorKeep)?;

    editor.replace(a_selector, Step::new_pick(a_new))?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:with-inner-merge[🌳]
        └── ·f6f0125 (⌂|1)
            └── ►:1[1]:anon:
                └── ·fccb3fd (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·b5c16ab (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── 🏁·8dcf66f (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·8f04e4a (⌂|1)
                            └── →:4: (main)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f6f0125 (HEAD -> with-inner-merge) on top of inner merge
    *   fccb3fd Merge branch 'B' into with-inner-merge
    |\  
    | * 8f04e4a (B) C: new file with 10 lines
    * | b5c16ab (A) A: SHA-256 reworded
    |/  
    * 8dcf66f (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha256(0f01c778cee8081dcfd51d010dfcf6dca150fad6cd58bc269b9aed22587b97fb): Sha256(f6f0125b1f12060e754ba973e8e630cdb51622c06fa9ebdc76afa5510817e2d3),
        Sha256(8ab779b6d7ff461b6a09a86724e26c18a4d9d66f733a1cf3c927dc98e6eecbec): Sha256(fccb3fdbafb315097348b7c66fa86a745d923f0f59574cc31650b52f5f1390a2),
    }
    ");

    Ok(())
}

#[test]
fn changing_edges_rewrites_sha256_parentage() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable(FIXTURE)?;
    insta::assert_debug_snapshot!(repo.object_hash(), @"Sha256");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0f01c77 (HEAD -> with-inner-merge) on top of inner merge
    *   8ab779b Merge branch 'B' into with-inner-merge
    |\  
    | * 8f04e4a (B) C: new file with 10 lines
    * | 2ff29ff (A) A: 10 lines on top
    |/  
    * 8dcf66f (tag: base, main) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let inner_merge = editor.repo().rev_parse_single("HEAD~")?.detach();
    let a = editor.repo().rev_parse_single("A")?.detach();
    let b_refname = editor.repo().find_reference("B")?.inner.name;

    let (inner_merge_selector, _) = editor.find_selectable_commit(inner_merge)?;
    let (a_selector, _) = editor.find_selectable_commit(a)?;
    let b_ref_selector = editor.select_reference(b_refname.as_ref())?;
    let (b_selector, _) = editor.find_reference_target(b_ref_selector)?;

    let removed_orders = editor.remove_edges(inner_merge_selector, b_ref_selector)?;
    insta::assert_debug_snapshot!(removed_orders, @"
    [
        1,
    ]
    ");
    editor.add_edge(a_selector, b_selector, 1)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:with-inner-merge[🌳]
        ├── ·636f2bd (⌂|1)
        └── ·93b14a1 (⌂|1)
            └── ►:1[1]:A
                └── ·9d083f9 (⌂|1)
                    ├── ►:2[3]:main
                    │   └── 🏁·8dcf66f (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·8f04e4a (⌂|1)
                            └── →:2: (main)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 636f2bd (HEAD -> with-inner-merge) on top of inner merge
    * 93b14a1 Merge branch 'B' into with-inner-merge
    *   9d083f9 (A) A: 10 lines on top
    |\  
    | * 8f04e4a (B) C: new file with 10 lines
    |/  
    * 8dcf66f (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha256(0f01c778cee8081dcfd51d010dfcf6dca150fad6cd58bc269b9aed22587b97fb): Sha256(636f2bd7679b194672f68390fed8f677231454871ea9d56e315062c62c07698c),
        Sha256(2ff29ffab0c95397a8da853ee4ee8bdad788b23a55a1071edf3f218a13c3e10e): Sha256(9d083f9be0cdd2bfe774f8c05fb9599c4ac4078b68001c79d7a598617f09748b),
        Sha256(8ab779b6d7ff461b6a09a86724e26c18a4d9d66f733a1cf3c927dc98e6eecbec): Sha256(93b14a10bd709ad12d18dffc51d13e46c2fe05f7d45be7769cc04593a689cacd),
    }
    ");

    Ok(())
}
