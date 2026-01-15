#![allow(missing_docs)]
use but_serde::BStringForFrontend;
use gix::ObjectId;
use serde::Serialize;

use crate::commit_engine::RejectionReason;

/// The JSON serializable type of [super::CreateCommitOutcome].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommitOutcome {
    /// Paths that contained at least one rejected hunk, for instance, a change that didn't apply, along with the reason for the rejection.
    pub paths_to_rejected_changes: Vec<(RejectionReason, BStringForFrontend)>,
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
            paths_to_rejected_changes: rejected_specs
                .into_iter()
                .map(|(reason, spec)| (reason, spec.path.into()))
                .collect(),
            new_commit,
            commit_mapping,
        }
    }
}
