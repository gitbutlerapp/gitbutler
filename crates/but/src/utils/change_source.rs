//! The checkout an operation reads its uncommitted changes from: identifying one,
//! listing what each has, and opening the repository behind it.
//!
//! CLI IDs for uncommitted changes are minted in one flat namespace across every
//! checkout, so [`ChangeSourceId`] is what keeps them apart. This module owns that
//! concept end to end; `crate::id` only mints the IDs.

use bstr::{BStr, BString};
use but_ctx::Context;
use but_workspace::commit::ChangeSource;
use nonempty::NonEmpty;

use crate::{CliResult, bad_input, id::UncommittedHunkOrFile};

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

    /// The container selector that scopes a path to this checkout, i.e. the `X` in
    /// `X:<path>`.
    pub fn selector(&self) -> &BStr {
        match self {
            ChangeSourceId::Head => BStr::new(crate::id::UNCOMMITTED),
            ChangeSourceId::Worktree(name) => name.as_ref(),
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
    let mut names = Vec::new();
    for worktree in ctx.worktrees_with_state()? {
        if worktree.archived {
            continue;
        }
        // A worktree without a usable `HEAD` (unborn, vanished, or checking out
        // the workspace ref) cannot be diffed, and operations refuse it - minting
        // an ID for it would either fail every command or print IDs that nothing
        // accepts.
        if ctx.worktree_head(worktree.name.as_ref())?.is_none() {
            continue;
        }
        names.push(worktree.name);
    }
    Ok(names)
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

/// The single checkout that all of `sources` name.
///
/// An operation reads its changes from one repository and cancels them out in that
/// same checkout, so a selection spanning several of them could not be applied in
/// one go. An empty selection is the main worktree, which is what "everything
/// uncommitted" and "nothing at all" both mean.
pub fn single_source(
    sources: impl IntoIterator<Item = ChangeSourceId>,
) -> CliResult<ChangeSourceId> {
    let mut sources = sources.into_iter();
    let Some(source) = sources.next() else {
        return Ok(ChangeSourceId::Head);
    };
    if let Some(other) = sources.find(|other| *other != source) {
        return Err(bad_input(format!(
            "Cannot use changes from {} and {} together",
            source.describe(),
            other.describe()
        ))
        .hint("An operation can only take changes from one checkout at a time")
        .into());
    }
    Ok(source)
}

/// Uncommitted changes that all come from one checkout.
///
/// The invariant lives here rather than in a check before each use: an operation
/// reads its changes from one repository and cancels them out in that same
/// checkout, so a selection spanning several of them is not a thing that can be
/// acted on. Construction is the only way in, so nothing downstream needs to
/// re-validate or carry an error path for it.
#[derive(Debug)]
pub struct UncommittedSelection {
    source: ChangeSourceId,
    changes: NonEmpty<UncommittedHunkOrFile>,
}

impl UncommittedSelection {
    /// Errors when `changes` span several checkouts.
    pub fn new(changes: NonEmpty<UncommittedHunkOrFile>) -> CliResult<Self> {
        let source = single_source(changes.iter().map(|change| change.source.clone()))?;
        Ok(Self { source, changes })
    }

    /// The checkout every change was read from.
    pub fn source(&self) -> &ChangeSourceId {
        &self.source
    }

    /// The changes themselves.
    pub fn into_changes(self) -> NonEmpty<UncommittedHunkOrFile> {
        self.changes
    }
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

    /// The checkout this reads from, for pairing a
    /// [`crate::utils::diff_specs::DiffSpecBuilder`] with the repository it was
    /// constructed on.
    pub fn source(&self) -> ChangeSourceId {
        match &self.0 {
            None => ChangeSourceId::Head,
            Some((name, _repo)) => ChangeSourceId::Worktree(name.clone()),
        }
    }

    /// The repository uncommitted changes are read from, which is `main` for the
    /// main worktree.
    ///
    /// A linked worktree's repository shares `main`'s object database, so it also
    /// answers the commit and tree lookups a
    /// [`crate::utils::diff_specs::DiffSpecBuilder`] makes.
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
