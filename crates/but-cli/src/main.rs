//! A debug-CLI for making `but`-crates functionality available in real-world repositories.
#![deny(rust_2018_idioms)]
use anyhow::{Result, bail};
use command::parse_diff_spec;

mod args;
use crate::command::{RepositoryOpenMode, repo_and_maybe_project};
use args::Args;

mod command;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    if args.trace {
        trace::init()?;
    }
    let _op_span = tracing::info_span!("cli-op").entered();

    match &args.cmd {
        args::Subcommands::DiscardChange {
            hunk_indices,
            hunk_headers,
            current_path,
            previous_path,
        } => command::discard_change(
            &args.current_dir,
            current_path,
            previous_path.as_deref(),
            if !hunk_indices.is_empty() {
                Some(command::discard_change::IndicesOrHeaders::Indices(
                    hunk_indices,
                ))
            } else if !hunk_headers.is_empty() {
                Some(command::discard_change::IndicesOrHeaders::Headers(
                    hunk_headers,
                ))
            } else {
                None
            },
        ),
        args::Subcommands::Commit {
            current_path,
            previous_path,
            hunk_headers,
            message,
            amend,
            parent,
            workspace_tip,
            stack_segment_ref,
            diff_spec,
        } => {
            let (repo, project) = repo_and_maybe_project(&args, RepositoryOpenMode::Merge)?;
            let diff_spec = parse_diff_spec(diff_spec)?;
            command::commit(
                repo,
                project,
                message.as_deref(),
                *amend,
                parent.as_deref(),
                stack_segment_ref.as_deref(),
                workspace_tip.as_deref(),
                current_path.as_deref(),
                previous_path.as_deref(),
                if !hunk_headers.is_empty() {
                    Some(hunk_headers)
                } else {
                    None
                },
                diff_spec,
                args.json,
            )
        }
        args::Subcommands::HunkDependency { simple } => {
            command::diff::locks(&args.current_dir, *simple, args.json)
        }
        args::Subcommands::Status {
            unified_diff,
            context_lines,
        } => command::diff::status(&args.current_dir, *unified_diff, *context_lines, args.json),
        args::Subcommands::CommitChanges {
            unified_diff,
            current_commit,
            previous_commit,
        } => command::diff::commit_changes(
            &args.current_dir,
            current_commit,
            previous_commit.as_deref(),
            *unified_diff,
        ),
        args::Subcommands::Stacks => command::stacks::list(&args.current_dir, args.json),
        args::Subcommands::StackDetails { id } => command::stacks::details(*id, &args.current_dir),
        args::Subcommands::StackBranches {
            id,
            branch_name,
            description,
        } => match (branch_name, id) {
            (Some(branch_name), maybe_id) => command::stacks::create_branch(
                *maybe_id,
                branch_name,
                description.as_deref(),
                &args.current_dir,
                args.json,
            ),
            (None, Some(id)) => command::stacks::branches(*id, &args.current_dir, args.json),
            (None, None) => {
                bail!(
                    "You must provide a stack ID to list branches. Use `--branch-name` to create a new branch."
                )
            }
        },
        args::Subcommands::StackBranchCommits { id, name } => {
            command::stacks::branch_commits(*id, name, &args.current_dir, args.json)
        }
    }
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    pub fn init() -> anyhow::Result<()> {
        tracing_subscriber::registry()
            .with(
                tracing_forest::ForestLayer::from(
                    tracing_forest::printer::PrettyPrinter::new().writer(std::io::stderr),
                )
                .with_filter(LevelFilter::DEBUG),
            )
            .init();
        Ok(())
    }
}
