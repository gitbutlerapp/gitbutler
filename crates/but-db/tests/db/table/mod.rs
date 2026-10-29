use but_db::DbHandle;

mod branch_order;
mod butler_actions;
mod ci_check;
mod claude;
mod fetch_status;
mod file_write_lock;
mod forge_review;
mod gerrit_metadata;
mod hunk_assignments;
mod virtual_branches;
mod worktree_meta;

/// Return a valid DB handle with all migrations applied, ready for use, and *in-memory* only.
fn in_memory_db() -> DbHandle {
    DbHandle::new_at_path(":memory:").expect("in-memory always works")
}
