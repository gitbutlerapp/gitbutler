//! Implementation of the `but open` command.

use but_ctx::Context;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{BranchOrCommit, Purpose},
        open::Platform,
    },
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

/// What the link selects once the app is open.
enum Selection {
    /// The workspace, when no target was given.
    Workspace,
    /// A branch, by its full ref name.
    Branch(String),
    /// A commit, by change ID where it has one — that survives amending and
    /// rebasing — and by object ID where it does not.
    Commit {
        change_id: Option<String>,
        commit_id: gix::ObjectId,
    },
}

struct OpenOperation {
    project: String,
    selection: Selection,
    print_only: bool,
}

pub fn open(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<OpenOutcome> {
    let operation = resolve(ctx, args)?;
    Ok(run(operation)?)
}

fn resolve(ctx: &mut Context, args: Platform) -> CliResult<OpenOperation> {
    let Platform { target, print } = args;

    let guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let repo = ctx.repo.get()?;
    let project = but_project_handle::ProjectHandle::from_path(repo.git_dir())?.to_string();

    let selection = match target {
        None => Selection::Workspace,
        Some(target) => match target
            .resolve_in_workspace(&repo, &id_map, Purpose::Target, None)?
            .into_branch_or_commit()?
        {
            // The app addresses branches by full ref name, as its own codec
            // writes them; a short name would never match.
            BranchOrCommit::Branch(branch) => Selection::Branch(
                branch
                    .resolve_existing_local_branch(&repo)?
                    .as_bstr()
                    .to_string(),
            ),
            BranchOrCommit::Commit(commit) => Selection::Commit {
                change_id: commit.change_id.map(|id| id.to_string()),
                commit_id: commit.commit_id,
            },
        },
    };

    Ok(OpenOperation {
        project,
        selection,
        print_only: print,
    })
}

fn run(operation: OpenOperation) -> anyhow::Result<OpenOutcome> {
    let OpenOperation {
        project,
        selection,
        print_only,
    } = operation;

    // The app's own address space, as `apps/lite/ui/src/cursor-url.ts` writes
    // it: the stacks cursor names what the workspace page has selected.
    let mut url = format!("but://app/project/{project}/workspace");
    let cursor = match selection {
        Selection::Workspace => None,
        Selection::Branch(ref_name) => Some(format!("branch:{ref_name}")),
        Selection::Commit {
            change_id: Some(change_id),
            ..
        } => Some(format!("change:{change_id}")),
        Selection::Commit { commit_id, .. } => Some(format!("commit:{commit_id}")),
    };
    if let Some(cursor) = cursor {
        url.push_str("?stacks=");
        url.push_str(&cursor);
    }

    if !print_only {
        but_api::open::open_url(url.clone())?;
    }

    Ok(OpenOutcome {
        url,
        opened: !print_only,
    })
}

#[must_use]
pub struct OpenOutcome {
    url: String,
    opened: bool,
}

impl CliOutputHuman for OpenOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self { url, opened } = self;

        if opened {
            writeln!(out, "Opening {url}")?;
        } else {
            writeln!(out, "{url}")?;
        }

        Ok(())
    }
}

impl CliOutput for OpenOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        struct Output {
            url: String,
            opened: bool,
        }

        let Self { url, opened } = self;

        Output { url, opened }
    }
}
