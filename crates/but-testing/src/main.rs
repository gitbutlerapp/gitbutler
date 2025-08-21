//! A debug-CLI for making `but`-crates functionality available in real-world repositories.
#![deny(rust_2018_idioms)]
use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Result, bail};
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
        args::Subcommands::AddProject {
            switch_to_workspace,
            path,
        } => command::project::add(
            data_dir(args.app_data_dir)?,
            path.to_owned(),
            switch_to_workspace.to_owned(),
        ),

        args::Subcommands::RemoveReference {
            permit_empty_stacks,
            keep_metadata,
            short_name,
        } => command::remove_reference(
            &args,
            short_name,
            but_workspace::branch::remove_reference::Options {
                avoid_anonymous_stacks: !permit_empty_stacks,
                keep_metadata: *keep_metadata,
            },
        ),
        args::Subcommands::CreateReference {
            above,
            below,
            short_name,
        } => command::create_reference(&args, short_name, above.as_deref(), below.as_deref()),
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
        args::Subcommands::WatchDb => command::watch_db(&args),
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
            extra_target,
            no_debug_workspace,
            no_dot,
        } => command::graph(
            &args,
            ref_name.as_deref(),
            *no_open,
            limit.flatten(),
            limit_extension.clone(),
            extra_target.as_deref(),
            *hard_limit,
            *debug,
            *no_debug_workspace,
            *no_dot,
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
                args.v3,
            ),
            (None, Some(id)) => command::stacks::branches(*id, &args.current_dir, args.json),
            (None, None) => {
                bail!(
                    "You must provide a stack ID to list branches. Use `--branch-name` to create a new branch."
                )
            }
        },
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

pub fn data_dir(app_data_dir: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let path = if let Some(dir) = app_data_dir {
        std::fs::create_dir_all(&dir).context("Failed to assure the designated data-dir exists")?;
        dir
    } else {
        dirs_next::data_dir()
            .map(|dir| dir.join("com.gitbutler.app"))
            .context("no data-directory available on this platform")?
    };
    if !path.is_dir() {
        bail!("Path '{}' must be a valid directory", path.display());
    }
    Ok(path)
}
