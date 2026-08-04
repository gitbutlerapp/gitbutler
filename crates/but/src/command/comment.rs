//! Implementation of the `but _comment` command.

use but_api::comments::{self, store};
use but_comments::{DiffComment, DiffSide, NewComment, StoredComment};
use but_core::sync::RepoShared;
use but_ctx::Context;
use gix::prelude::ObjectIdExt as _;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{Purpose, ResolvedCliIdArg},
        comment::{Platform, Subcommands},
    },
    bad_input,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

#[derive(Debug)]
pub enum CommentOperation {
    List,
    Archive {
        /// The full id of the comment to archive.
        id: String,
    },
    /// The named comment is already archived (typically auto-archived because its anchor
    /// disappeared, often through the agent's own fix): a success no-op, never an error.
    ArchiveAlreadyDone {
        /// The full id of the already-archived comment.
        id: String,
    },
    Add {
        path: String,
        /// The commit whose diff to anchor to, or `None` for the uncommitted worktree diff.
        commit_id: Option<gix::ObjectId>,
        side: DiffSide,
        line_number: u32,
        payload: String,
    },
}

#[must_use]
pub enum CommentOutcome {
    Listed(Vec<DiffComment>),
    WaitTimedOut { timeout_secs: u64 },
    Archived { id: String },
    AlreadyArchived { id: String },
    Added(DiffComment),
}

impl CliOutputHuman for CommentOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            CommentOutcome::Listed(comments) if comments.is_empty() => {
                writeln!(out, "No comments")?;
            }
            CommentOutcome::Listed(comments) => {
                for (index, comment) in comments.into_iter().enumerate() {
                    if index > 0 {
                        writeln!(out)?;
                    }
                    write_comment(out, &comment, true)?;
                }
            }
            CommentOutcome::WaitTimedOut { timeout_secs } => {
                writeln!(
                    out,
                    "No comments appeared within {timeout_secs}s. Run `but _comment list --wait` again to keep waiting."
                )?;
            }
            CommentOutcome::Archived { id } => {
                writeln!(out, "Archived comment {id}")?;
            }
            CommentOutcome::AlreadyArchived { id } => {
                writeln!(out, "Comment {id} was already archived; nothing to do")?;
            }
            CommentOutcome::Added(comment) => {
                writeln!(out, "Added comment")?;
                write_comment(out, &comment, false)?;
            }
        }
        Ok(())
    }
}

fn write_comment(
    out: &mut dyn WriteWithUtils,
    comment: &DiffComment,
    with_context: bool,
) -> anyhow::Result<()> {
    let scope = match &comment.commit_change_id {
        None => "uncommitted".to_string(),
        Some(change_id) => format!("commit {change_id}"),
    };
    let side = match comment.side {
        DiffSide::Old => ", old side",
        DiffSide::New => "",
    };
    writeln!(
        out,
        "[{}] {}:{} ({scope}{side})",
        comment.id, comment.path, comment.line_number
    )?;
    for line in comment.payload.lines() {
        writeln!(out, "  {line}")?;
    }
    if with_context && let Some(context) = &comment.context {
        for line in context.lines() {
            writeln!(out, "  | {line}")?;
        }
    }
    Ok(())
}

impl CliOutput for CommentOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(
            tag = "type",
            rename_all = "camelCase",
            rename_all_fields = "camelCase"
        )]
        enum Output {
            Listed { comments: Vec<DiffComment> },
            Archived { archived: String },
            Added { comment: DiffComment },
        }

        match self {
            CommentOutcome::Listed(comments) => Output::Listed { comments },
            // The timeout is an empty listing as far as consumers are concerned.
            CommentOutcome::WaitTimedOut { .. } => Output::Listed { comments: vec![] },
            // Already-archived is indistinguishable from archiving for consumers: the goal
            // state is reached either way.
            CommentOutcome::Archived { id } | CommentOutcome::AlreadyArchived { id } => {
                Output::Archived { archived: id }
            }
            CommentOutcome::Added(comment) => Output::Added { comment },
        }
    }
}

pub fn comment(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<CommentOutcome> {
    // Waiting must not hold the shared worktree guard across its sleeps — that would block
    // exclusive operations (like the GUI committing) for the whole wait — so it manages its own
    // short-lived guards per poll instead of the acquire-once flow below.
    if let Subcommands::List {
        wait: true,
        timeout,
    } = args.cmd
    {
        return Ok(run_wait(ctx, timeout)?);
    }

    let guard = ctx.shared_worktree_access();
    let perm = guard.read_permission();
    let operation = resolve(ctx, args, perm)?;
    Ok(run(ctx, operation, perm)?)
}

/// Poll for unarchived comments until one survives re-anchoring or `timeout_secs` elapses.
///
/// The cheap existence check reads only the comments file; the full (diff-computing,
/// auto-archiving) listing runs only when rows the CLI could actually surface exist. A row can
/// exist yet produce an empty listing when its anchor is gone — the listing archives it and the
/// wait continues.
fn run_wait(ctx: &Context, timeout_secs: u64) -> anyhow::Result<CommentOutcome> {
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
    /// When rows exist but none are listable (e.g. their commit's branch is unapplied), every
    /// poll pays for a full listing — back off so a watching agent doesn't burn a status scan
    /// every two seconds indefinitely.
    const POLL_INTERVAL_NOTHING_LISTABLE: std::time::Duration = std::time::Duration::from_secs(10);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        let mut interval = POLL_INTERVAL;
        // Blank rows are invisible to the CLI, so they must not trigger the expensive listing.
        let rows_exist = store(ctx)
            .read()
            .iter()
            .any(|comment| comment.archived_at_ms.is_none() && !comment.payload.trim().is_empty());
        if rows_exist {
            // Long waits outlive the cached workspace projection: a commit created mid-wait
            // must be resolvable or its comments would be treated as scope-less.
            ctx.invalidate_workspace_cache()?;
            let comments = {
                let guard = ctx.shared_worktree_access();
                comments::comments_list_with_perm(ctx, guard.read_permission())?
            };
            let comments = without_blank_payloads(comments);
            if !comments.is_empty() {
                return Ok(CommentOutcome::Listed(comments));
            }
            interval = POLL_INTERVAL_NOTHING_LISTABLE;
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Ok(CommentOutcome::WaitTimedOut { timeout_secs });
        }
        std::thread::sleep(remaining.min(interval));
    }
}

/// Hide comments without any text yet: the GUI creates the backend comment the moment the
/// gutter button is clicked, before the user has typed anything, and agents must not wake up on
/// (or act on) those.
fn without_blank_payloads(comments: Vec<DiffComment>) -> Vec<DiffComment> {
    comments
        .into_iter()
        .filter(|comment| !comment.payload.trim().is_empty())
        .collect()
}

fn resolve(ctx: &Context, args: Platform, perm: &RepoShared) -> CliResult<CommentOperation> {
    match args.cmd {
        Subcommands::List { wait: false, .. } => Ok(CommentOperation::List),
        Subcommands::List { wait: true, .. } => {
            unreachable!("waiting listings are dispatched before resolve")
        }
        Subcommands::Archive { id } => {
            let matches = store(ctx)
                .read()
                .into_iter()
                .filter(|comment| comment.id.starts_with(&id))
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Err(bad_input("No comment with this id")
                    .arg_name("<ID>")
                    .arg_value(id)
                    .hint("Use `but _comment list` to see the ids of all comments")
                    .into());
            }
            let unarchived: Vec<&StoredComment> = matches
                .iter()
                .filter(|comment| comment.archived_at_ms.is_none())
                .collect();
            match unarchived.as_slice() {
                [comment] => Ok(CommentOperation::Archive {
                    id: comment.id.clone(),
                }),
                // The prefix uniquely names an archived comment: the goal state is already
                // reached (typically it was auto-archived because the agent's own fix removed
                // the anchored line). Anything ambiguous stays an error, even when every match
                // is archived — success must not claim a comment the caller didn't identify.
                [] if matches.len() == 1 => Ok(CommentOperation::ArchiveAlreadyDone {
                    id: matches[0].id.clone(),
                }),
                _ => Err(bad_input("The id prefix matches more than one comment")
                    .arg_name("<ID>")
                    .arg_value(id)
                    .hint("Use more characters of the id shown by `but _comment list`")
                    .into()),
            }
        }
        Subcommands::Add {
            anchor,
            message,
            commit,
            old,
        } => {
            let (path, line_number) = anchor
                .rsplit_once(':')
                .and_then(|(path, line)| Some((path, line.parse::<u32>().ok()?)))
                .filter(|(path, line)| !path.is_empty() && *line > 0)
                .ok_or_else(|| {
                    bad_input("The anchor must have the form `<path>:<line>`")
                        .arg_name("<ANCHOR>")
                        .arg_value(anchor.clone())
                        .hint("For example `src/main.rs:42`")
                })?;
            let commit_id = commit
                .map(|commit| -> CliResult<gix::ObjectId> {
                    let repo = ctx.repo.get()?;
                    let id_map = IdMap::new_from_context(ctx, perm)?;
                    let value = commit.to_string();
                    match commit.resolve_in_workspace(&repo, &id_map, Purpose::Target, None)? {
                        ResolvedCliIdArg::Commit(commit) => Ok(commit.commit_id),
                        _ => Err(bad_input("Only commits can be commented on")
                            .arg_name("--commit")
                            .arg_value(value)
                            .hint("Use a commit CLI ID or change id from `but status`")
                            .into()),
                    }
                })
                .transpose()?;
            Ok(CommentOperation::Add {
                path: path.to_string(),
                commit_id,
                side: if old { DiffSide::Old } else { DiffSide::New },
                line_number,
                payload: message,
            })
        }
    }
}

pub fn run(
    ctx: &Context,
    operation: CommentOperation,
    perm: &RepoShared,
) -> anyhow::Result<CommentOutcome> {
    match operation {
        CommentOperation::List => Ok(CommentOutcome::Listed(without_blank_payloads(
            comments::comments_list_with_perm(ctx, perm)?,
        ))),
        CommentOperation::Archive { id } => {
            // `false` means another process archived it between resolving and now — the goal
            // state is reached either way.
            if comments::comment_archive(ctx, id.clone())? {
                Ok(CommentOutcome::Archived { id })
            } else {
                Ok(CommentOutcome::AlreadyArchived { id })
            }
        }
        CommentOperation::ArchiveAlreadyDone { id } => Ok(CommentOutcome::AlreadyArchived { id }),
        CommentOperation::Add {
            path,
            commit_id,
            side,
            line_number,
            payload,
        } => {
            let commit_change_id = commit_id
                .map(|commit_id| -> anyhow::Result<String> {
                    let repo = ctx.repo.get()?;
                    // Use the same change-id derivation the list-side anchor resolution uses,
                    // so commits without a stored change-id header still round-trip.
                    Ok(but_core::Commit::from_id(commit_id.attach(&repo))?
                        .change_id()
                        .to_string())
                })
                .transpose()?;
            let comment = comments::comment_create_with_perm(
                ctx,
                NewComment {
                    id: None,
                    path,
                    commit_change_id,
                    side,
                    line_number,
                    payload,
                },
                perm,
            )?;
            Ok(CommentOutcome::Added(comment))
        }
    }
}
