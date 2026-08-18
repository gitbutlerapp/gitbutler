//! The checkout an operation reads its uncommitted changes from: identifying one
//! and listing what each has.
//!
//! CLI IDs for uncommitted changes are minted in one flat namespace across every
//! checkout, so [`ChangeSourceId`] is what keeps them apart. This module owns that
//! concept end to end; `crate::id` only mints the IDs.

use bstr::{BStr, BString};
use but_ctx::Context;

/// The checkout that an uncommitted change lives in.
///
/// Mixed into the hash an uncommitted file's CLI ID is derived from, which is what
/// tells the same path dirty in two different checkouts apart.
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

    /// Names the checkout for human-facing output, as the tail of a sentence like
    /// "all hunks in `<path>` in ...".
    pub fn describe(&self) -> String {
        match self {
            ChangeSourceId::Head => "the uncommitted area".into(),
            ChangeSourceId::Worktree(name) => format!("worktree {name}"),
        }
    }
}

/// The uncommitted changes of one checkout.
#[derive(Debug, Clone)]
pub struct SourceChanges {
    /// The checkout these were read from.
    pub source: ChangeSourceId,
    /// The changed files, which carry the tree status that hunks do not.
    pub changes: Vec<but_core::ui::TreeChange>,
    /// The hunks those changes split into.
    pub hunks: Vec<but_core::SingleHunk>,
}

/// The names of the linked worktrees whose uncommitted changes get CLI IDs.
///
/// Empty unless the `worktreeManipulation` feature flag is on, and empty when the
/// context repository is itself a linked worktree: such a context keeps its own
/// database, so [`Context::worktrees_with_state()`] refuses to read worktree state
/// from it. IDs are built by nearly every command, so that case degrades to the
/// main worktree rather than taking the whole CLI down.
///
/// Must not be called while a database handle is borrowed, see
/// [`Context::worktrees_with_state()`].
pub fn active_worktree_sources(ctx: &Context) -> anyhow::Result<Vec<BString>> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        return Ok(Vec::new());
    }
    let in_linked_worktree = {
        let repo = ctx.repo.get()?;
        repo.git_dir() != repo.common_dir()
    };
    if in_linked_worktree {
        return Ok(Vec::new());
    }
    Ok(ctx
        .worktrees_with_state()?
        .into_iter()
        .filter(|worktree| !worktree.archived)
        .map(|worktree| worktree.name)
        .collect())
}

/// The uncommitted changes of every checkout that gets CLI IDs: the main worktree
/// built from `head_changes` first, then each of `worktree_names`.
///
/// A linked worktree is diffed against its own `HEAD`, as it is not part of the
/// workspace. Its repository shares `repo`'s object database, so it can read
/// everything the editor can.
///
/// Every caller that builds a [`crate::IdMap`] must pass the same set of sources:
/// short IDs are disambiguated against the whole namespace, so a map built from
/// fewer sources can hand out IDs that the next command cannot resolve.
pub fn changes_by_source(
    repo: &gix::Repository,
    context_lines: u32,
    worktree_names: Vec<BString>,
    head_changes: Vec<but_core::ui::TreeChange>,
) -> anyhow::Result<Vec<SourceChanges>> {
    let mut out = Vec::with_capacity(worktree_names.len() + 1);
    let head_hunks = but_core::hunks_from_changes(repo, head_changes.clone(), context_lines);
    out.push(SourceChanges {
        source: ChangeSourceId::Head,
        changes: head_changes,
        hunks: head_hunks,
    });
    for name in worktree_names {
        let wt_repo = but_workspace::worktrees::open_worktree_repo(repo, name.as_ref())?;
        let changes = but_core::diff::ui::worktree_changes(&wt_repo)?.changes;
        let hunks = but_core::hunks_from_changes(&wt_repo, changes.clone(), context_lines);
        out.push(SourceChanges {
            source: ChangeSourceId::Worktree(name),
            changes,
            hunks,
        });
    }
    Ok(out)
}
