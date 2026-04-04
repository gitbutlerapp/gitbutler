/// These tests cover the `sign_if_configured` property on the Step::Pick.
use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Pick, Step};
use but_testsupport::{cat_commit, graph_tree, visualize_commit_graph_all};

use crate::utils::{fixture_writable_with_signing, standard_options};

#[test]
fn commits_maintain_state_if_not_cherry_picked() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing("four-commits-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * dd72792 (HEAD -> main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Modify the "c" commit to no longer be signed
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let mut pick = Pick::new_pick(c.detach());
    pick.sign_if_configured = false;
    editor.replace(c_sel, Step::Pick(pick))?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        └── ·dd72792 (⌂|1) ►c
            └── ►:1[1]:b
                └── ·e5aa7b5 (⌂|1)
                    └── ►:2[2]:a
                        └── ·3bfeb52 (⌂|1)
                            └── ►:3[3]:base
                                └── ·b6e2f57 (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn commits_are_signed_by_default() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing("four-commits-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * dd72792 (HEAD -> main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Remove the "b" commit so "c" gets cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·de980c3 (⌂|1) ►c
        └── ·3bfeb52 (⌂|1) ►a, ►b
            └── ►:1[1]:base
                └── ·b6e2f57 (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * de980c3 (HEAD -> main, c) c
    * 3bfeb52 (b, a) a
    * b6e2f57 (base) base
    ");

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
fn when_cherry_picking_dont_resign_if_not_set() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing("four-commits-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * dd72792 (HEAD -> main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Modify the "c" commit to no longer be signed
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let mut pick = Pick::new_pick(c.detach());
    pick.sign_if_configured = false;
    editor.replace(c_sel, Step::Pick(pick))?;

    // Remove the "b" commit so "c" gets cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·06fee46 (⌂|1) ►c
        └── ·3bfeb52 (⌂|1) ►a, ►b
            └── ►:1[1]:base
                └── ·b6e2f57 (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 06fee46 (HEAD -> main, c) c
    * 3bfeb52 (b, a) a
    * b6e2f57 (base) base
    ");

    insta::assert_snapshot!(cat_commit(&repo, "c")?, @"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent 3bfeb524461f65f82bf5027fc895fe9fd5f36203
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    change-id 1

    c
    ");

    Ok(())
}
