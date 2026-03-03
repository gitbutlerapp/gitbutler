#![allow(missing_docs)]
use but_serde::BStringForFrontend;
use gix::ObjectId;
use serde::Serialize;

use crate::commit_engine::RejectionReason;

fn required_steps_from_rejections(
    rejected_specs: &[(RejectionReason, but_serde::BStringForFrontend)],
) -> Vec<String> {
    if rejected_specs.is_empty() {
        return Vec::new();
    }

    let mut steps = vec![
        "Review rejected files and lock details listed in this dialog.".to_string(),
        "If a lock references an unknown or missing stack, open Branches and apply/recreate the stack context.".to_string(),
        "Resolve conflicting/dependent changes in affected branches, then retry the commit.".to_string(),
    ];

    if rejected_specs.iter().any(|(reason, _)| {
        matches!(
            reason,
            RejectionReason::WorkspaceMergeConflict
                | RejectionReason::WorkspaceMergeConflictOfUnrelatedFile
                | RejectionReason::CherryPickMergeConflict
        )
    }) {
        steps.push(
            "Conflicts were detected: resolve conflicted commits first before retrying this commit."
                .to_string(),
        );
    }

    if rejected_specs
        .iter()
        .any(|(reason, _)| matches!(reason, RejectionReason::MissingDiffSpecAssociation))
    {
        steps.push(
            "Some selected hunks are stale: re-open the changed file and reselect the desired hunks."
                .to_string(),
        );
    }

    steps
}

/// The JSON serializable type of [super::CreateCommitOutcome].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommitOutcome {
    /// Paths that contained at least one rejected hunk, for instance, a change that didn't apply, along with the reason for the rejection.
    pub paths_to_rejected_changes: Vec<(RejectionReason, BStringForFrontend)>,
    /// Ordered, user-facing remediation steps intended for non-CLI consumers.
    pub required_steps: Vec<String>,
    /// The newly created commit, if there was one. It maybe that a couple of paths were rejected, but the commit was created anyway.
    #[serde(with = "but_serde::object_id_opt")]
    pub new_commit: Option<gix::ObjectId>,
    /// A listing of all commits `(old, new)` with the initial commit hash on the left and the rewritten version of it on the right side of each tuple.
    pub commit_mapping: Vec<(ObjectId, ObjectId)>,
}

impl From<super::CreateCommitOutcome> for CreateCommitOutcome {
    fn from(
        super::CreateCommitOutcome {
            rejected_specs,
            new_commit,
            changed_tree_pre_cherry_pick: _,
            references: _,
            rebase_output,
            index: _,
        }: super::CreateCommitOutcome,
    ) -> Self {
        let paths_to_rejected_changes: Vec<_> = rejected_specs
            .into_iter()
            .map(|(reason, spec)| (reason, spec.path.into()))
            .collect();
        let commit_mapping = if let Some(rebase_output) = rebase_output {
            rebase_output
                .commit_mapping
                .iter()
                .map(|(_, a, b)| (*a, *b))
                .collect()
        } else {
            Vec::new()
        };
        CreateCommitOutcome {
            required_steps: required_steps_from_rejections(&paths_to_rejected_changes),
            paths_to_rejected_changes,
            new_commit,
            commit_mapping,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::required_steps_from_rejections;
    use crate::commit_engine::RejectionReason;

    #[test]
    fn required_steps_are_present_for_rejections() {
        let steps = required_steps_from_rejections(&[(
            RejectionReason::WorkspaceMergeConflictOfUnrelatedFile,
            ".config/dotnet-tools.json".into(),
        )]);
        assert!(
            !steps.is_empty(),
            "required steps must be present for UI consumers when rejections exist"
        );
        assert!(
            steps.iter().any(|s| s.contains("Conflicts were detected")),
            "conflict-specific guidance should be included"
        );
    }
}
