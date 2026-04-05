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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(1)])?,
        vec![(commit(1), None)]
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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(1)])?,
        vec![(commit(1), Some(change_id(1)))]
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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(3)])?,
        vec![(commit(3), Some(change_id(2)))]
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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(1)])?,
        vec![(commit(1), Some(change_id(2)))]
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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(1)])?,
        vec![(commit(1), None)]
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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(1)])?,
        vec![(commit(1), Some(change_id(1)))]
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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(1)])?,
        vec![(commit(1), None)]
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
        cache
            .commit_metadata()
            .change_ids_for_commits([commit(1)])?,
        vec![(commit(1), Some(arbitrary.clone()))]
    );

    Ok(())
}

#[test]
fn multi_lookup_preserves_input_order_duplicates_and_missing_entries() -> anyhow::Result<()> {
    let mut cache = in_memory_project_cache();

    cache
        .commit_metadata_mut()?
        .set_change_ids(vec![(commit(2), change_id(2)), (commit(1), change_id(1))])?;

    assert_eq!(
        cache.commit_metadata().change_ids_for_commits([
            commit(2),
            commit(3),
            commit(1),
            commit(2)
        ])?,
        vec![
            (commit(2), Some(change_id(2))),
            (commit(3), None),
            (commit(1), Some(change_id(1))),
            (commit(2), Some(change_id(2))),
        ]
    );

    Ok(())
}

#[test]
fn multi_lookup_handles_more_entries_than_sqlite_variable_limit_chunk_size() -> anyhow::Result<()> {
    const LOOKUP_SIZE: u16 = 1000;

    let mut cache = in_memory_project_cache();

    let entries = (1..=LOOKUP_SIZE).map(|value| (commit(value), change_id((value - 1).into())));
    cache.commit_metadata_mut()?.set_change_ids(entries)?;

    let lookup = (1..=LOOKUP_SIZE).map(commit);
    let expected: Vec<_> = (1..=LOOKUP_SIZE)
        .map(|value| (commit(value), Some(change_id((value - 1).into()))))
        .collect();

    assert_eq!(
        cache.commit_metadata().change_ids_for_commits(lookup)?,
        expected
    );

    Ok(())
}

fn change_id(value: u128) -> ChangeId {
    ChangeId::from_number_for_testing(value)
}

fn commit(value: u16) -> ObjectId {
    ObjectId::from_hex(format!("{value:04x}6678319e0fa3a20e54f22d47fc8cf1ceaade").as_bytes())
        .expect("statically valid object id")
}
