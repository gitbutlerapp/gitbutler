/// These tests cover the signing behavior on the Step::Pick.
use anyhow::Result;
use but_core::commit::SignCommit;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, GraphEditorOptions, Pick, Step, cherry_pick::PickMode};
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Modify the "c" commit to no longer be signed
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let mut pick = Pick::new_pick(c.detach());
    pick.sign_commit = SignCommit::No;
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
                                └── 🏁·b6e2f57 (⌂|1)
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
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
        ├── ·06106c2 (⌂|1) ►c
        └── ·3bfeb52 (⌂|1) ►a, ►b
            └── ►:1[1]:base
                └── 🏁·b6e2f57 (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 06106c2 (HEAD -> main, c) c
    * 3bfeb52 (b, a) a
    * b6e2f57 (base) base
    ");

    insta::assert_snapshot!(cat_commit(&repo, "c")?, @"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent 3bfeb524461f65f82bf5027fc895fe9fd5f36203
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    change-id npznqkxwqsymyowwmpltqqvnvuqqrsoy
    gpgsig -----BEGIN SSH SIGNATURE-----
     U1NIU0lHAAAAAQAAARcAAAAHc3NoLXJzYQAAAAMBAAEAAAEBALgYZ0wtPvJyZ40qWRIe8A
     bAYhKYgt0bWX3Z16PyZjWEF+FFx9bRSThY0Bc45TNzon133/aaTWMBnO9RDPw50wZH2ULI
     xF8Q90BkBq9GI4lcliz8ovpwn3ezN6TQu+Ub1LbTWD2GOaCyUKpuQH96AsmOT5KNASfbdJ
     jf8ezbO+kZg8+J1HMS83gOxhxj15Gwf1cCJAInXr/phYX8BmAZWSHZHu8foy6IG1g1dutr
     2QyAGFddwDKObsrbejsOhwbF7u7PTEGWWO63ZlKS5/QfXg4hCoyWsrTW7lVqI6Xgxk4zOa
     U+EnrNSr2BBXGSSgAqe1vo8TVWggNh/ACdnZa4Y6EAAAADZ2l0AAAAAAAAAAZzaGE1MTIA
     AAEUAAAADHJzYS1zaGEyLTUxMgAAAQBPEv21QjFZJ+/CxMSCs1zb3yxEjqvPo181qaioTw
     BFjDsJgnNLj5H9Uw/uCoTrXkmvOFpdbCJMb0iuf4aiDxqP7Q8wonC66tmdgbkyNQxJyl8T
     CexJ8bhSrTFGu5vX9E2xdcYt5dCpUrD49w3a4hCAcoLAXrNFuGu9LDRRFfh8Bmp6zjXgYC
     XZ0tI4iFDutMulDhmQZicFYPomb0TgHOzpDwr9+zX7pJOhX2xbeM3wbgj0hIfCDb2W81Rn
     A5coj4FSlkXqpYC8mg/jwO54d4cfn2/y2oXesKAY5yxrZPIPlb7vmiLwiEcEh9YQhTT0c0
     3KOol2J6bRKScwko1nMzSz
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Modify the "c" commit to no longer be signed
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let mut pick = Pick::new_pick(c.detach());
    pick.sign_commit = SignCommit::No;
    editor.replace(c_sel, Step::Pick(pick))?;

    // Remove the "b" commit so "c" gets cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·a773b84 (⌂|1) ►c
        └── ·3bfeb52 (⌂|1) ►a, ►b
            └── ►:1[1]:base
                └── 🏁·b6e2f57 (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a773b84 (HEAD -> main, c) c
    * 3bfeb52 (b, a) a
    * b6e2f57 (base) base
    ");

    insta::assert_snapshot!(cat_commit(&repo, "c")?, @"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent 3bfeb524461f65f82bf5027fc895fe9fd5f36203
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    change-id npznqkxwqsymyowwmpltqqvnvuqqrsoy

    c
    ");

    Ok(())
}

/// Picking with [`PickMode::Force`] and [`SignCommit::Yes`] should cause the pick to be
/// cherry-picked and signed even in absence of other changes, regardless of signing config.
#[test]
fn force_picked_commit_with_sign_yes_is_signed_when_otherwise_unchanged() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing(
        "unsigned-commits-with-signing-key-setup-but-signing-disabled",
    )?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * ea8caac (HEAD -> main, top) top
    * 135e6ba (mid) mid
    * 7a5aacf (base) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create_with_opts(
        &mut ws,
        &mut *meta,
        &repo,
        &GraphEditorOptions {
            default_sign_commit: SignCommit::No,
            ..<_>::default()
        },
    )?;

    // Force sign the top commit
    let top_commit_id = repo.rev_parse_single("top")?.detach();
    let top_commit_sel = editor.select_commit(top_commit_id)?;
    let mut pick = Pick::new_pick(top_commit_id);
    pick.pick_mode = PickMode::Force;
    pick.sign_commit = SignCommit::Yes;
    editor.replace(top_commit_sel, Step::Pick(pick))?;

    let outcome = editor.rebase()?;
    let materialize_outcome = outcome.materialize()?;

    let after = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(after, @"
    * a120c22 (HEAD -> main, top) top
    * 135e6ba (mid) mid
    * 7a5aacf (base) base
    ");

    let commit_mappings = materialize_outcome.history.commit_mappings();
    assert_eq!(
        commit_mappings.len(),
        1,
        "expected 1 commit to be cherry-picked"
    );
    let new_commit_id = commit_mappings
        .get(&top_commit_id)
        .expect("the force-signed commit should be in the commit mappings");

    let new_commit = repo.find_commit(*new_commit_id)?;
    assert!(
        new_commit
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_some(),
        "expected the force-signed commit to be signed"
    );

    Ok(())
}

/// Force-picking an ancestor with [`SignCommit::Yes`] should _not_ cause a cascade of signatures
/// on descendants that are picked with [`SignCommit::No`].
#[test]
fn force_picked_ancestor_does_not_sign_descendants_picked_with_sign_commit_no() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing(
        "unsigned-commits-with-signing-key-setup-but-signing-disabled",
    )?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * ea8caac (HEAD -> main, top) top
    * 135e6ba (mid) mid
    * 7a5aacf (base) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create_with_opts(
        &mut ws,
        &mut *meta,
        &repo,
        &GraphEditorOptions {
            default_sign_commit: SignCommit::No,
            ..<_>::default()
        },
    )?;

    let top_commit_id = repo.rev_parse_single("top")?.detach();
    let mid_commit_id = repo.rev_parse_single("mid")?.detach();

    // We pick the mid commit with forced signing. This should cause it to be signed, but its
    // descendant top commit should _not_ get signed as it was picked with SignCommit::No
    let mid_sel = editor.select_commit(mid_commit_id)?;
    let mut pick = Pick::new_pick(mid_commit_id);
    pick.pick_mode = PickMode::Force;
    pick.sign_commit = SignCommit::Yes;
    editor.replace(mid_sel, Step::Pick(pick))?;

    let outcome = editor.rebase()?;
    let materialize_outcome = outcome.materialize()?;

    let after = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(after, @"
    * ddf9e11 (HEAD -> main, top) top
    * 70ea083 (mid) mid
    * 7a5aacf (base) base
    ");

    let commit_mappings = materialize_outcome.history.commit_mappings();
    assert_eq!(
        commit_mappings.len(),
        2,
        "expected 2 commits to be cherry-picked"
    );
    let new_mid_commit_id = commit_mappings
        .get(&mid_commit_id)
        .expect("the force-signed commit should be in the commit mappings");
    let new_top_commit_id = commit_mappings
        .get(&top_commit_id)
        .expect("the head commit should be in the commit mappings");

    let new_top_commit = repo.find_commit(*new_top_commit_id)?;
    let new_mid_commit = repo.find_commit(*new_mid_commit_id)?;
    assert!(
        new_top_commit
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_none(),
        "top commit should not have been cascade-signed"
    );
    assert!(
        new_mid_commit
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_some(),
        "mid commit should have been force-signed"
    );

    Ok(())
}

/// Force-picking an ancestor with [`SignCommit::Yes`] _should_ cause a cascade of signatures
/// when descendants are also picked with [`SignCommit::Yes`].
///
/// This is the primary mechanism by which we can programmatically sign/re-sign a branch
/// independently of Git-compatible configuration.
#[test]
fn force_picked_ancestor_triggers_cascading_signatures_on_descendants_picked_with_sign_commit_yes()
-> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing(
        "unsigned-commits-with-signing-key-setup-but-signing-disabled",
    )?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * ea8caac (HEAD -> main, top) top
    * 135e6ba (mid) mid
    * 7a5aacf (base) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create_with_opts(
        &mut ws,
        &mut *meta,
        &repo,
        &GraphEditorOptions {
            default_sign_commit: SignCommit::Yes,
            ..<_>::default()
        },
    )?;

    let top_commit_id = repo.rev_parse_single("top")?.detach();
    let mid_commit_id = repo.rev_parse_single("mid")?.detach();

    // We pick the mid commit with force. This should cause it to be signed, and its descendant
    // top commit should get signed through the cascading rewrites.
    let mid_sel = editor.select_commit(mid_commit_id)?;
    let mut pick = Pick::new_pick(mid_commit_id);
    pick.pick_mode = PickMode::Force;
    pick.sign_commit = SignCommit::Yes;
    editor.replace(mid_sel, Step::Pick(pick))?;

    let outcome = editor.rebase()?;
    let materialize_outcome = outcome.materialize()?;

    let after = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(after, @"
    * 0ceb853 (HEAD -> main, top) top
    * 70ea083 (mid) mid
    * 7a5aacf (base) base
    ");

    let commit_mappings = materialize_outcome.history.commit_mappings();
    assert_eq!(
        commit_mappings.len(),
        2,
        "expected 2 commits to be cherry-picked"
    );
    let new_mid_commit_id = commit_mappings
        .get(&mid_commit_id)
        .expect("the force-signed commit should be in the commit mappings");
    let new_top_commit_id = commit_mappings
        .get(&top_commit_id)
        .expect("the head commit should be in the commit mappings");

    let new_top_commit = repo.find_commit(*new_top_commit_id)?;
    let new_mid_commit = repo.find_commit(*new_mid_commit_id)?;
    assert!(
        new_mid_commit
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_some(),
        "mid commit should be signed"
    );
    assert!(
        new_top_commit
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_some(),
        "top commit should be signed"
    );

    Ok(())
}

/// A commit picked with [`SignCommit::IfSignCommitsEnabled`] should not be signed when
/// Git-compatible signing is not enabled in the config.
#[test]
fn commit_picked_with_sign_if_enabled_is_not_signed_when_signing_config_is_disabled() -> Result<()>
{
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing(
        "unsigned-commits-with-signing-key-setup-but-signing-disabled",
    )?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * ea8caac (HEAD -> main, top) top
    * 135e6ba (mid) mid
    * 7a5aacf (base) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;

    let mut editor = Editor::create_with_opts(
        &mut ws,
        &mut *meta,
        &repo,
        &GraphEditorOptions {
            default_sign_commit: SignCommit::IfSignCommitsEnabled,
            ..<_>::default()
        },
    )?;

    let top_commit_id = repo.rev_parse_single("top")?.detach();
    let mid_commit_id = repo.rev_parse_single("mid")?.detach();

    // Delete the mid commit so the top commit gets picked. The top commit should _NOT_ get signed
    // as signing config is not enabled, and there is a sign guard in place on the pick.
    let mid_sel = editor.select_commit(mid_commit_id)?;
    editor.replace(mid_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let materialize_outcome = outcome.materialize()?;

    let after = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(after, @"
    * de73d4b (HEAD -> main, top) top
    * 7a5aacf (mid, base) base
    ");

    let commit_mappings = materialize_outcome.history.commit_mappings();
    assert_eq!(
        commit_mappings.len(),
        1,
        "expected 1 commit to be cherry-picked"
    );
    let new_top_commit_id = commit_mappings
        .get(&top_commit_id)
        .expect("the head commit should be in the commit mappings");

    let new_commit = repo.find_commit(*new_top_commit_id)?;
    assert!(
        new_commit
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_none(),
        "the cherry-picked top commit should not be signed due to the sign guard"
    );

    Ok(())
}

/// Test for an edge case where a parent-less commit would not be cherry-picked at all even when
/// picked with [`PickMode::Force`] and [`SignCommit::Yes`].
#[test]
fn parentless_commit_force_picked_with_sign_yes_is_signed() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_with_signing(
        "unsigned-commits-with-signing-key-setup-but-signing-disabled",
    )?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @"
    * ea8caac (HEAD -> main, top) top
    * 135e6ba (mid) mid
    * 7a5aacf (base) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;

    let mut editor = Editor::create_with_opts(
        &mut ws,
        &mut *meta,
        &repo,
        &GraphEditorOptions {
            default_sign_commit: SignCommit::IfSignCommitsEnabled,
            ..<_>::default()
        },
    )?;

    let base_commit_id = repo.rev_parse_single("base")?.detach();

    // We pick the base commit with force, which should cause it to get signed.
    let base_sel = editor.select_commit(base_commit_id)?;
    let mut pick = Pick::new_pick(base_commit_id);
    pick.pick_mode = PickMode::Force;
    pick.sign_commit = SignCommit::Yes;
    editor.replace(base_sel, Step::Pick(pick))?;

    let outcome = editor.rebase()?;
    let materialize_outcome = outcome.materialize()?;

    let commit_mappings = materialize_outcome.history.commit_mappings();
    let new_base_commit_id = commit_mappings
        .get(&base_commit_id)
        .expect("the base commit should be in the commit mappings");

    let new_base_commit = repo.find_commit(*new_base_commit_id)?;
    assert!(
        new_base_commit
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_some(),
        "the cherry-picked base commit should be signed"
    );

    Ok(())
}
