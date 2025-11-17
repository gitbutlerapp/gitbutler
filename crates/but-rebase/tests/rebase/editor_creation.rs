use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::GraphExt;
use but_testsupport::{graph_tree, visualize_commit_graph_all};

use crate::utils::{fixture, standard_options};

#[test]
fn four_commits() -> Result<()> {
    let (repo, meta) = fixture("four-commits")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.create_editor()?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 35b8235197020a417e9405ab5d4db6f204e8d84b"]
        1 [ label="pick: d591dfed1777b8f00f5b7b6f427537eeb5878178"]
        2 [ label="pick: a96434e2505c2ea0896cf4f58fec0778e074d3da"]
        3 [ label="pick: 120e3a90b753a492cef9a552ae3b9ba1f1391362"]
        4 [ label="reference: refs/heads/main"]
        1 -> 0 [ label="order: 0"]
        2 -> 1 [ label="order: 0"]
        3 -> 2 [ label="order: 0"]
        4 -> 3 [ label="order: 0"]
    }
    "#);

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, meta) = fixture("merge-in-the-middle")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.create_editor()?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 8f0d33828e5c859c95fb9e9fc063374fdd482536"]
        1 [ label="reference: refs/tags/base"]
        2 [ label="reference: refs/heads/main"]
        3 [ label="pick: add59d26b2ffd7468fcb44c2db48111dd8f481e5"]
        4 [ label="reference: refs/heads/A"]
        5 [ label="pick: 984fd1c6d3975901147b1f02aae6ef0a16e5904e"]
        6 [ label="reference: refs/heads/B"]
        7 [ label="pick: 2fc288c36c8bb710c78203f78ea9883724ce142b"]
        8 [ label="pick: e8ee978dac10e6a85006543ef08be07c5824b4f7"]
        9 [ label="reference: refs/heads/with-inner-merge"]
        1 -> 0 [ label="order: 0"]
        2 -> 1 [ label="order: 0"]
        3 -> 2 [ label="order: 0"]
        4 -> 3 [ label="order: 0"]
        5 -> 2 [ label="order: 0"]
        6 -> 5 [ label="order: 0"]
        7 -> 4 [ label="order: 0"]
        7 -> 6 [ label="order: 1"]
        8 -> 7 [ label="order: 0"]
        9 -> 8 [ label="order: 0"]
    }
    "#);

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, meta) = fixture("three-branches-merged")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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

    let editor = graph.create_editor()?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 8f0d33828e5c859c95fb9e9fc063374fdd482536"]
        1 [ label="reference: refs/tags/base"]
        2 [ label="pick: add59d26b2ffd7468fcb44c2db48111dd8f481e5"]
        3 [ label="reference: refs/heads/A"]
        4 [ label="pick: 62e05ba0716487f5e494a72952e296eca8c9f276"]
        5 [ label="pick: a7487625f079bedf4d20e48f052312c010117b38"]
        6 [ label="reference: refs/heads/B"]
        7 [ label="pick: 984fd1c6d3975901147b1f02aae6ef0a16e5904e"]
        8 [ label="pick: 68a2fc349e13a186e6d65871a31bad244d25e6f4"]
        9 [ label="pick: 930563a048351f05b14cc7b9c0a48640e5a306b0"]
        10 [ label="reference: refs/heads/C"]
        11 [ label="pick: 134887021e06909021776c023a608f8ef179e859"]
        12 [ label="reference: refs/heads/main"]
        1 -> 0 [ label="order: 0"]
        2 -> 1 [ label="order: 0"]
        3 -> 2 [ label="order: 0"]
        4 -> 1 [ label="order: 0"]
        5 -> 4 [ label="order: 0"]
        6 -> 5 [ label="order: 0"]
        7 -> 1 [ label="order: 0"]
        8 -> 7 [ label="order: 0"]
        9 -> 8 [ label="order: 0"]
        10 -> 9 [ label="order: 0"]
        11 -> 3 [ label="order: 0"]
        11 -> 6 [ label="order: 1"]
        11 -> 10 [ label="order: 2"]
        12 -> 11 [ label="order: 0"]
    }
    "#);

    Ok(())
}

#[test]
fn many_references() -> Result<()> {
    let (repo, meta) = fixture("many-references")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe (Z, Y, X) a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉►:0[0]:main[🌳]
        ├── ·120e3a9 (⌂|1)
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1) ►X, ►Y, ►Z
        └── ·35b8235 (⌂|1)
    ");

    let editor = graph.create_editor()?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 35b8235197020a417e9405ab5d4db6f204e8d84b"]
        1 [ label="pick: d591dfed1777b8f00f5b7b6f427537eeb5878178"]
        2 [ label="reference: refs/heads/Z"]
        3 [ label="reference: refs/heads/Y"]
        4 [ label="reference: refs/heads/X"]
        5 [ label="pick: a96434e2505c2ea0896cf4f58fec0778e074d3da"]
        6 [ label="pick: 120e3a90b753a492cef9a552ae3b9ba1f1391362"]
        7 [ label="reference: refs/heads/main"]
        1 -> 0 [ label="order: 0"]
        2 -> 1 [ label="order: 0"]
        3 -> 2 [ label="order: 0"]
        4 -> 3 [ label="order: 0"]
        5 -> 4 [ label="order: 0"]
        6 -> 5 [ label="order: 0"]
        7 -> 6 [ label="order: 0"]
    }
    "#);

    Ok(())
}
