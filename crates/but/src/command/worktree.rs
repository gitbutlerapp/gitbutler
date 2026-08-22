use std::path::PathBuf;

use bstr::BString;
use but_api::worktrees::{ListedWorktree, WorktreeListing};
use but_core::sync::RepoShared;
use but_ctx::Context;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::atoms::CliIdArg,
    bad_input,
    id::ShortId,
    theme::{Paint, Theme},
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

/// How many archived worktrees a plain `but wt list` shows.
const ARCHIVED_PREVIEW: usize = 3;

pub fn list(ctx: &Context, archived: bool, active: bool) -> CliResult<ListOutcome> {
    let guard = ctx.shared_worktree_access();
    let listing = but_api::worktrees::worktrees_list_with_perm(ctx, guard.read_permission())?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let short_id = |worktree: &ListedWorktree| {
        id_map
            .worktrees
            .values()
            .find(|with_id| with_id.name == worktree.name)
            .map(|with_id| with_id.short_id.clone())
    };
    let WorktreeListing {
        active: active_worktrees,
        archived: archived_worktrees,
    } = listing;
    let rows = |worktrees: Vec<ListedWorktree>| {
        worktrees
            .into_iter()
            .map(|worktree| Row {
                id: short_id(&worktree),
                name: worktree.name,
                ref_name: worktree.ref_name,
                path: worktree.path,
                updated_at_ms: worktree.updated_at_ms,
            })
            .collect()
    };
    Ok(ListOutcome {
        active: (active || !archived).then(|| rows(active_worktrees)),
        archived: (archived || !active).then(|| rows(archived_worktrees)),
        archived_limit: (!active && !archived).then_some(ARCHIVED_PREVIEW),
    })
}

pub fn set_archived(
    ctx: &Context,
    worktree: &CliIdArg,
    archived: bool,
) -> CliResult<ArchiveOutcome> {
    let guard = ctx.shared_worktree_access();
    let name = resolve(ctx, worktree, guard.read_permission())?;
    but_api::worktrees::worktree_set_archived_with_perm(
        ctx,
        name.as_ref(),
        archived,
        guard.read_permission(),
    )?;
    Ok(ArchiveOutcome { name, archived })
}

pub fn remove(ctx: &mut Context, worktree: &CliIdArg, force: bool) -> CliResult<RemoveOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let name = resolve(ctx, worktree, guard.read_permission())?;
    but_api::worktrees::worktree_remove_with_perm(
        ctx,
        name.as_ref(),
        force,
        guard.write_permission(),
    )?;
    Ok(RemoveOutcome { name })
}

/// The stable name of the worktree `arg` refers to: its exact name first, as archived
/// worktrees have no CLI ID, then the CLI ID of an active one.
fn resolve(ctx: &Context, arg: &CliIdArg, perm: &RepoShared) -> CliResult<BString> {
    but_api::worktrees::ensure_worktree_manipulation_enabled(ctx)?;
    if let Some(worktree) = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.name == arg.0.as_bytes())
    {
        return Ok(worktree.name);
    }
    let repo = ctx.repo.get()?;
    let id_map = IdMap::new_from_context(ctx, perm)?;
    if let Some(name) = arg.try_resolve_worktree(&repo, &id_map)? {
        return Ok(name);
    }
    Err(bad_input(format!("Could not find worktree: '{arg}'"))
        .hint("Run `but worktree list` for the worktrees and their IDs.")
        .into())
}

struct Row {
    /// Only active worktrees with a usable `HEAD` have one.
    id: Option<ShortId>,
    name: BString,
    ref_name: Option<gix::refs::FullName>,
    path: PathBuf,
    updated_at_ms: Option<i64>,
}

impl Row {
    fn write_human(&self, out: &mut dyn WriteWithUtils, theme: &Theme) -> anyhow::Result<()> {
        if let Some(id) = &self.id {
            write!(out, "{} ", theme.cli_id.paint(id))?;
        }
        write!(out, "{}", self.name)?;
        match &self.ref_name {
            Some(ref_name) if ref_name.shorten() != self.name => write!(out, " ({ref_name})")?,
            Some(_) => {}
            None => write!(out, " (detached)")?,
        }
        writeln!(out, " - {}", self.path.display())?;
        Ok(())
    }
}

/// The sections asked for, with the archived one cut off after `archived_limit` entries in
/// human output.
#[must_use]
pub struct ListOutcome {
    active: Option<Vec<Row>>,
    archived: Option<Vec<Row>>,
    archived_limit: Option<usize>,
}

fn write_section(
    out: &mut dyn WriteWithUtils,
    theme: &Theme,
    title: &str,
    rows: &[Row],
    limit: Option<usize>,
) -> anyhow::Result<()> {
    writeln!(out, "{title}")?;
    if rows.is_empty() {
        writeln!(out, "(none)")?;
    }
    let shown = limit.unwrap_or(rows.len()).min(rows.len());
    for row in &rows[..shown] {
        row.write_human(out, theme)?;
    }
    let hidden = rows.len() - shown;
    if hidden > 0 {
        writeln!(out, "and {hidden} more... Use `--archived` to list all.")?;
    }
    Ok(())
}

impl CliOutputHuman for ListOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        theme: &'static Theme,
    ) -> anyhow::Result<()> {
        if let Some(active) = &self.active {
            write_section(out, theme, "Active worktrees", active, None)?;
        }
        if let Some(archived) = &self.archived {
            if self.active.is_some() {
                writeln!(out)?;
            }
            write_section(
                out,
                theme,
                "Archived worktrees",
                archived,
                self.archived_limit,
            )?;
        }
        Ok(())
    }
}

impl CliOutput for ListOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Worktree {
            id: Option<ShortId>,
            name: String,
            ref_name: Option<String>,
            path: String,
            updated_at_ms: Option<i64>,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            #[serde(skip_serializing_if = "Option::is_none")]
            active: Option<Vec<Worktree>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            archived: Option<Vec<Worktree>>,
        }

        let rows = |rows: Vec<Row>| {
            rows.into_iter()
                .map(|row| Worktree {
                    id: row.id,
                    name: row.name.to_string(),
                    ref_name: row.ref_name.map(|name| name.to_string()),
                    path: row.path.display().to_string(),
                    updated_at_ms: row.updated_at_ms,
                })
                .collect()
        };
        Output {
            active: self.active.map(rows),
            archived: self.archived.map(rows),
        }
    }
}

#[must_use]
pub struct ArchiveOutcome {
    name: BString,
    archived: bool,
}

impl CliOutputHuman for ArchiveOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let verb = if self.archived {
            "archived"
        } else {
            "unarchived"
        };
        writeln!(out, "Successfully {verb} {}", self.name)?;
        Ok(())
    }
}

impl CliOutput for ArchiveOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        struct Output {
            name: String,
            archived: bool,
        }
        Output {
            name: self.name.to_string(),
            archived: self.archived,
        }
    }
}

#[must_use]
pub struct RemoveOutcome {
    name: BString,
}

impl CliOutputHuman for RemoveOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        writeln!(out, "Removed worktree {}", self.name)?;
        Ok(())
    }
}

impl CliOutput for RemoveOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        struct Output {
            name: String,
        }
        Output {
            name: self.name.to_string(),
        }
    }
}
