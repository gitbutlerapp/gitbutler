use base::Subcommands;
use but_ctx::Context;
use colored::Colorize;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::{Conflicted, Empty, Integrated, SaflyUpdatable},
    StackStatuses::{UpToDate, UpdatesRequired},
};

use crate::{args::base, utils::OutputChannel};

pub async fn handle(
    cmd: Subcommands,
    ctx: &Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    match cmd {
        Subcommands::Fetch => {
            but_api::legacy::virtual_branches::fetch_from_remotes(
                ctx.legacy_project.id,
                Some("auto".to_string()),
            )
            .ok();
            Ok(())
        }
        Subcommands::Check => {
            if let Some(out) = out.for_human() {
                writeln!(out, "🔍 Checking base branch status...")?;
                let base_branch = but_api::legacy::virtual_branches::fetch_from_remotes(
                    ctx.legacy_project.id,
                    Some("auto".to_string()),
                )?;
                writeln!(out, "\n📍 Base branch:\t\t{}", base_branch.branch_name)?;
                writeln!(
                    out,
                    "⏫ Upstream commits:\t{} new commits on {}\n",
                    base_branch.behind, base_branch.branch_name
                )?;
                let commits = base_branch.recent_commits.iter().take(3);
                for commit in commits {
                    writeln!(
                        out,
                        "\t{} {}",
                        &commit.id[..7],
                        &commit
                            .description
                            .to_string()
                            .replace('\n', " ")
                            .chars()
                            .take(72)
                            .collect::<String>()
                    )?;
                }
                let hidden_commits = base_branch.behind.saturating_sub(3);
                if hidden_commits > 0 {
                    writeln!(
                        out,
                        "\t... ({hidden_commits} more - run `but base check --all` to see all)"
                    )?;
                }

                let status = but_api::legacy::virtual_branches::upstream_integration_statuses(
                    ctx.legacy_project.id,
                    None,
                )
                .await?;

                match status {
                    UpToDate => {
                        writeln!(out, "\n✅ Everything is up to date")?;
                    }
                    UpdatesRequired {
                        worktree_conflicts,
                        statuses,
                    } => {
                        if !worktree_conflicts.is_empty() {
                            writeln!(
                                out,
                                "\n❗️ There are uncommitted changes in the worktree that may conflict with the updates."
                            )?;
                        }
                        if !statuses.is_empty() {
                            writeln!(out, "\n{}", "Active Branch Status".bold())?;
                            for (_id, status) in statuses {
                                for bs in status.branch_statuses {
                                    let status_icon = match bs.status {
                                        SaflyUpdatable => "✅".to_string(),
                                        Integrated => "🔄".to_string(),
                                        Conflicted { rebasable } => {
                                            if rebasable {
                                                "⚠️".to_string()
                                            } else {
                                                "❗️".to_string()
                                            }
                                        }
                                        Empty => "✅".to_string(),
                                    };
                                    let status_text = match bs.status {
                                        SaflyUpdatable => "Updatable".green(),
                                        Integrated => "Integrated".blue(),
                                        Conflicted { rebasable } => {
                                            if rebasable {
                                                "Conflicted (Rebasable)".yellow()
                                            } else {
                                                "Conflicted (Not Rebasable)".red()
                                            }
                                        }
                                        Empty => "Nothing to do".normal(),
                                    };
                                    writeln!(
                                        out,
                                        "\n{} {} ({})",
                                        status_icon, bs.name, status_text
                                    )?;
                                }
                            }
                        }
                    }
                }
                writeln!(out, "\nRun `but pull` to update your branches")?;
            }
            Ok(())
        }
    }
}
