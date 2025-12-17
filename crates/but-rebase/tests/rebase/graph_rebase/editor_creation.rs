use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, testing::TestingDot as _};
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

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 120e3a90b753a492cef9a552ae3b9ba1f1391362"]
        1 [ label="reference: refs/heads/main"]
        2 [ label="pick: a96434e2505c2ea0896cf4f58fec0778e074d3da"]
        3 [ label="pick: d591dfed1777b8f00f5b7b6f427537eeb5878178"]
        4 [ label="pick: 35b8235197020a417e9405ab5d4db6f204e8d84b"]
        1 -> 0 [ label="order: 0"]
        0 -> 2 [ label="order: 0"]
        2 -> 3 [ label="order: 0"]
        3 -> 4 [ label="order: 0"]
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

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: e8ee978dac10e6a85006543ef08be07c5824b4f7"]
        1 [ label="reference: refs/heads/with-inner-merge"]
        2 [ label="pick: 2fc288c36c8bb710c78203f78ea9883724ce142b"]
        3 [ label="pick: 984fd1c6d3975901147b1f02aae6ef0a16e5904e"]
        4 [ label="reference: refs/heads/B"]
        5 [ label="pick: add59d26b2ffd7468fcb44c2db48111dd8f481e5"]
        6 [ label="reference: refs/heads/A"]
        7 [ label="pick: 8f0d33828e5c859c95fb9e9fc063374fdd482536"]
        8 [ label="reference: refs/tags/base"]
        9 [ label="reference: refs/heads/main"]
        1 -> 0 [ label="order: 0"]
        4 -> 3 [ label="order: 0"]
        6 -> 5 [ label="order: 0"]
        8 -> 7 [ label="order: 0"]
        9 -> 8 [ label="order: 0"]
        0 -> 2 [ label="order: 0"]
        2 -> 6 [ label="order: 0"]
        2 -> 4 [ label="order: 1"]
        3 -> 9 [ label="order: 0"]
        5 -> 9 [ label="order: 0"]
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

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 134887021e06909021776c023a608f8ef179e859"]
        1 [ label="reference: refs/heads/main"]
        2 [ label="pick: 930563a048351f05b14cc7b9c0a48640e5a306b0"]
        3 [ label="reference: refs/heads/C"]
        4 [ label="pick: 68a2fc349e13a186e6d65871a31bad244d25e6f4"]
        5 [ label="pick: 984fd1c6d3975901147b1f02aae6ef0a16e5904e"]
        6 [ label="pick: a7487625f079bedf4d20e48f052312c010117b38"]
        7 [ label="reference: refs/heads/B"]
        8 [ label="pick: 62e05ba0716487f5e494a72952e296eca8c9f276"]
        9 [ label="pick: add59d26b2ffd7468fcb44c2db48111dd8f481e5"]
        10 [ label="reference: refs/heads/A"]
        11 [ label="pick: 8f0d33828e5c859c95fb9e9fc063374fdd482536"]
        12 [ label="reference: refs/tags/base"]
        1 -> 0 [ label="order: 0"]
        3 -> 2 [ label="order: 0"]
        7 -> 6 [ label="order: 0"]
        10 -> 9 [ label="order: 0"]
        12 -> 11 [ label="order: 0"]
        0 -> 10 [ label="order: 0"]
        0 -> 7 [ label="order: 1"]
        0 -> 3 [ label="order: 2"]
        2 -> 4 [ label="order: 0"]
        4 -> 5 [ label="order: 0"]
        5 -> 12 [ label="order: 0"]
        6 -> 8 [ label="order: 0"]
        8 -> 12 [ label="order: 0"]
        9 -> 12 [ label="order: 0"]
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

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 120e3a90b753a492cef9a552ae3b9ba1f1391362"]
        1 [ label="reference: refs/heads/main"]
        2 [ label="pick: a96434e2505c2ea0896cf4f58fec0778e074d3da"]
        3 [ label="pick: d591dfed1777b8f00f5b7b6f427537eeb5878178"]
        4 [ label="reference: refs/heads/Z"]
        5 [ label="reference: refs/heads/Y"]
        6 [ label="reference: refs/heads/X"]
        7 [ label="pick: 35b8235197020a417e9405ab5d4db6f204e8d84b"]
        1 -> 0 [ label="order: 0"]
        4 -> 3 [ label="order: 0"]
        5 -> 4 [ label="order: 0"]
        6 -> 5 [ label="order: 0"]
        0 -> 2 [ label="order: 0"]
        2 -> 6 [ label="order: 0"]
        3 -> 7 [ label="order: 0"]
    }
    "#);

    Ok(())
}

#[test]
fn first_parent_leg_long() -> Result<()> {
    let (repo, meta) = fixture("first-parent-leg-long")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 6ac5745 (HEAD -> with-inner-merge) on top of inner merge
    *   d20f547 Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | 198d2e4 (A) A: 10 more more lines on top
    * | 7325853 A: 10 more lines on top
    * | add59d2 A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉►:0[0]:with-inner-merge[🌳]
        └── ·6ac5745 (⌂|1)
            └── ►:1[1]:anon:
                └── ·d20f547 (⌂|1)
                    ├── ►:2[2]:A
                    │   ├── ·198d2e4 (⌂|1)
                    │   ├── ·7325853 (⌂|1)
                    │   └── ·add59d2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── ·8f0d338 (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·984fd1c (⌂|1)
                            └── →:4: (main)
    ");

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: 6ac5745355fd039cb8eed6cd239401600a8e4f45"]
        1 [ label="reference: refs/heads/with-inner-merge"]
        2 [ label="pick: d20f547c5da0fa3540ab246b1b22025e8911dbfa"]
        3 [ label="pick: 984fd1c6d3975901147b1f02aae6ef0a16e5904e"]
        4 [ label="reference: refs/heads/B"]
        5 [ label="pick: 198d2e43732183d60f22d1392d8b7f694b0bfcd5"]
        6 [ label="reference: refs/heads/A"]
        7 [ label="pick: 73258535ae058c6ba8a01dad12d31df6baa3daf6"]
        8 [ label="pick: add59d26b2ffd7468fcb44c2db48111dd8f481e5"]
        9 [ label="pick: 8f0d33828e5c859c95fb9e9fc063374fdd482536"]
        10 [ label="reference: refs/tags/base"]
        11 [ label="reference: refs/heads/main"]
        1 -> 0 [ label="order: 0"]
        4 -> 3 [ label="order: 0"]
        6 -> 5 [ label="order: 0"]
        10 -> 9 [ label="order: 0"]
        11 -> 10 [ label="order: 0"]
        0 -> 2 [ label="order: 0"]
        2 -> 6 [ label="order: 0"]
        2 -> 4 [ label="order: 1"]
        3 -> 11 [ label="order: 0"]
        5 -> 7 [ label="order: 0"]
        7 -> 8 [ label="order: 0"]
        8 -> 11 [ label="order: 0"]
    }
    "#);

    Ok(())
}

#[test]
fn second_parent_leg_long() -> Result<()> {
    let (repo, meta) = fixture("second-parent-leg-long")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a6775ea (HEAD -> with-inner-merge) on top of inner merge
    *   b85214b Merge branch 'B' into with-inner-merge
    |\  
    | * f87f875 (B) C: 10 more more lines on top
    | * cb181a0 C: 10 more lines on top
    | * 984fd1c C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉►:0[0]:with-inner-merge[🌳]
        └── ·a6775ea (⌂|1)
            └── ►:1[1]:anon:
                └── ·b85214b (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·add59d2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── ·8f0d338 (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        ├── ·f87f875 (⌂|1)
                        ├── ·cb181a0 (⌂|1)
                        └── ·984fd1c (⌂|1)
                            └── →:4: (main)
    ");

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_dot(), @r#"
    digraph {
        0 [ label="pick: a6775eaa730914fa51d480e4d9a216dd0a2765cf"]
        1 [ label="reference: refs/heads/with-inner-merge"]
        2 [ label="pick: b85214b9301ae61f68933e42ea650f4004e69d01"]
        3 [ label="pick: f87f875efb21a3c0cc5496a546a216db9236831a"]
        4 [ label="reference: refs/heads/B"]
        5 [ label="pick: cb181a0e46969f6ed7f9a3b202e01f6f41be341d"]
        6 [ label="pick: 984fd1c6d3975901147b1f02aae6ef0a16e5904e"]
        7 [ label="pick: add59d26b2ffd7468fcb44c2db48111dd8f481e5"]
        8 [ label="reference: refs/heads/A"]
        9 [ label="pick: 8f0d33828e5c859c95fb9e9fc063374fdd482536"]
        10 [ label="reference: refs/tags/base"]
        11 [ label="reference: refs/heads/main"]
        1 -> 0 [ label="order: 0"]
        4 -> 3 [ label="order: 0"]
        8 -> 7 [ label="order: 0"]
        10 -> 9 [ label="order: 0"]
        11 -> 10 [ label="order: 0"]
        0 -> 2 [ label="order: 0"]
        2 -> 8 [ label="order: 0"]
        2 -> 4 [ label="order: 1"]
        3 -> 5 [ label="order: 0"]
        5 -> 6 [ label="order: 0"]
        6 -> 11 [ label="order: 0"]
        7 -> 11 [ label="order: 0"]
    }
    "#);

    Ok(())
}
