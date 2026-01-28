/// The name of the file holding our state, useful for watching for changes.
use anyhow::Result;
use but_db::{HunkAssignmentsHandle, HunkAssignmentsHandleMut};

use crate::HunkAssignment;

pub fn assignments(db: HunkAssignmentsHandle) -> Result<Vec<HunkAssignment>> {
    let assignments = db
        .list_all()?
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<HunkAssignment>>>()?;
    Ok(assignments)
}

pub fn set_assignments(
    db: HunkAssignmentsHandleMut,
    assignments: Vec<HunkAssignment>,
) -> Result<()> {
    let assignments: Vec<but_db::HunkAssignment> = assignments
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<but_db::HunkAssignment>>>(
    )?;
    db.set_all(assignments).map_err(Into::into)
}
