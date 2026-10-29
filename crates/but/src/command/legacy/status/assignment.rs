use std::collections::BTreeMap;

use bstr::BString;

use crate::{IdMap, id::WorktreeHunk};

#[derive(Debug, Clone)]
pub struct CliHunkAssignment {
    #[expect(dead_code)]
    pub inner: WorktreeHunk,
    /// The CLI ID representation of this assignment
    pub cli_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct FileAssignment {
    pub path: BString,
    pub assignments: Vec<CliHunkAssignment>,
}

impl FileAssignment {
    pub fn get_assignments_by_file(id_map: &IdMap) -> BTreeMap<BString, Self> {
        let mut assignments_by_file: BTreeMap<BString, FileAssignment> = BTreeMap::new();
        for uncommitted_file in id_map.uncommitted_files.values() {
            let path = uncommitted_file.path();
            let assignments = if let Some(file_assignment) = assignments_by_file.get_mut(path) {
                &mut file_assignment.assignments
            } else {
                &mut assignments_by_file
                    .entry(path.to_owned())
                    .or_insert(FileAssignment {
                        path: path.to_owned(),
                        assignments: Vec::new(),
                    })
                    .assignments
            };
            for hunk_assignment in uncommitted_file.hunk_assignments() {
                assignments.push(CliHunkAssignment {
                    inner: hunk_assignment.1.clone(),
                    cli_id: uncommitted_file.short_id.clone(),
                });
            }
        }
        assignments_by_file
    }
}
