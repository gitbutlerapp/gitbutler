#![allow(missing_docs)]
use crate::commit_engine::HunkHeader;
use bstr::BString;
use gitbutler_serde::BStringForFrontend;
use serde::{Deserialize, Serialize};

/// The JSON serializable type of [super::DiffSpec].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSpec {
    /// lossless version of `previous_path` if this was a rename.
    pub previous_path_bytes: Option<BString>,
    /// lossless version of `path`.
    pub path_bytes: BString,
    /// The headers of the hunks to use, or empty if all changes are to be used.
    pub hunk_headers: Vec<HunkHeader>,
}

impl From<DiffSpec> for super::DiffSpec {
    fn from(
        DiffSpec {
            path_bytes,
            hunk_headers,
            previous_path_bytes,
        }: DiffSpec,
    ) -> Self {
        super::DiffSpec {
            previous_path: previous_path_bytes,
            path: path_bytes,
            hunk_headers,
        }
    }
}

/// The JSON serializable type of [super::CreateCommitOutcome].
// TODO(ST): this type should contain mappings from old to new commits so that the UI knows what state to update, maybe.
#[derive(Debug, Serialize)]
pub struct CreateCommitOutcome {
    /// Paths that contained at least one rejected hunk, i.e. a change that didn't apply.
    pub paths_to_rejected_changes: Vec<BStringForFrontend>,
    /// The newly created commit.
    // TODO:(ST) this probably rather wants to be some outcome of the rebase engine that contains enough information
    //       to update the UI without popping.
    #[serde(with = "gitbutler_serde::object_id_opt")]
    pub new_commit: Option<gix::ObjectId>,
}

impl From<super::CreateCommitOutcome> for CreateCommitOutcome {
    fn from(
        super::CreateCommitOutcome {
            rejected_specs,
            new_commit,
            ref_edit: _,
        }: super::CreateCommitOutcome,
    ) -> Self {
        CreateCommitOutcome {
            paths_to_rejected_changes: rejected_specs
                .into_iter()
                .map(|spec| spec.path.into())
                .collect(),
            new_commit,
        }
    }
}
