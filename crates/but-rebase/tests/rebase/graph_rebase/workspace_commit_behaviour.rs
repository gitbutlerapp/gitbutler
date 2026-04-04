//! These tests cover behaviour specific to the workspace commit

use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, LookupStep, Pick, Step};
use but_testsupport::{cat_commit, graph_tree, visualize_commit_graph_all};

use crate::utils::{fixture_writable, fixture_writable_with_signing, standard_options};

#[test]
fn workspace_remains_unchanged_with_no_operations() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * 8795f47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * dd72792 (main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let id = repo.rev_parse_single("gitbutler/workspace")?;
    let selector = editor.select_commit(id.detach())?;
    let step = editor.lookup_step(selector)?;

    assert_eq!(
        step,
        Step::Pick(Pick::new_workspace_pick(id.detach())),
        "Workspace step should match workspace pick defaults"
    );

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:gitbutler/workspace[🌳]
        ├── ·8600a31 (⌂|1)
        └── ·2b9cba3 (⌂|1) ►c, ►main
            └── ►:1[1]:b
                └── ·8df3400 (⌂|1)
                    └── ►:2[2]:a
                        └── ·5b128a2 (⌂|1)
                            └── ►:3[3]:base
                                └── ·3b506ba (⌂|1)
    ");

    let step = outcome.lookup_step(selector)?;
    assert_eq!(
        step,
        Step::Pick(Pick::new_workspace_pick(id.detach())),
        "Workspace step should match workspace pick defaults after first rebase"
    );

    let mat_outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&mat_outcome.workspace.graph).to_string());

    let step = mat_outcome.lookup_step(selector)?;
    assert_eq!(
        step,
        Step::Pick(Pick::new_workspace_pick(id.detach())),
        "Workspace step should match workspace pick defaults after materialization"
    );

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn workspace_commit_is_not_signed_after_cherry_pick() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * 8795f47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * dd72792 (main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Remove the "b" commit so "c" and the workspace commit get cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:gitbutler/workspace[🌳]
        ├── ·04c2142 (⌂|1)
        ├── ·f5d7b3a (⌂|1) ►c, ►main
        └── ·5b128a2 (⌂|1) ►a, ►b
            └── ►:1[1]:base
                └── ·3b506ba (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 31c75e2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * de980c3 (main, c) c
    * 3bfeb52 (b, a) a
    * b6e2f57 (base) base
    ");

    insta::assert_snapshot!(cat_commit(&repo, "gitbutler/workspace")?, @"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent de980c3adf6a0fd63e4b0662297c16d0c9e7177c
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    change-id 1

    GitButler Workspace Commit
    ");

    // We expect "c" to remain signed
    insta::assert_snapshot!(cat_commit(&repo, "c")?, @"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent 3bfeb524461f65f82bf5027fc895fe9fd5f36203
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    change-id 1
    gpgsig -----BEGIN SSH SIGNATURE-----
     U1NIU0lHAAAAAQAAARcAAAAHc3NoLXJzYQAAAAMBAAEAAAEBALgYZ0wtPvJyZ40qWRIe8A
     bAYhKYgt0bWX3Z16PyZjWEF+FFx9bRSThY0Bc45TNzon133/aaTWMBnO9RDPw50wZH2ULI
     xF8Q90BkBq9GI4lcliz8ovpwn3ezN6TQu+Ub1LbTWD2GOaCyUKpuQH96AsmOT5KNASfbdJ
     jf8ezbO+kZg8+J1HMS83gOxhxj15Gwf1cCJAInXr/phYX8BmAZWSHZHu8foy6IG1g1dutr
     2QyAGFddwDKObsrbejsOhwbF7u7PTEGWWO63ZlKS5/QfXg4hCoyWsrTW7lVqI6Xgxk4zOa
     U+EnrNSr2BBXGSSgAqe1vo8TVWggNh/ACdnZa4Y6EAAAADZ2l0AAAAAAAAAAZzaGE1MTIA
     AAEUAAAADHJzYS1zaGEyLTUxMgAAAQCzgTRGROlhLbgBHE+/7Kp1Iy5zhO3KCQUqL1mxoN
     MIP2YYq26jA7Xqxd5ZXBmQ/GjuPUb9SRiYt3gGQ24XuE3IPfMk4KEgR+ko/NyDWAx1M/kk
     J4Kc6h7JoxNFDQFDY1Lj8BXNJ/DemHEHd6ncjBjdZlSlDpeB+x4Lv1fSnRF3RKhzXTA+sZ
     aHOH9hZhWAftrV1IyG4JOfNeMaaHXt8HEuEPNUvCEajqqFCaQK9jBf3hd7biPUd/fQ2XUm
     UfWrxBKP4ZKbO+/JQLmtJfIsxev6no7pF2nxnbmX+ivzE8n/TZJR3xuzBtXNsc1zBdkApM
     LXBDTIkoN64ekxY0tJjYsE
     -----END SSH SIGNATURE-----

    c
    ");

    Ok(())
}

#[test]
fn ad_hoc_workspace_keeps_regular_defaults() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let id = repo.rev_parse_single("HEAD")?;
    let selector = editor.select_commit(id.detach())?;
    let step = editor.lookup_step(selector)?;

    assert_eq!(
        step,
        Step::Pick(Pick::new_pick(id.detach())),
        "Step should match regular pick defaults"
    );

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·120e3a9 (⌂|1)
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1)
        └── ·35b8235 (⌂|1)
    ");

    let step = outcome.lookup_step(selector)?;
    assert_eq!(
        step,
        Step::Pick(Pick::new_pick(id.detach())),
        "Step should match regular pick defaults after rebase"
    );

    let mat_outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&mat_outcome.workspace.graph).to_string());

    let step = mat_outcome.lookup_step(selector)?;
    assert_eq!(
        step,
        Step::Pick(Pick::new_pick(id.detach())),
        "Step should match regular pick defaults after materialization"
    );

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn workspace_commit_should_not_be_allowed_to_conflict() -> Result<()> {
    let (repo, _tmpdir, mut meta) =
        fixture_writable_with_signing("workspace-with-wc-content-signed")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 01bb7bd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * dd72792 (main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Dropping c will cause the workspace commit to conflict because the WC
    // depends on a file created in c
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.replace(c_sel, Step::None)?;

    // We should see an error given saying the workspace commit ended up being
    // conflicted
    insta::assert_debug_snapshot!(editor.rebase(), @r#"
    Err(
        "Commit 01bb7bd5af4d6d3cf2e131f7ffb82431b84083e0 was marked as not conflictable, but resulted in a conflicted state",
    )
    "#);

    Ok(())
}

#[test]
fn workspace_commit_should_not_be_allowed_to_have_non_reference_parents() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * 8795f47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * dd72792 (main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Replace both 'main' and 'c' references with Step::None. The commit 'c'
    // has two references pointing to it, so we need to remove both for the
    // workspace commit's parent path to traverse through None and hit
    // Pick(c), violating the parents_must_be_references constraint.
    let main_ref = editor.select_reference("refs/heads/main".try_into()?)?;
    editor.replace(main_ref, Step::None)?;
    let c_ref = editor.select_reference("refs/heads/c".try_into()?)?;
    editor.replace(c_ref, Step::None)?;

    // We should see an error saying the workspace commit has parents that are
    // not references
    insta::assert_debug_snapshot!(editor.rebase(), @r#"
    Err(
        "Commit 8795f479823adfeb8c692cf953ded9a57c17530c has parents that are not referenced",
    )
    "#);

    Ok(())
}
