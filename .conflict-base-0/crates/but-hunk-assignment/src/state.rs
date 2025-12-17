/// The name of the file holding our state, useful for watching for changes.
use anyhow::Result;
use but_db::DbHandle;

use crate::HunkAssignment;

pub fn assignments(db: &mut DbHandle) -> Result<Vec<HunkAssignment>> {
    let assignments = db
        .hunk_assignments()
        .list_all()?
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<HunkAssignment>>>()?;
    Ok(assignments)
}

pub fn set_assignments(db: &mut DbHandle, assignments: Vec<HunkAssignment>) -> Result<()> {
    let assignments: Vec<but_db::HunkAssignment> = assignments
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<but_db::HunkAssignment>>>(
    )?;
    db.hunk_assignments().set_all(assignments)
}
