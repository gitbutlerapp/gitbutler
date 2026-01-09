//! These tests cover behaviour specific to the workspace commit
use std::fs;

use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, LookupStep, Pick, Step};
use but_testsupport::{cat_commit, visualize_commit_graph_all};

use crate::utils::{fixture_writable, fixture_writable_with_signing, standard_options};

#[test]
fn assert_consistent_private_key() -> Result<()> {
    let (_repo, tmpdir, _meta) = fixture_writable_with_signing("workspace-signed")?;

    let key = fs::read_to_string(tmpdir.path().join("signature.key"))?;
    insta::assert_snapshot!(key, @r"
    -----BEGIN OPENSSH PRIVATE KEY-----
    b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAABFwAAAAdzc2gtcn
    NhAAAAAwEAAQAAAQEA6PXEslQskTtIE0vEaO6uiKjR6UxGu/B+UOSD6Mrdsb0U24DH4F2+
    3oRdVQgPH2Q9v6HBPD2jNJdN9hFGUtr51kQgy4QMIo9Lm7gZem0/Ho52qgSYDlZoHY0Uzc
    kr/Gb81btJg/5LSi8J4M2u1aABvDSO8s7d4sISkGVP+wl6p3CRi7hN9+J0F+jw4ckDUGeC
    TC9MIpgb6/2z251k7Sx4Ei4KUqmWBCBuLU60dkH7m6wCzFnzf09Y1lZ63w73xuEh7T+Cin
    8NgI3+iQFzt7bHlWCIs9kuccFseiD8lXl0zE21jxJdhjmV1Q176AstV43QCdUM+8LdPNSj
    hCeUL7WqmQAAA8iSueZdkrnmXQAAAAdzc2gtcnNhAAABAQDo9cSyVCyRO0gTS8Ro7q6IqN
    HpTEa78H5Q5IPoyt2xvRTbgMfgXb7ehF1VCA8fZD2/ocE8PaM0l032EUZS2vnWRCDLhAwi
    j0ubuBl6bT8ejnaqBJgOVmgdjRTNySv8ZvzVu0mD/ktKLwngza7VoAG8NI7yzt3iwhKQZU
    /7CXqncJGLuE334nQX6PDhyQNQZ4JML0wimBvr/bPbnWTtLHgSLgpSqZYEIG4tTrR2Qfub
    rALMWfN/T1jWVnrfDvfG4SHtP4KKfw2Ajf6JAXO3tseVYIiz2S5xwWx6IPyVeXTMTbWPEl
    2GOZXVDXvoCy1XjdAJ1Qz7wt081KOEJ5QvtaqZAAAAAwEAAQAAAQEA3S0p+N2uCp0sCxXu
    fmnOT3VpBoUiyyDD7O1ox8aDwVJx0Q1tt3mJ1B37ttWV9gnoDl725cjngPD+VdeE2vmIJo
    Q8Vr0iAFXoRQn/NpsuSEaeJ0GBVGt5IkVmMRMErfjhp9LPM4Bl3yLV0Be4HJ5zx0pnReRe
    CgKUOX/W9dLEHt8Tb4GuygorFC5GQwoHU0SlswHD8b9jEWNELB/Fy9x5P9IVrK3fK8CufH
    qqd84ssbImDMrixSIhMxFBIqlW/IyhMOa6FvbTwVxl4pOajQNtwHzQB6ye8CU0sNxPpC21
    /Ixfyf+1YXRuiTVCSfZC1O37fo2FgH9yK4qqh8FHKknW2QAAAIEAvxU0LExhDbTamH4u7O
    EIeCOR5S/O2MyI/aDbayxoHTx/AyTX01tGi973T+gSudh1OHUo0i4t2qXH0UtHivQjkwAx
    O4j6sJyGoYu7FC5e6jnAfCoECNRLPxPCCDbyyGh5XekKoGCR4EnmAYDV9eFulwD8sAYhdE
    4HaQysV/NhxaQAAACBAPzsVtQcdp9OCMtY+MK+nbKya4JajYe/vfOyPJLwCo4WX9xxdHrl
    4782TSxxtPJegGDCFvbHRh2sY/ACMC3zqdRg2dPASCDTsP5Oob72L76Bg0zJjoSdSJwCjD
    pMWS5gS0eNyZ5K2B1NgwjCYl6xKQFCeoixvZwaiDmzmJckAiVHAAAAgQDry0Jg/+k/N0LS
    ek4l+CTdWiCDLH2YRpZf+uMHrHdleBF3cLU3tsvAztRHQbpZVBVol6RCX7kXZb4oiAXxBs
    1+DtZaWWnCAll+lzPqxBYvbnNWinu+lmthvidXViV5keBDXwBz/B66inApVvq4PRSvIRti
    /wNPSpUQOE43pLMhHwAAABB0ZXN0QGV4YW1wbGUuY29tAQ==
    -----END OPENSSH PRIVATE KEY-----
    ");

    Ok(())
}

#[test]
fn workspace_remains_unchanged_with_no_operations() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * 8600a31 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2b9cba3 (main, c) c
    * 8df3400 (b) b
    * 5b128a2 (a) a
    * 3b506ba (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.to_editor(&repo)?;

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
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * 8600a31 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2b9cba3 (main, c) c
    * 8df3400 (b) b
    * 5b128a2 (a) a
    * 3b506ba (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    // Remove the "b" commit so "c" and the workspace commit get cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 959e2d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e15040c (main, c) c
    * 5b128a2 (b, a) a
    * 3b506ba (base) base
    ");

    insta::assert_snapshot!(cat_commit(&repo, "gitbutler/workspace")?, @r"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent e15040cb8141e591e34472039a6a7f129f0e9003
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000000001

    GitButler Workspace Commit
    ");

    // We expect "c" to remain signed
    insta::assert_snapshot!(cat_commit(&repo, "c")?, @r"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent 5b128a2ec3b714a5f78d9f97ff6f89b416069017
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000000001
    gpgsig -----BEGIN SSH SIGNATURE-----
     U1NIU0lHAAAAAQAAARcAAAAHc3NoLXJzYQAAAAMBAAEAAAEBAOj1xLJULJE7SBNLxGjuro
     io0elMRrvwflDkg+jK3bG9FNuAx+Bdvt6EXVUIDx9kPb+hwTw9ozSXTfYRRlLa+dZEIMuE
     DCKPS5u4GXptPx6OdqoEmA5WaB2NFM3JK/xm/NW7SYP+S0ovCeDNrtWgAbw0jvLO3eLCEp
     BlT/sJeqdwkYu4TffidBfo8OHJA1BngkwvTCKYG+v9s9udZO0seBIuClKplgQgbi1OtHZB
     +5usAsxZ839PWNZWet8O98bhIe0/gop/DYCN/okBc7e2x5VgiLPZLnHBbHog/JV5dMxNtY
     8SXYY5ldUNe+gLLVeN0AnVDPvC3TzUo4QnlC+1qpkAAAADZ2l0AAAAAAAAAAZzaGE1MTIA
     AAEUAAAADHJzYS1zaGEyLTUxMgAAAQBNbYKjqDniGBtM8BYGD4kBX+au4A99bzK9JmKWyA
     IBm18OOtazZu+Pu91opFp8+OMeSY8uTJJrEW+Tu8N7a5JK+CyrmUgZWVOkv0WVkxbRfQfW
     iWz5xfLblK1ZLebO9e5EMN7rBMMugAyu6/lx2XcyUobzwg6hux0nPNW1e1QC19qDR4ZTZV
     V/ynV7EAuLc5B2BKNIK1jZJkWsJl0IA2NyWpp+ELdmvBZ8cb2eAJMi8hq8QuRn/YxB+Jt9
     EFkBS3EoJqdZCfMsyHJE0QVnOAGxXII3XfGd6anBafYCBXt/5/cqtKWbNhfUNcTAw5Lm1J
     kfoK8UfwdQ7EQzuZ2ATjiQ
     -----END SSH SIGNATURE-----

    c
    ");

    Ok(())
}

#[test]
fn ad_hoc_workspace_keeps_regular_defaults() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.to_editor(&repo)?;

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
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("workspace-with-wc-content-signed")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f7f22fe (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 734fb4e (main, c) c
    * 17ac6d7 (b) b
    * d8d3a16 (a) a
    * b1b6109 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // Dropping c will cause the workspace commit to conflict because the WC
    // depends on a file created in c
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.replace(c_sel, Step::None)?;

    // We should see an error given saying the workspace commit ended up being
    // conflicted
    insta::assert_debug_snapshot!(editor.rebase(), @r#"
    Err(
        "Commit f7f22fe8b0257bdb8e8fc9dfdfb1976a56474e06 was marked as not conflictable, but resulted in a conflicted state",
    )
    "#);

    Ok(())
}

#[test]
fn workspace_commit_should_not_be_allowed_to_have_non_reference_parents() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * 8600a31 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2b9cba3 (main, c) c
    * 8df3400 (b) b
    * 5b128a2 (a) a
    * 3b506ba (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

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
        "Commit 8600a31c2ef9503945e3d6e17470445196252611 has parents that are not referenced",
    )
    "#);

    Ok(())
}
