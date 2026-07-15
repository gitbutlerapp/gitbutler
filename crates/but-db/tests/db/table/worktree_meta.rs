use but_db::WorktreeMeta;

use crate::table::in_memory_db;

#[test]
fn get_nonexistent() -> anyhow::Result<()> {
    let db = in_memory_db();

    let result = db.worktree_meta().get(b"missing")?;
    assert!(result.is_none());
    assert_eq!(db.worktree_meta().list()?, vec![]);

    Ok(())
}

#[test]
fn upsert_and_get() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = WorktreeMeta {
        name: b"wt-one".to_vec(),
        archived: true,
    };
    db.worktree_meta_mut().upsert(meta.clone())?;

    let retrieved = db.worktree_meta().get(&meta.name)?;
    assert_eq!(retrieved, Some(meta.clone()));

    // Upsert replaces the existing row.
    let unarchived = WorktreeMeta {
        archived: false,
        ..meta.clone()
    };
    db.worktree_meta_mut().upsert(unarchived.clone())?;
    assert_eq!(db.worktree_meta().get(&meta.name)?, Some(unarchived));

    Ok(())
}

#[test]
fn list_is_sorted_by_name() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    for (name, archived) in [(&b"b"[..], false), (b"a", true), (b"c", false)] {
        db.worktree_meta_mut().upsert(WorktreeMeta {
            name: name.to_vec(),
            archived,
        })?;
    }

    let names: Vec<_> = db
        .worktree_meta()
        .list()?
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert_eq!(names, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);

    Ok(())
}

#[test]
fn non_utf8_names_roundtrip() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = WorktreeMeta {
        name: vec![0xff, 0xfe, b'w', b't'],
        archived: false,
    };
    db.worktree_meta_mut().upsert(meta.clone())?;

    assert_eq!(db.worktree_meta().get(&meta.name)?, Some(meta));

    Ok(())
}

#[test]
fn delete() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = WorktreeMeta {
        name: b"wt-one".to_vec(),
        archived: false,
    };
    db.worktree_meta_mut().upsert(meta.clone())?;
    db.worktree_meta_mut().delete(&meta.name)?;

    assert_eq!(db.worktree_meta().get(&meta.name)?, None);

    Ok(())
}

#[test]
fn with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = WorktreeMeta {
        name: b"wt-one".to_vec(),
        archived: true,
    };

    let mut trans = db.transaction()?;
    trans.worktree_meta_mut().upsert(meta.clone())?;
    assert_eq!(trans.worktree_meta().get(&meta.name)?, Some(meta.clone()));
    trans.commit()?;

    assert_eq!(db.worktree_meta().get(&meta.name)?, Some(meta));

    Ok(())
}

#[test]
fn transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = WorktreeMeta {
        name: b"wt-one".to_vec(),
        archived: false,
    };
    let mut trans = db.transaction()?;
    trans.worktree_meta_mut().upsert(meta.clone())?;
    trans.rollback()?;

    assert_eq!(db.worktree_meta().get(&meta.name)?, None);

    Ok(())
}
