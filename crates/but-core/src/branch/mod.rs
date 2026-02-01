use std::{collections::BTreeMap, path::PathBuf};

type WorktreePathByRef = BTreeMap<gix::refs::FullName, Vec<PathBuf>>;

mod normalize;
pub use normalize::normalize_short_name;

mod generate;
pub use generate::{canned_refname, find_unique_refname, unique_canned_refname};

/// A way to safely delete branches, which is only the case it's checked out nowhere.
pub mod safe_delete;

/// State for reuse when [safely deleting references](SafeDelete::delete_reference).
#[derive(Debug)]
pub struct SafeDelete {
    /// A mapping of one or more worktree paths that are affected by changes to the keyed reference name.
    worktrees_by_ref: WorktreePathByRef,
}
