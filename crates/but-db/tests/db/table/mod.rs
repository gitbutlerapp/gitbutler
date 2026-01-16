use but_db::DbHandle;

mod ci_check;
mod file_write_lock;
mod forge_review;
mod gerrit_metadata;

/// Return a valid DB handle with all migations applied, ready for use, and *in-memory* only.
fn in_memory_db() -> DbHandle {
    DbHandle::new_at_url(":memory:").expect("in-memory always works")
}
