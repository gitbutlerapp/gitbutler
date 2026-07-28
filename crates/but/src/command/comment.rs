//! Implementation of the `but comment` command.

use but_api::comments::{self, DiffComment, DiffSide, NewComment};
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
    Archived { id: String },
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
            CommentOutcome::Archived { id } => {
                writeln!(out, "Archived comment {id}")?;
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
        #[serde(untagged, rename_all_fields = "camelCase")]
        enum Output {
            Listed { comments: Vec<DiffComment> },
            Archived { archived: String },
            Added { comment: DiffComment },
        }

        match self {
            CommentOutcome::Listed(comments) => Output::Listed { comments },
            CommentOutcome::Archived { id } => Output::Archived { archived: id },
            CommentOutcome::Added(comment) => Output::Added { comment },
        }
    }
}

pub fn comment(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<CommentOutcome> {
    let guard = ctx.shared_worktree_access();
    let perm = guard.read_permission();
    let operation = resolve(ctx, args, perm)?;
    Ok(run(ctx, operation, perm)?)
}

fn resolve(ctx: &Context, args: Platform, perm: &RepoShared) -> CliResult<CommentOperation> {
    match args.cmd {
        Subcommands::List => Ok(CommentOperation::List),
        Subcommands::Archive { id } => {
            let matches: Vec<String> = ctx
                .db
                .get_cache()?
                .diff_comments()
                .list_unarchived()?
                .into_iter()
                .filter(|comment| comment.id.starts_with(&id))
                .map(|comment| comment.id)
                .collect();
            match matches.as_slice() {
                [] => Err(bad_input("No unarchived comment with this id")
                    .arg_name("<ID>")
                    .arg_value(id)
                    .hint("Use `but comment list` to see the ids of all comments")
                    .into()),
                [id] => Ok(CommentOperation::Archive { id: id.clone() }),
                _ => Err(bad_input("The id prefix matches more than one comment")
                    .arg_name("<ID>")
                    .arg_value(id)
                    .hint("Use more characters of the id shown by `but comment list`")
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
                    let id_map = IdMap::new_from_context(ctx, None, perm)?;
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
        CommentOperation::List => Ok(CommentOutcome::Listed(comments::comments_list_with_perm(
            ctx, perm,
        )?)),
        CommentOperation::Archive { id } => {
            let archived = comments::comment_archive(ctx, id.clone())?;
            anyhow::ensure!(archived, "Comment {id} was archived concurrently");
            Ok(CommentOutcome::Archived { id })
        }
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
