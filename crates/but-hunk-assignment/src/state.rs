/// The name of the file holding our state, useful for watching for changes.
const FILE_NAME: &str = "hunk-assignment.toml";

use std::path::{Path, PathBuf};

use anyhow::Result;
use gitbutler_fs::read_toml_file_or_default;
use serde::{Deserialize, Serialize};

use crate::HunkAssignment;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HunkAssignments {
    pub assignments: Vec<HunkAssignment>,
}

pub(crate) struct AssignmentsHandle {
    /// The path to the file containing the hunk assignment state.
    file_path: PathBuf,
}

impl AssignmentsHandle {
    pub fn new(base_path: &Path) -> Self {
        let file_path = base_path.join(FILE_NAME);
        Self { file_path }
    }

    pub fn assignments(&self) -> Result<Vec<HunkAssignment>> {
        let val = self.read_file()?;
        Ok(val.assignments)
    }

    pub fn set_assignments(&self, assignments: Vec<HunkAssignment>) -> Result<()> {
        let mut val = self.read_file()?;
        val.assignments = assignments;
        self.write_file(val)
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    fn read_file(&self) -> Result<HunkAssignments> {
        read_toml_file_or_default(&self.file_path)
    }

    fn write_file(&self, assignments: HunkAssignments) -> Result<()> {
        gitbutler_fs::write(&self.file_path, toml::to_string(&assignments)?)
    }
}
