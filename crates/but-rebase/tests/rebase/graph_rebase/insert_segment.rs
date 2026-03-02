//! These tests exercise the insert segment operation.
use std::collections::HashSet;

use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, Step, mutate};
use but_testsupport::{git_status, visualize_commit_graph_all};
use gix::prelude::ObjectIdExt;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn insert_single_node_segment_above() -> Result<()> {
    let (repo, _tmp, _meta) = fixture_writable("three-branches-merged")?;
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
}
