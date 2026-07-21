//! These tests cover behaviour specific to the workspace commit

use anyhow::Result;
use but_graph::edit::Pick;
use but_testsupport::{cat_commit, graph_tree, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::{
    add_stack_with_segments,
    utils::{fixture_writable, fixture_writable_with_signing},
};

#[test]
fn workspace_remains_unchanged_with_no_operations() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
        snapbox::str![[r#"
* 8795f47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* dd72792 (main, c) c
* e5aa7b5 (b) b
* 3bfeb52 (a) a
* b6e2f57 (base) base

"#]]
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let id = repo.rev_parse_single("gitbutler/workspace")?;
    assert_eq!(
        graph.managed_workspace_commit_id(),
        Some(id.detach()),
        "construction records the managed workspace commit without workspace metadata"
    );

    let editor = graph.into_mut(&repo)?;

    let selector = editor.select_commit(id.detach())?;
    let pick = editor.pick_at(selector);

    assert_eq!(
        pick,
        Some(Pick::new_workspace_pick(id.detach())),
        "Workspace pick should match workspace pick defaults"
    );

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  c
│ ◎  gitbutler/workspace[🌳]
│ ●  👉·8795f47 (→)
├─╯
│ ◎  main
├─╯
●  ·dd72792 (→)
◎  b
●  ·e5aa7b5 (→)
◎  a
●  ·3bfeb52 (→)
◎  base
●  🏁·b6e2f57 (→)

"#]]
    );

    let pick = outcome.pick_at(selector);
    assert_eq!(
        pick,
        Some(Pick::new_workspace_pick(id.detach())),
        "Workspace pick should match workspace pick defaults after first rebase"
    );

    let mat_outcome =
        outcome.materialize_changes(&*meta, but_graph::edit::MaterializeOptions::default())?;
    assert_eq!(
        overlayed,
        graph_tree(&mat_outcome.workspace.graph).to_string()
    );

    // `MaterializeOutcome` no longer carries step policy, only the sealed graph:
    // assert the node still addresses the workspace commit after materialization.
    assert_eq!(
        mat_outcome.graph.nodes()[selector]
            .kind()
            .addressable_commit_id(),
        Some(id.detach()),
        "Workspace node should still address the workspace commit after materialization"
    );

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn workspace_commit_is_not_signed_after_cherry_pick() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("workspace-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
        snapbox::str![[r#"
* 8795f47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* dd72792 (main, c) c
* e5aa7b5 (b) b
* 3bfeb52 (a) a
* b6e2f57 (base) base

"#]]
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut editor = graph.into_mut(&repo)?;

    // Remove the "b" commit so "c" and the workspace commit get cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.remove(b_sel)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  c
│ ◎  gitbutler/workspace[🌳]
│ ●  👉·badca2f (→)
├─╯
│ ◎  main
├─╯
●  ·06106c2 (→)
◎  b
◌  no-op
◎  a
●  ·3bfeb52 (→)
◎  base
●  🏁·b6e2f57 (→)

"#]]
    );
    let outcome =
        outcome.materialize_changes(&*meta, but_graph::edit::MaterializeOptions::default())?;
    // The re-traversed post-materialize workspace, unlike the preview
    // (a direct projection of the sealed edit graph), contains no
    // tombstones and re-segments references from disk.
    snapbox::assert_data_eq!(
        graph_tree(&outcome.workspace.graph).to_string(),
        snapbox::str![[r#"
◎  a
│ ◎  b
├─╯
│ ◎  c
│ │ ◎  gitbutler/workspace[🌳]
│ │ ●  👉·badca2f (→)
│ ├─╯
│ │ ◎  main
│ ├─╯
│ ●  ·06106c2 (→)
├─╯
●  ·3bfeb52 (→)
◎  base
●  🏁·b6e2f57 (→)

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* badca2f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 06106c2 (main, c) c
* 3bfeb52 (b, a) a
* b6e2f57 (base) base

"#]]
    );

    snapbox::assert_data_eq!(
        cat_commit(&repo, "gitbutler/workspace")?,
        snapbox::str![[r#"
tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
parent 06106c2b20d31a1454a733b85f8737077cee9b5c
author author <author@example.com> 946684800 +0000
committer Committer (Memory Override) <committer@example.com> 946771200 +0000
gitbutler-headers-version 2
change-id wznplrwlpuqosonpqkwvqtwymskounvy

GitButler Workspace Commit

"#]]
    );

    // We expect "c" to remain signed
    snapbox::assert_data_eq!(
        cat_commit(&repo, "c")?,
        snapbox::str![[r#"
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

"#]]
    );

    Ok(())
}

#[test]
fn ad_hoc_workspace_keeps_regular_defaults() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

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

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    let editor = graph.into_mut(&repo)?;

    let id = repo.rev_parse_single("HEAD")?;
    let selector = editor.select_commit(id.detach())?;
    let pick = editor.pick_at(selector);

    assert_eq!(
        pick,
        Some(Pick::new_pick(id.detach())),
        "Pick should match regular pick defaults"
    );

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  main[🌳]
●  👉·120e3a9 (→)
●  ·a96434e (→)
●  ·d591dfe (→)
●  🏁·35b8235 (→)

"#]]
    );

    let pick = outcome.pick_at(selector);
    assert_eq!(
        pick,
        Some(Pick::new_pick(id.detach())),
        "Pick should match regular pick defaults after rebase"
    );

    let mat_outcome =
        outcome.materialize_changes(&*meta, but_graph::edit::MaterializeOptions::default())?;
    assert_eq!(
        overlayed,
        graph_tree(&mat_outcome.workspace.graph).to_string()
    );

    // `MaterializeOutcome` no longer carries step policy, only the sealed graph:
    // assert the node still addresses the same commit after materialization.
    assert_eq!(
        mat_outcome.graph.nodes()[selector]
            .kind()
            .addressable_commit_id(),
        Some(id.detach()),
        "Node should still address the same commit after materialization"
    );

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn workspace_commit_should_not_be_allowed_to_conflict() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("workspace-with-wc-content-signed")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 01bb7bd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* dd72792 (main, c) c
* e5aa7b5 (b) b
* 3bfeb52 (a) a
* b6e2f57 (base) base

"#]]
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    let mut editor = graph.into_mut(&repo)?;

    // Dropping c will cause the workspace commit to conflict because the WC
    // depends on a file created in c
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.remove(c_sel)?;

    // We should see an error given saying the workspace commit ended up being
    // conflicted
    snapbox::assert_data_eq!(
        editor.rebase().to_debug(),
        snapbox::str![[r#"
Err(
    "Commit 01bb7bd5af4d6d3cf2e131f7ffb82431b84083e0 was marked as not conflictable, but resulted in a conflicted state",
)

"#]]
    );

    Ok(())
}

#[test]
fn workspace_commit_with_deleted_branch_ref_rebases_successfully() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-with-empty-stack")?;

    add_stack_with_segments(
        &mut meta,
        1,
        "stack-1",
        but_testsupport::StackState::InWorkspace,
        &[],
    );
    add_stack_with_segments(
        &mut meta,
        2,
        "stack-2",
        but_testsupport::StackState::InWorkspace,
        &[],
    );

    // Before deletion, both stacks are present.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   74bcc92 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
* | 2169646 (stack-1) Commit D
* | 46ef828 Commit C
|/
| * a0f2ac5 (origin/main, main) Commit X
|/
* f555940 (stack-2) Commit A
* d664be0 Commit B
* fafd9d0 init

"#]]
        .raw()
    );

    // Delete the stack-1 branch ref to simulate a branch whose ref was
    // removed (e.g. force-pushed or deleted) after the workspace commit was
    // created.  The workspace commit still has the old stack-1 tip as a
    // parent, but there is no longer a Reference node for it in the graph.
    repo.find_reference("refs/heads/stack-1")?
        .delete()
        .expect("stack-1 ref should be deletable");

    // Reload so gix sees the ref deletion on disk.
    let repo = gix::open(repo.path())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   74bcc92 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
* | 2169646 Commit D
* | 46ef828 Commit C
|/
| * a0f2ac5 (origin/main, main) Commit X
|/
* f555940 (stack-2) Commit A
* d664be0 Commit B
* fafd9d0 init

"#]]
        .raw()
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    let editor = graph.into_mut(&repo)?;

    // The rebase should succeed even though the workspace commit has a
    // parent that no longer has a corresponding Reference node.
    let outcome = editor.rebase()?;
    let _materialized =
        outcome.materialize_changes(&*meta, but_graph::edit::MaterializeOptions::default())?;

    Ok(())
}
