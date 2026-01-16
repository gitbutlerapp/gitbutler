use but_db::DbHandle;

mod ci_check;

/// Return a valid DB handle with all migations applied, ready for use, and *in-memory* only.
fn in_memory_db() -> DbHandle {
    DbHandle::new_at_url(":memory:").expect("in-memory always works")
}
