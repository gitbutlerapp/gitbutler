//! A debug-CLI for making `but`-crates functionality available in real-world repositories.
#![deny(rust_2018_idioms)]
use std::str::FromStr;

use anyhow::{Result, bail};
use but_workspace::HunkHeader;
use command::parse_diff_spec;
use gix::bstr::BString;

mod args;
use crate::command::{RepositoryOpenMode, repo_and_maybe_project};
use args::Args;

mod command;

// NOTE: Just for the `watch` function which unfortunately sends events through async channels.
#[tokio::main]
async fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    if args.trace {
        trace::init()?;
    }
    let _op_span = tracing::info_span!("cli-op").entered();

    match &args.cmd {
        args::Subcommands::OpMode => command::operating_mode(&args),
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
        args::Subcommands::Watch => command::watch(&args).await,
        args::Subcommands::Stacks { workspace_only } => {
            command::stacks::list(&args.current_dir, args.json, args.v3, *workspace_only)
        }
        args::Subcommands::BranchDetails { ref_name } => {
            command::stacks::branch_details(ref_name, &args.current_dir, args.v3)
        }
        args::Subcommands::StackDetails { id } => {
            command::stacks::details(*id, &args.current_dir, args.v3)
        }
        args::Subcommands::RefInfo {
            ref_name,
            expensive,
        } => command::ref_info(&args, ref_name.as_deref(), *expensive),
        args::Subcommands::Graph {
            ref_name,
            no_open,
            limit,
            limit_extension,
            hard_limit,
            debug,
        } => command::graph(
            &args,
            ref_name.as_deref(),
            *no_open,
            limit.flatten(),
            limit_extension.clone(),
            *hard_limit,
            *debug,
        ),
        args::Subcommands::HunkAssignments => {
            command::assignment::hunk_assignments(&args.current_dir, args.json)
        }
        args::Subcommands::AssignHunk {
            path,
            stack_id,
            old_start,
            old_lines,
            new_start,
            new_lines,
        } => {
            let assignment = but_hunk_assignment::HunkAssignmentRequest {
                path_bytes: BString::from_str(path)?,
                stack_id: Some(*stack_id),
                hunk_header: Some(HunkHeader {
                    old_start: *old_start,
                    old_lines: *old_lines,
                    new_start: *new_start,
                    new_lines: *new_lines,
                }),
            };
            command::assignment::assign_hunk(&args.current_dir, args.json, assignment)
        }
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
