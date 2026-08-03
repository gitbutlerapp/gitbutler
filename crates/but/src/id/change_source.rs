//! Which checkout an uncommitted change was read from.

use bstr::{BStr, BString};

/// The checkout that an uncommitted change lives in.
///
/// Uncommitted CLI IDs are minted in one flat namespace across every checkout,
/// formatted exactly alike no matter where they come from, so this is what tells
/// the same path dirty in two different checkouts apart: it is mixed into the
/// hash an uncommitted file's ID is derived from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeSourceId {
    /// The main worktree of the project.
    Head,
    /// A linked worktree, identified by its stable name, i.e. the directory name
    /// under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
    Worktree(BString),
}

impl ChangeSourceId {
    /// The linked worktree name, or `None` for the main worktree.
    pub fn worktree_name(&self) -> Option<&BStr> {
        match self {
            ChangeSourceId::Head => None,
            ChangeSourceId::Worktree(name) => Some(name.as_ref()),
        }
    }

    /// Names the checkout for human-facing output, as the tail of a sentence
    /// like "all hunks in `<path>` in ...".
    pub fn describe(&self) -> String {
        match self {
            ChangeSourceId::Head => "the uncommitted area".into(),
            ChangeSourceId::Worktree(name) => format!("worktree {name}"),
        }
    }
}
