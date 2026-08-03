//! Resolving the checkout that an operation reads its uncommitted changes from.

use bstr::BString;
use but_ctx::Context;
use but_workspace::commit::ChangeSource;
use nonempty::NonEmpty;

use crate::{CliResult, bad_input, id::ChangeSourceId, id::UncommittedHunkOrFile};

/// The single checkout that all of `changes` come from.
///
/// An operation reads its changes from one repository and cancels them out in
/// that same checkout, so a selection spanning several of them could not be
/// applied in one go. Rejecting it here keeps the error next to the arguments
/// that caused it.
pub fn single_source(changes: &NonEmpty<UncommittedHunkOrFile>) -> CliResult<ChangeSourceId> {
    let source = changes.head.source.clone();
    if let Some(other) = changes.tail.iter().find(|change| change.source != source) {
        return Err(bad_input(format!(
            "Cannot use changes from {} and {} together",
            source.describe(),
            other.source.describe()
        ))
        .hint("An operation can only take changes from one checkout at a time")
        .into());
    }
    Ok(source)
}

/// The checkout that an operation reads its uncommitted changes from, opened for
/// the duration of that operation.
///
/// The repository is owned here rather than borrowed from the [`Context`], so it
/// outlives every [`ChangeSource`] derived from it while the transaction runs.
#[derive(Debug)]
pub struct ChangeSourceRepo(Option<(BString, gix::Repository)>);

impl ChangeSourceRepo {
    /// Open the checkout `source` names.
    ///
    /// The feature flag and the worktree's active state are not re-checked: a
    /// [`ChangeSourceId::Worktree`] only exists because [`crate::IdMap`] minted an
    /// ID for an active worktree earlier in this same command, under the same
    /// worktree lock.
    pub fn open(ctx: &Context, source: &ChangeSourceId) -> anyhow::Result<Self> {
        let ChangeSourceId::Worktree(name) = source else {
            return Ok(Self(None));
        };
        // A plain from-disk open, as `ChangeSource::Worktree` requires: it shares the
        // editor repo's object database and has no object memory, so objects written
        // through it land loose and are immediately visible to the editor.
        let repo = ctx.repo.get()?;
        let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, name.as_ref())?;
        Ok(Self(Some((name.clone(), wt_repo))))
    }

    /// The repository uncommitted changes are read from, which is `main` for the
    /// main worktree.
    ///
    /// A linked worktree's repository shares `main`'s object database, so it also
    /// answers the commit and tree lookups a [`crate::utils::diff_specs::DiffSpecBuilder`]
    /// makes.
    pub fn repo<'a>(&'a self, main: &'a gix::Repository) -> &'a gix::Repository {
        match &self.0 {
            None => main,
            Some((_name, repo)) => repo,
        }
    }

    /// This checkout as the change source of an editor-backed operation.
    pub fn as_change_source(&self) -> ChangeSource<'_> {
        match &self.0 {
            None => ChangeSource::Head,
            Some((name, repo)) => ChangeSource::Worktree {
                repo,
                name: name.as_ref(),
            },
        }
    }
}
