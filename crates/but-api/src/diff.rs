use but_ctx::Context;
use gix::prelude::ObjectIdExt;
use tracing::instrument;

boolean_enums::gen_boolean_enum!(pub serde ComputeLineStats);

use but_core::diff::CommitDetails;

/// JSON types
// TODO: add schemars
pub mod json {
    use but_core::diff::LineStats;
    use serde::Serialize;

    /// The JSON sibling of [but_core::diff::CommitDetails].
    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CommitDetails {
        /// The commit itself.
        // TODO: make this our own json structure - this one is GUI specific and isn't great
        pub commit: but_workspace::ui::Commit,
        /// The changes
        pub changes: Vec<but_core::ui::TreeChange>,
        /// The stats of the changes.
        // TODO: adapt the frontend to be more specific as well.
        #[serde(rename = "stats")]
        pub line_stats: Option<LineStats>,
        /// Conflicting entries in `commit` as stored in the conflict commit metadata.
        pub conflict_entries: Option<but_core::commit::ConflictEntries>,
    }

    impl From<but_core::diff::CommitDetails> for CommitDetails {
        fn from(
            but_core::diff::CommitDetails {
                commit,
                diff_with_first_parent,
                line_stats,
                conflict_entries,
            }: but_core::diff::CommitDetails,
        ) -> Self {
            CommitDetails {
                commit: commit.into(),
                changes: diff_with_first_parent.into_iter().map(Into::into).collect(),
                line_stats,
                conflict_entries,
            }
        }
    }
}

/// Compute the tree-diff for `commit_id` with its first parent and optionally calculate `line_stats`.
/// It's V2 because it supports the line-stats.
#[but_api_macros::api_cmd_tauri(json::CommitDetails)]
#[instrument(err(Debug))]
pub fn commit_details_v2(
    ctx: &Context,
    commit_id: gix::ObjectId,
    line_stats: ComputeLineStats,
) -> anyhow::Result<CommitDetails> {
    let repo = ctx.repo.get()?;
    CommitDetails::from_commit_id(commit_id.attach(&repo), line_stats.into())
}
