//! These tests cover behaviour specific to the workspace commit
use std::fs;

use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, LookupStep, Pick, Step};
use but_testsupport::{cat_commit, visualize_commit_graph_all};

use crate::utils::{fixture_writable, fixture_writable_with_signing, standard_options};

#[test]
fn assert_consistent_private_key() -> Result<()> {
    let (_repo, tmpdir, _meta) = fixture_writable_with_signing("workspace-signed")?;

    let key = fs::read_to_string(tmpdir.path().join("signature.key"))?;
    insta::assert_snapshot!(key, @"
    -----BEGIN OPENSSH PRIVATE KEY-----
    b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAABFwAAAAdzc2gtcn
    NhAAAAAwEAAQAAAQEAuBhnTC0+8nJnjSpZEh7wBsBiEpiC3RtZfdnXo/JmNYQX4UXH1tFJ
    OFjQFzjlM3OifXff9ppNYwGc71EM/DnTBkfZQsjEXxD3QGQGr0YjiVyWLPyi+nCfd7M3pN
    C75RvUttNYPYY5oLJQqm5Af3oCyY5Pko0BJ9t0mN/x7Ns76RmDz4nUcxLzeA7GHGPXkbB/
    VwIkAidev+mFhfwGYBlZIdke7x+jLogbWDV262vZDIAYV13AMo5uytt6Ow6HBsXu7s9MQZ
    ZY7rdmUpLn9B9eDiEKjJaytNbuVWojpeDGTjM5pT4Ses1KvYEFcZJKACp7W+jxNVaCA2H8
    AJ2dlrhjoQAAA8hDQKQaQ0CkGgAAAAdzc2gtcnNhAAABAQC4GGdMLT7ycmeNKlkSHvAGwG
    ISmILdG1l92dej8mY1hBfhRcfW0Uk4WNAXOOUzc6J9d9/2mk1jAZzvUQz8OdMGR9lCyMRf
    EPdAZAavRiOJXJYs/KL6cJ93szek0LvlG9S201g9hjmgslCqbkB/egLJjk+SjQEn23SY3/
    Hs2zvpGYPPidRzEvN4DsYcY9eRsH9XAiQCJ16/6YWF/AZgGVkh2R7vH6MuiBtYNXbra9kM
    gBhXXcAyjm7K23o7DocGxe7uz0xBlljut2ZSkuf0H14OIQqMlrK01u5VaiOl4MZOMzmlPh
    J6zUq9gQVxkkoAKntb6PE1VoIDYfwAnZ2WuGOhAAAAAwEAAQAAAQBzUx5K00FOoiqKfU/l
    ESpuIFCPs6ivGHX8Z941nyE2PzSyc4NX6C2FNeXN1l+G1tag4NqVYl4+OoF0TgLjctnmYl
    YRBzI1F6y8Uqz5WefjIfQV5IG4f5r2YnfmMLi0MrYTfdwWVqJ9L5dm3MBc2zMpzpO8i8aA
    kHK/XfLw3Pnv8HLgbfmxRDVfMJ46UtsMuTtHcFQdXpQh9JpOlbG+xvCKfCSN+W/SoaSGQo
    1Bt96/MSPPausBnSkcyk4LaeHDO3h2TjVfxCd6fTN0JqgMQ4vvHkiz7UPhx6T0ofkDm+gc
    hbZ8RDOY7msYQcdYziwXRozkWmc/u3fhw37Orji6SzgBAAAAgBurWQGzpqnHSTDbvWOEkF
    LLW3m87GY6MwZFbGnDR2T5sH5nLsVsAgV7D2JwAigM5lGf245E5zyOUSo5QGaVg67mu4Fd
    j05zDi7FESnADqZPCwyH4UrU0jFTTsbgWlo++uEH9ghlYkOodoCBeiG7t7+B1j9dyBWMVJ
    XsV1VmYJSLAAAAgQDc6HENFCofL+9ZI02ATx0z9I4yfEE8f4l4azGVa18ziRFsuH//vzOO
    ZNKUcHmnD5qWSOWzl7UMHfcn2cdv75Oac2CJEAg/lIEtPcTwDngHiESZtqiwOcInwxH1iN
    d4trHNnyvtFoaPWJR0RQ5gkOQrPMd/ZqXpTugkS2pjqNcNwQAAAIEA1Vbra7Tys8xfUZFz
    vZtHxp6cDZ9MV/YH0RLvGqjPueAPerqUgMVnGa/6yRABfPauLhqfqs2q8eMjcfb5hnZ8lB
    YGsxf0dDAMkeeAsKmtMroNGqDHODfnBVyemBH+YuvBR7IS64zOpEGU9DpeDnoqBXOezmkW
    +VXuLOvsScuijeEAAAAQdGVzdEBleGFtcGxlLmNvbQECAw==
    -----END OPENSSH PRIVATE KEY-----
    ");

    Ok(())
}

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

    let step = outcome.lookup_step(selector)?;
    assert_eq!(
        step,
        Step::Pick(Pick::new_workspace_pick(id.detach())),
        "Workspace step should match workspace pick defaults after first rebase"
    );

    let mat_outcome = outcome.materialize()?;

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
    outcome.materialize()?;

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

    let step = outcome.lookup_step(selector)?;
    assert_eq!(
        step,
        Step::Pick(Pick::new_pick(id.detach())),
        "Step should match regular pick defaults after rebase"
    );

    let mat_outcome = outcome.materialize()?;

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
