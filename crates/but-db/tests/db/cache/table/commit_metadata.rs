use but_core::ChangeId;
use gix::ObjectId;

use crate::cache::in_memory_project_cache;

#[test]
fn read_empty() -> anyhow::Result<()> {
    let cache = in_memory_project_cache();

    assert!(
        cache
            .commit_metadata()
            .commit_hashes_by_change_id(&change_id(1))?
            .is_empty()
    );
    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(1))?,
        None
    );

    Ok(())
}

#[test]
fn set_and_get_single_pair() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();

    cache
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(1), change_id(1))])?;

    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(1))?,
        Some(change_id(1))
    );
    assert_eq!(
        cache
            .commit_metadata()
            .commit_hashes_by_change_id(&change_id(1))?,
        vec![commit(1)]
    );

    Ok(())
}

#[test]
fn set_many_pairs_in_one_batch() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();

    cache.commit_metadata_mut()?.set_change_ids(vec![
        (commit(2), change_id(1)),
        (commit(1), change_id(1)),
        (commit(3), change_id(2)),
    ])?;

    assert_eq!(
        cache
            .commit_metadata()
            .commit_hashes_by_change_id(&change_id(1))?,
        vec![commit(1), commit(2)]
    );
    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(3))?,
        Some(change_id(2))
    );

    Ok(())
}

#[test]
fn set_replaces_existing_change_id_for_commit() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();

    cache
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(1), change_id(1))])?;
    cache
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(1), change_id(2))])?;

    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(1))?,
        Some(change_id(2))
    );
    assert!(
        cache
            .commit_metadata()
            .commit_hashes_by_change_id(&change_id(1))?
            .is_empty()
    );
    assert_eq!(
        cache
            .commit_metadata()
            .commit_hashes_by_change_id(&change_id(2))?,
        vec![commit(1)]
    );

    Ok(())
}

#[test]
fn delete_commits_removes_metadata_and_change_id_relation() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();

    cache
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(1), change_id(1)), (commit(2), change_id(1))])?;
    cache
        .commit_metadata_mut()?
        .delete_commits(vec![commit(1)])?;

    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(1))?,
        None
    );
    assert_eq!(
        cache
            .commit_metadata()
            .commit_hashes_by_change_id(&change_id(1))?,
        vec![commit(2)]
    );

    Ok(())
}

#[test]
fn transaction_commit_persists() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();

    let mut trans = cache.deferred_transaction()?;
    trans
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(1), change_id(1))])?;
    trans.commit()?;

    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(1))?,
        Some(change_id(1))
    );

    Ok(())
}

#[test]
fn transaction_rollback_discards() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();

    let mut trans = cache.deferred_transaction()?;
    trans
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(1), change_id(1))])?;
    trans.rollback()?;

    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(1))?,
        None
    );

    Ok(())
}

#[test]
fn arbitrary_change_ids_roundtrip_losslessly() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();
    let arbitrary = ChangeId::from_number_for_testing(123456789);

    cache
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(1), arbitrary.clone())])?;

    assert_eq!(
        cache.commit_metadata().change_id_for_commit(commit(1))?,
        Some(arbitrary.clone())
    );

    Ok(())
}

fn change_id(value: u128) -> ChangeId {
    ChangeId::from_number_for_testing(value)
}

fn commit(value: u8) -> ObjectId {
    ObjectId::from_hex(format!("{value:02x}696678319e0fa3a20e54f22d47fc8cf1ceaade").as_bytes())
        .expect("statically valid object id")
}
