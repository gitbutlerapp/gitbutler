//! Commands for listing, creating, archiving and removing linked git worktrees,
//! and the resolution of a [`ChangesSource`](crate::commit::json::ChangesSource) for
//! the commands that can commit from one.
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//!
//! None of the mutations here take part in the oplog: archived state is a
//! project-database row, and a checkout can neither be created nor restored from a snapshot.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use but_core::{
    branch::unique_canned_refname,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::worktrees::WorktreeEntry;
use but_workspace::worktrees::{WorktreeBranch, open_worktree_repo};
use gix::bstr::{BStr, BString, ByteSlice};
use serde::Serialize;
use tracing::instrument;

use crate::commit::json::ChangesSource;

/// A linked worktree as listed by [`worktrees_list()`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ListedWorktree {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_lossy")
    )]
    pub name: BString,
    /// The worktree checkout directory.
    #[serde(with = "but_serde::path_lossy")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub path: PathBuf,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    #[serde(with = "but_serde::fullname_lossy_opt")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::fullname_lossy_opt")
    )]
    pub ref_name: Option<gix::refs::FullName>,
    /// When the worktree or its branch was last updated according to their reflogs, in
    /// milliseconds since the epoch, or `None` without any reflog.
    pub updated_at_ms: Option<i64>,
}

/// All listable linked worktrees, separated by archived state, each most recently
/// updated first and otherwise by name.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorktreeListing {
    /// Non-archived worktrees.
    pub active: Vec<ListedWorktree>,
    /// Archived worktrees, hidden from the workspace but still on disk.
    pub archived: Vec<ListedWorktree>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ListedWorktree);
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WorktreeListing);

/// Look up the *active* linked worktree named `name`.
///
/// Every command here operates on active worktrees only - an archived one is
/// hidden from the graph, so operations against it could not be materialized.
///
/// Must not be called while a database handle is borrowed, see
/// [`but_ctx::Context::worktrees_with_state()`].
fn active_worktree(ctx: &but_ctx::Context, name: &str) -> Result<WorktreeEntry> {
    let worktree = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.name == name.as_bytes())
        .with_context(|| format!("Worktree {name} does not exist"))?;
    if worktree.archived {
        bail!("Worktree {name} is archived");
    }
    if ctx.worktree_head(worktree.name.as_bstr())?.is_none() {
        // Unborn, workspace-ref checkout, or broken - nothing to operate on.
        bail!("Worktree {name} has no usable HEAD");
    }
    Ok(worktree)
}

/// The checkout `source` names, failing for a linked worktree that isn't active.
///
/// Must not be called while a database handle is borrowed, see
/// [`but_ctx::Context::worktrees_with_state()`].
pub(crate) fn worktree_kind(
    ctx: &but_ctx::Context,
    source: &ChangesSource,
) -> Result<but_graph::WorktreeKind> {
    Ok(match source {
        ChangesSource::Head => but_graph::WorktreeKind::Main,
        ChangesSource::Worktree(name) => {
            but_graph::WorktreeKind::LinkedId(active_worktree(ctx, name)?.name)
        }
    })
}

/// Open the checkout that `source` reads its changes from, returning its stable
/// name along with a plain from-disk open of it, or `None` for the main worktree.
///
/// Callers turn this into a [`ChangeSource`](but_workspace::commit::ChangeSource)
/// for the duration of an editor-backed operation.
///
/// Must not be called while a database handle is borrowed, see
/// [`but_ctx::Context::worktrees_with_state()`].
pub(crate) fn open_changes_source(
    ctx: &but_ctx::Context,
    source: &ChangesSource,
) -> Result<Option<(BString, gix::Repository)>> {
    let ChangesSource::Worktree(name) = source else {
        return Ok(None);
    };
    let name = active_worktree(ctx, name)?.name;
    let repo = ctx.repo.get()?;
    let wt_repo = open_worktree_repo(&repo, name.as_bstr())?;
    Ok(Some((name, wt_repo)))
}

/// The branch that the linked worktree `source` names has checked out, or `None` for
/// the main worktree or a detached `HEAD`.
///
/// Operations that commit a linked worktree's changes name it in their snapshot, so undo
/// moves it back and leaves the changes uncommitted there.
pub(crate) fn changes_source_ref(
    ctx: &but_ctx::Context,
    source: &ChangesSource,
) -> Result<Option<gix::refs::FullName>> {
    let ChangesSource::Worktree(name) = source else {
        return Ok(None);
    };
    Ok(ctx
        .worktree_head(name.as_bytes().as_bstr())?
        .and_then(|head| head.ref_name))
}

/// List all usable linked worktrees, split by archived state.
#[but_api(napi, provides = [Worktrees])]
#[instrument(err(Debug))]
pub fn worktrees_list(ctx: &mut but_ctx::Context) -> Result<WorktreeListing> {
    let guard = ctx.shared_worktree_access();
    worktrees_list_with_perm(ctx, guard.read_permission())
}

/// See [`worktrees_list()`]; this variant is for callers that already hold shared worktree access.
pub fn worktrees_list_with_perm(
    ctx: &but_ctx::Context,
    _perm: &RepoShared,
) -> Result<WorktreeListing> {
    let mut listing = WorktreeListing {
        active: Vec::new(),
        archived: Vec::new(),
    };
    // This reconciles the archived state and must run before any database
    // handle is borrowed.
    let entries = ctx.worktrees_with_state()?;
    let repo = ctx.repo.get()?;
    for entry in entries {
        // A worktree with nothing to show yet (unborn, workspace-ref checkout,
        // broken) stays adopted and archivable, but is not listed.
        let Some(head) = ctx.worktree_head(entry.name.as_bstr())? else {
            continue;
        };
        // Recency is decoration - a broken reflog must not take the listing down.
        let updated_at_ms = match but_workspace::worktrees::updated_at(&repo, entry.name.as_bstr())
        {
            Ok(time) => time.map(|time| time.seconds * 1000),
            Err(err) => {
                tracing::warn!(name = %entry.name, ?err, "Could not read the worktree's reflogs");
                None
            }
        };
        let worktree = ListedWorktree {
            name: entry.name,
            path: entry.path,
            ref_name: head.ref_name,
            updated_at_ms,
        };
        if entry.archived {
            listing.archived.push(worktree);
        } else {
            listing.active.push(worktree);
        }
    }
    for worktrees in [&mut listing.active, &mut listing.archived] {
        worktrees.sort_by(|a, b| {
            b.updated_at_ms
                .cmp(&a.updated_at_ms)
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    Ok(listing)
}

/// Persist the archived state of the linked worktree named `name`.
///
/// Archived worktrees are hidden from graph traversal and only minimally listed,
/// which is how projects that predate GitButler's worktree support avoid showing
/// every worktree ever created.
#[but_api(napi, invalidates = [Worktrees, Workspace])]
#[instrument(err(Debug))]
pub fn worktree_set_archived(
    ctx: &mut but_ctx::Context,
    name: String,
    archived: bool,
) -> Result<()> {
    let guard = ctx.shared_worktree_access();
    worktree_set_archived_with_perm(
        ctx,
        BStr::new(name.as_str()),
        archived,
        guard.read_permission(),
    )
}

/// See [`worktree_set_archived()`]; this variant is for callers that already hold shared
/// worktree access.
pub fn worktree_set_archived_with_perm(
    ctx: &but_ctx::Context,
    name: &BStr,
    archived: bool,
    _perm: &RepoShared,
) -> Result<()> {
    ctx.set_worktree_archived(name, archived)?;
    // A cached workspace would still be seeded with the old set of worktree tips.
    ctx.invalidate_workspace_cache()
}

/// Remove the linked worktree named `name` from disk the way `git worktree remove` does,
/// which refuses a dirty checkout unless `force`, and forget its archived state so a
/// worktree created under the same name later starts out active.
///
/// A locked worktree is removed like any other.
///
/// This works on archived worktrees as well.
#[but_api(napi, invalidates = [Worktrees, Workspace])]
#[instrument(err(Debug))]
pub fn worktree_remove(ctx: &mut but_ctx::Context, name: String, force: bool) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    worktree_remove_with_perm(
        ctx,
        BStr::new(name.as_str()),
        force,
        guard.write_permission(),
    )
}

/// See [`worktree_remove()`]; this variant is for callers that already hold exclusive
/// worktree access.
pub fn worktree_remove_with_perm(
    ctx: &but_ctx::Context,
    name: &BStr,
    force: bool,
    _perm: &mut RepoExclusive,
) -> Result<()> {
    let worktree = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.name == name)
        .with_context(|| format!("Worktree {name} does not exist"))?;
    let repo = ctx.repo.get()?;
    but_workspace::worktrees::remove(&repo, worktree.name.as_ref(), force)?;
    ctx.db
        .get_cache_mut()?
        .worktree_meta_mut()
        .delete(&worktree.name)?;
    ctx.invalidate_workspace_cache()
}

/// A linked worktree freshly created by [`worktree_new()`] or [`worktree_new_on_branch()`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NewWorktree {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_lossy")
    )]
    pub name: BString,
    /// The worktree checkout directory.
    #[serde(with = "but_serde::path_lossy")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub path: PathBuf,
    /// The branch checked out in the worktree.
    #[serde(with = "but_serde::fullname_lossy")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::fullname_lossy")
    )]
    pub ref_name: gix::refs::FullName,
    /// The commit the worktree starts at.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub base: gix::ObjectId,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(NewWorktree);

/// Create a linked worktree on a new branch starting at the workspace's highest base, see
/// [`but_graph::Workspace::highest_base()`].
///
/// The branch is `new_ref` or a canned name, and the checkout lives at
/// `~/.gitbutler-worktrees/<repo-dir-basename>/<slug>`, where the repository directory is
/// the main worktree. The slug is derived from the short branch name, falling back to `wt`
/// if empty. If the directory is taken, creation retries up to five times with a random ASCII suffix.
/// This fails without a target to base the worktree on, and refuses an existing branch, see
/// [`worktree_new_on_branch()`] for that.
#[but_api(napi, invalidates = [Worktrees, Workspace])]
#[instrument(err(Debug))]
pub fn worktree_new(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::MaybeLossyFullNameRef)] new_ref: Option<gix::refs::FullName>,
) -> Result<NewWorktree> {
    let mut guard = ctx.exclusive_worktree_access();
    worktree_new_with_perm(ctx, new_ref, guard.write_permission())
}

/// See [`worktree_new()`]; this variant is for callers that already hold exclusive
/// worktree access.
pub fn worktree_new_with_perm(
    ctx: &but_ctx::Context,
    new_ref: Option<gix::refs::FullName>,
    perm: &mut RepoExclusive,
) -> Result<NewWorktree> {
    let base = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        ws.highest_base()
            .context("The workspace has no target to base a new worktree on")?
    };
    worktree_new_at_base_with_perm(ctx, new_ref, base, perm)
}

/// Create a worktree on a new branch at a given base.
pub fn worktree_new_at_base_with_perm(
    ctx: &but_ctx::Context,
    new_ref: Option<gix::refs::FullName>,
    base: gix::ObjectId,
    perm: &mut RepoExclusive,
) -> Result<NewWorktree> {
    let ref_name = match new_ref {
        Some(ref_name) => ref_name,
        None => unique_canned_refname(&*ctx.repo.get()?)?,
    };
    let (name, path) = add_worktree(
        ctx,
        WorktreeBranch::New {
            name: ref_name.as_ref(),
            base,
        },
        perm,
    )?;
    Ok(NewWorktree {
        name,
        path,
        ref_name,
        base,
    })
}

/// Create a linked worktree on the existing local branch `ref_name`, placed like [`worktree_new()`].
///
/// This refuses a branch that is part of the workspace, checked out in any worktree, or whose
/// tip is conflicted.
#[but_api(napi, invalidates = [Worktrees, Workspace])]
#[instrument(err(Debug))]
pub fn worktree_new_on_branch(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::FullNameBytes)] ref_name: gix::refs::FullName,
) -> Result<NewWorktree> {
    let mut guard = ctx.exclusive_worktree_access();
    worktree_new_on_branch_with_perm(ctx, ref_name, guard.write_permission())
}

/// See [`worktree_new_on_branch()`]; this variant is for callers that already hold exclusive
/// worktree access.
pub fn worktree_new_on_branch_with_perm(
    ctx: &but_ctx::Context,
    ref_name: gix::refs::FullName,
    perm: &mut RepoExclusive,
) -> Result<NewWorktree> {
    let base = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        if ws.refname_is_segment(ref_name.as_ref()) {
            bail!(
                "Branch '{}' is already in the workspace",
                ref_name.shorten()
            );
        }
        let tip = repo
            .find_reference(ref_name.as_ref())
            .with_context(|| format!("Branch '{}' does not exist", ref_name.shorten()))?
            .peel_to_commit()?;
        if but_core::Commit::from_id(tip.id())?.is_conflicted() {
            bail!("Cannot create a worktree on a conflicted commit");
        }
        tip.id
    };
    let (name, path) = add_worktree(ctx, WorktreeBranch::Existing(ref_name.as_ref()), perm)?;
    Ok(NewWorktree {
        name,
        path,
        ref_name,
        base,
    })
}

fn add_worktree(
    ctx: &but_ctx::Context,
    branch: WorktreeBranch<'_>,
    _perm: &mut RepoExclusive,
) -> Result<(BString, PathBuf)> {
    let repo = ctx.repo.get()?;
    let slug = slug(branch.name().shorten());
    let slug = if slug.is_empty() { "wt" } else { &slug };
    let repo_name = repo
        .workdir()
        .context("Cannot create a worktree without a main working directory")?
        .file_name()
        .context("Cannot derive a repository directory basename")?;
    let root = but_path::home_dir()
        .context("Could not resolve the home directory for worktrees")?
        .join(".gitbutler-worktrees")
        .join(repo_name);
    let (name, path) = create_with_suffix(
        &root,
        slug,
        || uuid::Uuid::new_v4().simple().to_string()[..8].to_owned(),
        |path| but_workspace::worktrees::add(&repo, path, branch),
    )?;
    let path = gix::path::realpath(&path)?;
    drop(repo);
    ctx.invalidate_workspace_cache()?;
    Ok((name, path))
}

const CREATION_RETRIES: usize = 5;

/// Retry only refusals made before creation changes any repository state. In particular,
/// a checkout/copy failure may leave a path behind, but must never trigger a retry.
fn create_with_suffix(
    root: &std::path::Path,
    slug: &str,
    mut suffix: impl FnMut() -> String,
    mut create: impl FnMut(&std::path::Path) -> Result<BString>,
) -> Result<(BString, PathBuf)> {
    let mut path = root.join(slug);
    for attempt in 0..=CREATION_RETRIES {
        match create(&path) {
            Ok(name) => return Ok((name, path)),
            Err(err)
                if err.is::<but_workspace::worktrees::DestinationExists>()
                    && attempt < CREATION_RETRIES =>
            {
                path = root.join(format!("{slug}-{}", suffix()));
            }
            Err(err) => return Err(err),
        }
    }
    unreachable!("the last attempt returns either success or error")
}

/// Turn `name` into a single lowercase ASCII path component, keeping letters and digits and
/// collapsing everything else into single hyphens. Empty if nothing survives.
fn slug(name: &BStr) -> String {
    let mut slug = String::new();
    for byte in name.iter() {
        if byte.is_ascii_alphanumeric() {
            slug.push(byte.to_ascii_lowercase() as char);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::{CREATION_RETRIES, create_with_suffix};
    use but_workspace::worktrees::DestinationExists;
    use std::path::Path;

    #[test]
    fn collisions_retry_with_fresh_suffixes_and_stop_on_success() -> anyhow::Result<()> {
        let mut suffixes = ["aaaaaaaa", "bbbbbbbb"].into_iter();
        let mut attempted = Vec::new();
        let (name, path) = create_with_suffix(
            Path::new("root"),
            "feature",
            || suffixes.next().expect("only two retries").to_owned(),
            |path| {
                attempted.push(path.to_owned());
                if attempted.len() < 3 {
                    Err(DestinationExists(path.to_owned()).into())
                } else {
                    Ok("feature-bbbbbbbb".into())
                }
            },
        )?;
        assert_eq!(
            attempted,
            [
                "root/feature",
                "root/feature-aaaaaaaa",
                "root/feature-bbbbbbbb"
            ]
            .map(std::path::PathBuf::from),
            "retry suffixes replace rather than accumulate"
        );
        assert_eq!(name, "feature-bbbbbbbb", "return actual registered name");
        assert_eq!(
            path,
            Path::new("root/feature-bbbbbbbb"),
            "return successful destination"
        );
        Ok(())
    }

    #[test]
    fn collisions_exhaust_the_retry_limit() {
        let mut attempts = 0;
        let mut suffixes = 0;
        let err = create_with_suffix(
            Path::new("root"),
            "feature",
            || {
                suffixes += 1;
                format!("{suffixes:08x}")
            },
            |path| {
                attempts += 1;
                Err(DestinationExists(path.to_owned()).into())
            },
        )
        .unwrap_err();
        assert_eq!(
            attempts,
            CREATION_RETRIES + 1,
            "initial attempt plus bounded retries"
        );
        assert_eq!(
            suffixes, CREATION_RETRIES,
            "no suffix generated before first failure or after exhaustion"
        );
        assert_eq!(
            err.downcast_ref::<DestinationExists>().unwrap().0,
            Path::new("root/feature-00000005"),
            "preserve final creation error"
        );
    }

    #[test]
    fn unrelated_errors_do_not_retry_even_with_an_occupied_destination() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::create_dir(root.path().join("feature"))?;
        let mut attempts = 0;
        let err = create_with_suffix(
            root.path(),
            "feature",
            || panic!("unrelated errors must not generate suffixes"),
            |_| {
                attempts += 1;
                // An AlreadyExists from a later copy/checkout step is not a destination refusal.
                Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "copy failed").into())
            },
        )
        .unwrap_err();
        assert_eq!(attempts, 1, "only destination refusals can retry");
        assert_eq!(
            err.to_string(),
            "copy failed",
            "preserve unrelated creation error"
        );
        Ok(())
    }

    #[test]
    fn slug() {
        for (input, expected) in [
            ("Caleb/Foo bar", "caleb-foo-bar"),
            ("--a--", "a"),
            ("ünïcode", "n-code"),
            ("///", ""),
        ] {
            assert_eq!(super::slug(input.into()), expected, "{input}");
        }
    }
}
