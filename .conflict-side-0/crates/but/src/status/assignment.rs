use bstr::BString;
use but_hunk_assignment::HunkAssignment;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct FileAssignment {
    pub path: BString,
    pub assignments: Vec<HunkAssignment>,
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
            assignments: filtered_assignments,
        }
    }
    #[expect(dead_code)]
    pub fn hash(&self) -> String {
        let combined_ids: String = self
            .assignments
            .iter()
            .map(|a| a.id.unwrap_or_default().to_string())
            .collect::<Vec<_>>()
            .join("-");
        crate::id::hash(&format!("{},{}", &self.path.to_string(), &combined_ids))
    }
}

pub(crate) fn filter_by_stack_id<'a, I>(
    input: I,
    stack_id: &Option<but_workspace::StackId>,
) -> Vec<FileAssignment>
where
    I: IntoIterator<Item = &'a FileAssignment>,
{
    let mut out = Vec::new();
    for assignment in input {
        let filtered = assignment
            .assignments
            .iter()
            .filter(|a| a.stack_id == *stack_id)
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
