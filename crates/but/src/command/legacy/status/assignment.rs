use bstr::BString;
use but_core::ref_metadata::StackId;
use but_hunk_assignment::HunkAssignment;

use crate::legacy::id::CliId;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CLIHunkAssignment {
    #[serde(flatten)]
    pub inner: HunkAssignment,
    /// The CLI ID representation of this assignment
    pub cli_id: String,
}

impl From<HunkAssignment> for CLIHunkAssignment {
    fn from(inner: HunkAssignment) -> Self {
        let cli_id = CliId::file_from_assignment(&inner).to_string();
        Self { inner, cli_id }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct FileAssignment {
    #[serde(with = "but_serde::bstring_lossy")]
    pub path: BString,
    pub assignments: Vec<CLIHunkAssignment>,
}

impl FileAssignment {
    pub fn from_assignments(path: &BString, assignments: &[HunkAssignment]) -> Self {
        let mut filtered_assignments = Vec::new();
        for assignment in assignments {
            if assignment.path_bytes == *path {
                filtered_assignments.push(assignment.clone());
            }
        }
        Self {
            path: path.clone(),
            assignments: filtered_assignments.into_iter().map(Into::into).collect(),
        }
    }
    #[expect(dead_code)]
    pub fn hash(&self) -> String {
        let combined_ids: String = self
            .assignments
            .iter()
            .map(|a| a.inner.id.unwrap_or_default().to_string())
            .collect::<Vec<_>>()
            .join("-");
        crate::legacy::id::hash(&format!("{},{}", &self.path.to_string(), &combined_ids))
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
