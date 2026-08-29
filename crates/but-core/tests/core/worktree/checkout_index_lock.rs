use std::io::{Read as _, Seek as _, Write as _};

use but_core::worktree::safe_checkout_from_head;
use but_testsupport::{CommandExt, git, git_status, open_repo, writable_scenario};
use gix::object::tree::EntryKind;

use crate::worktree::utils::build_commit;

#[test]
fn external_index_lock_prevents_all_checkout_changes() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");
    let mut tree = repo.empty_tree().edit()?;
    tree.upsert(
        "existing.txt",
        EntryKind::Blob,
        repo.write_blob(b"original\n")?,
    )?;
    let initial = repo.new_commit("initial", tree.write()?.detach(), None::<gix::ObjectId>)?;
    safe_checkout_from_head(initial.id, &repo, Default::default())?;

    let next = build_commit(
        &repo,
        |tree| {
            tree.upsert(
                "existing.txt",
                EntryKind::Blob,
                repo.write_blob(b"changed\n")?,
            )?;
            tree.upsert("added.txt", EntryKind::Blob, repo.write_blob(b"added\n")?)?;
            Ok(())
        },
        "change existing and add file",
    )?;

    let existing_path = repo.workdir_path("existing.txt").expect("non-bare repo");
    let added_path = repo.workdir_path("added.txt").expect("non-bare repo");
    let index_path = repo.index_path();
    let index_before = std::fs::read(&index_path)?;
    let head_name_before = repo.head_name()?.expect("symbolic HEAD").to_owned();
    let head_id_before = repo.head_id()?.detach();
    let head_ref_id_before = repo.find_reference(&head_name_before)?.id().detach();

    let mut external_lock = gix::lock::File::acquire_to_update_resource(
        &index_path,
        gix::lock::acquire::Fail::Immediately,
        None,
    )?;
    external_lock.write_all(b"external owner")?;
    let lock_path = external_lock.lock_path().to_owned();

    let err = safe_checkout_from_head(next.id, &repo, Default::default())
        .expect_err("the external index lock must prevent checkout");

    assert_eq!(
        std::fs::read_to_string(&existing_path)?,
        "original\n",
        "checkout must not change existing files before reporting contention"
    );
    assert!(
        !added_path.exists(),
        "checkout must not add files before reporting contention"
    );
    assert_eq!(
        std::fs::read(&index_path)?,
        index_before,
        "checkout must leave the raw index unchanged"
    );
    assert_eq!(
        repo.head_name()?.as_ref(),
        Some(&head_name_before),
        "checkout must leave HEAD attached to the same reference"
    );
    assert_eq!(
        repo.head_id()?.detach(),
        head_id_before,
        "checkout must leave HEAD at the same commit"
    );
    assert_eq!(
        repo.find_reference(&head_name_before)?.id().detach(),
        head_ref_id_before,
        "checkout must leave the HEAD reference unchanged"
    );
    assert!(
        lock_path.exists(),
        "checkout must not remove an external index lock"
    );
    external_lock.rewind()?;
    let mut lock_contents = String::new();
    external_lock.read_to_string(&mut lock_contents)?;
    assert_eq!(
        lock_contents, "external owner",
        "checkout must not overwrite an external index lock"
    );
    match err.downcast_ref::<gix::lock::acquire::Error>() {
        Some(gix::lock::acquire::Error::PermanentlyLocked { resource_path, .. }) => {
            assert_eq!(
                resource_path, &index_path,
                "contention must be reported for this worktree's index path"
            );
        }
        other => panic!("expected gix index-lock contention, got {other:?}: {err:#}"),
    }

    drop(external_lock);
    safe_checkout_from_head(next.id, &repo, Default::default())?;
    assert_eq!(git_status(&repo)?, "", "successful checkout must be clean");
    git(&repo).args(["diff", "--exit-code", "HEAD", "--"]).run();
    Ok(())
}

#[test]
fn linked_worktree_preflight_uses_its_own_index_path() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("single-unsigned");
    let linked_root = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    let linked_path = linked_root.path().join("linked");
    git(&repo)
        .args(["worktree", "add", "--detach"])
        .arg(&linked_path)
        .run();
    let linked_repo = open_repo(&linked_path)?;
    assert_ne!(
        linked_repo.index_path(),
        repo.index_path(),
        "a linked worktree must have its own index"
    );

    let linked_index_path = linked_repo.index_path();
    let external_lock = gix::lock::File::acquire_to_update_resource(
        &linked_index_path,
        gix::lock::acquire::Fail::Immediately,
        None,
    )?;
    let err = safe_checkout_from_head(
        linked_repo.head_id()?.detach(),
        &linked_repo,
        Default::default(),
    )
    .expect_err("the linked worktree's index lock must prevent checkout");

    match err.downcast_ref::<gix::lock::acquire::Error>() {
        Some(gix::lock::acquire::Error::PermanentlyLocked { resource_path, .. }) => {
            assert_eq!(
                resource_path, &linked_index_path,
                "contention must name the linked worktree's index"
            );
        }
        other => panic!("expected gix index-lock contention, got {other:?}: {err:#}"),
    }
    assert!(
        external_lock.lock_path().exists(),
        "checkout must preserve the linked worktree's external lock"
    );
    Ok(())
}
