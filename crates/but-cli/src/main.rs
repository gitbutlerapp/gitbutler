//! A debug-CLI for making `but`-crates functionality available in real-world repositories.
use anyhow::Result;

mod args;
use crate::command::{repo_and_maybe_project, RepositoryOpenMode};
use args::Args;

mod command;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    if args.trace {
        trace::init()?;
    }
    let _op_span = tracing::info_span!("cli-op").entered();

    match &args.cmd {
        args::Subcommands::Commit {
            message,
            amend,
            parent,
        } => {
            let (repo, project) = repo_and_maybe_project(&args, RepositoryOpenMode::Merge)?;
            command::commit(repo, project, message.as_deref(), *amend, parent.as_deref())
        }
        args::Subcommands::HunkDependency => command::diff::locks(&args.current_dir),
        args::Subcommands::Status { unified_diff } => {
            command::diff::status(&args.current_dir, *unified_diff)
        }
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
        args::Subcommands::Stacks => command::stacks::list(&args.current_dir),
        args::Subcommands::StackBranches { id } => command::stacks::branches(id, &args.current_dir),
        args::Subcommands::StackBranchCommits { id, name } => {
            command::stacks::branch_commits(id, name, &args.current_dir)
        }
    }
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

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
