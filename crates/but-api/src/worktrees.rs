//! Commands for listing, creating, archiving and removing linked git worktrees (experimental),
//! and the resolution of a [`ChangesSource`](crate::commit::json::ChangesSource) for
//! the commands that can commit from one.
//!
//! All commands here are gated on the `featureFlags.worktreeManipulation` setting.
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
use but_workspace::worktrees::open_worktree_repo;
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

/// Fail unless the user opted into worktree manipulation.
pub fn ensure_worktree_manipulation_enabled(ctx: &but_ctx::Context) -> Result<()> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        bail!("worktree manipulation is not enabled (featureFlags.worktreeManipulation)");
    }
    Ok(())
}

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
    ensure_worktree_manipulation_enabled(ctx)?;
    let name = active_worktree(ctx, name)?.name;
    let repo = ctx.repo.get()?;
    let wt_repo = open_worktree_repo(&repo, name.as_bstr())?;
    Ok(Some((name, wt_repo)))
}

/// List all usable linked worktrees, split by archived state.
#[but_api(napi, provides = [Worktrees])]
#[instrument(err(Debug))]
pub fn worktrees_list(ctx: &mut but_ctx::Context) -> Result<WorktreeListing> {
    ensure_worktree_manipulation_enabled(ctx)?;
    let guard = ctx.shared_worktree_access();
    worktrees_list_with_perm(ctx, guard.read_permission())
}

/// See [`worktrees_list()`]; this variant is for callers that already hold shared worktree access.
pub fn worktrees_list_with_perm(
    ctx: &but_ctx::Context,
    _perm: &RepoShared,
) -> Result<WorktreeListing> {
    ensure_worktree_manipulation_enabled(ctx)?;
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
    ensure_worktree_manipulation_enabled(ctx)?;
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
    ensure_worktree_manipulation_enabled(ctx)?;
    ctx.set_worktree_archived(name, archived)?;
    // A cached workspace would still be seeded with the old set of worktree tips.
    ctx.invalidate_workspace_cache()
}

/// Remove the linked worktree named `name` from disk the way `git worktree remove` does,
/// which refuses a dirty checkout unless `force` and a locked one until it is unlocked, and
/// forget its archived state so a worktree created under the same name later starts out
/// active.
///
/// This works on archived worktrees as well.
#[but_api(napi, invalidates = [Worktrees, Workspace])]
#[instrument(err(Debug))]
pub fn worktree_remove(ctx: &mut but_ctx::Context, name: String, force: bool) -> Result<()> {
    ensure_worktree_manipulation_enabled(ctx)?;
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
    ensure_worktree_manipulation_enabled(ctx)?;
    let worktree = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.name == name)
        .with_context(|| format!("Worktree {name} does not exist"))?;
    let repo = ctx.repo.get()?;
    but_workspace::worktrees::remove(&repo, &worktree.path, force)?;
    ctx.db
        .get_cache_mut()?
        .worktree_meta_mut()
        .delete(&worktree.name)?;
    ctx.invalidate_workspace_cache()
}

/// A linked worktree freshly created by [`worktree_new()`].
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
    /// The branch created for and checked out in the worktree.
    #[serde(with = "but_serde::fullname_lossy")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::fullname_lossy")
    )]
    pub ref_name: gix::refs::FullName,
    /// The commit the branch starts at, the workspace's highest base.
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
/// `$GIT_COMMON_DIR/gb-wts/<slug>`, where the slug of the short branch name also names the
/// worktree. This fails without a target to base the worktree on, and refuses an existing
/// branch or directory.
#[but_api(napi, invalidates = [Worktrees, Workspace])]
#[instrument(err(Debug))]
pub fn worktree_new(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::MaybeLossyFullNameRef)] new_ref: Option<gix::refs::FullName>,
) -> Result<NewWorktree> {
    ensure_worktree_manipulation_enabled(ctx)?;
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
    ensure_worktree_manipulation_enabled(ctx)?;
    let (repo, ws, db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
    let base = ws
        .highest_base()
        .context("The workspace has no target to base a new worktree on")?;
    let ref_name = match new_ref {
        Some(ref_name) => ref_name,
        None => unique_canned_refname(&repo)?,
    };
    let slug = slug(ref_name.shorten());
    if slug.is_empty() {
        bail!(
            "Cannot derive a worktree directory from '{}'",
            ref_name.shorten()
        );
    }
    let path = repo.common_dir().join("gb-wts").join(&slug);
    let name = but_workspace::worktrees::add(&repo, &path, ref_name.as_ref(), base)?;
    let path = gix::path::realpath(&path)?;
    drop((repo, ws, db));
    ctx.invalidate_workspace_cache()?;
    Ok(NewWorktree {
        name,
        path,
        ref_name,
        base,
    })
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
