//! Arguments for `discard`.

#![deny(missing_docs)]

use crate::args::atoms::CliIdArg;

/// Discard branches, commits, or changes.
///
/// Changes may be selected by branch, commit, committed file, uncommitted file, or uncommitted
/// hunk CLI ID. Use `@` to discard all of the main worktree's uncommitted changes, or omit
/// `<CHANGES>` to discard those of the checkout `but` runs in. A linked worktree's uncommitted
/// changes cannot be discarded yet.
///
/// All provided changes must be the same kind. Committed files must come from the same commit.
///
/// The entire operation is recorded as a single oplog entry, so it can be undone with `but undo`.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// One or more branches, commits, or changes to discard.
    ///
    /// If omitted, all uncommitted changes of the checkout `but` runs in are discarded.
    pub changes: Vec<CliIdArg>,
}
