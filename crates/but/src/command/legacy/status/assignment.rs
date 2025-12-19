use std::collections::BTreeMap;

use bstr::BString;
use but_core::ref_metadata::StackId;
use but_hunk_assignment::HunkAssignment;

use crate::IdMap;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CLIHunkAssignment {
    #[serde(flatten)]
    pub inner: HunkAssignment,
    /// The CLI ID representation of this assignment
    pub cli_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct FileAssignment {
    #[serde(with = "but_serde::bstring_lossy")]
    pub path: BString,
    pub assignments: Vec<CLIHunkAssignment>,
}

impl FileAssignment {
    pub fn get_assignments_by_file(id_map: &IdMap) -> BTreeMap<BString, Self> {
        let mut assignments_by_file: BTreeMap<BString, FileAssignment> = BTreeMap::new();
        for (short_id, uncommitted_file) in &id_map.uncommitted_files {
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
            for hunk_assignment in &uncommitted_file.hunk_assignments {
                assignments.push(CLIHunkAssignment {
                    inner: hunk_assignment.clone(),
                    cli_id: short_id.to_owned(),
                });
            }
        }
        assignments_by_file
    }
}

pub(crate) fn filter_by_stack_id<'a, I>(input: I, stack_id: &Option<StackId>) -> Vec<FileAssignment>
where
    I: IntoIterator<Item = &'a FileAssignment>,
{
    let mut out = Vec::new();
    for assignment in input {
        let filtered = assignment
            .assignments
            .iter()
            .filter(|a| a.inner.stack_id == *stack_id)
            .cloned()
            .collect::<Vec<_>>();
        let mut updated = assignment.clone();
        updated.assignments = filtered;
        if updated.assignments.is_empty() {
            continue;
        }
        out.push(updated);
    }
    out
}
