use but_core::RepositoryExt;
use but_graph::VirtualBranchesTomlMetadata;
use but_settings::AppSettings;
use but_workspace::{
    StackId, StacksFilter,
    ui::{StackDetails, StackEntry},
};
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_repo::RepoCommands;

use crate::id::CliId;

pub(crate) fn commit_graph(project: &Project, json: bool) -> anyhow::Result<()> {
    let ctx = &mut CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;
    but_rules::process_rules(ctx).ok(); // TODO: this is doing double work (dependencies can be reused)

    // Get remote information for URL generation
    let remotes = project.remotes().unwrap_or_default();
    let primary_remote = remotes.first();
    let gerrit_mode = ctx
        .gix_repo()?
        .git_settings()?
        .gitbutler_gerrit_mode
        .unwrap_or(false);

    let stacks = stacks(ctx)?
        .iter()
        .filter_map(|s| s.id.map(|id| stack_details(ctx, id).map(|d| (id, d))))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    if json {
        return output_json(stacks.into_iter().map(|(_, stack)| stack).collect());
    }

    let mut nesting = 0;
    for (i, (stack_id, stack)) in stacks.iter().enumerate() {
        let marked = crate::mark::stack_marked(ctx, *stack_id).unwrap_or_default();
        let mut mark = if marked {
            Some("◀ Marked ▶".red().bold())
        } else {
            None
        };
        let mut second_consecutive = false;
        let mut stacked = false;
        for branch in stack.branch_details.iter() {
            let line = if second_consecutive {
                if branch.upstream_commits.is_empty() {
                    '├'
                } else {
                    '╭'
                }
            } else {
                '╭'
            };
            second_consecutive = branch.upstream_commits.is_empty();
            let extra_space = if !branch.upstream_commits.is_empty() {
                if stacked { "│ " } else { "  " }
            } else {
                ""
            };
            let id = CliId::branch(&branch.name.to_string())
                .to_string()
                .underline()
                .blue();
            println!(
                "{}{}{} [{}] {} {}",
                "│ ".repeat(nesting),
                extra_space,
                line,
                branch.name.to_string().green().bold(),
                id,
                mark.clone().unwrap_or_default()
            );
            mark = None; // show this on the first branch in the stack

            // Add URL for branch if remote is available
            if let Some(remote) = primary_remote {
                if let Some(remote_url) = &remote.url {
                    if let Some(branch_url) = crate::url_utils::generate_branch_url(remote_url, &branch.name.to_string(), gerrit_mode) {
                        let url_prefix = if gerrit_mode { "Changes:" } else { "Branch:" };
                        println!(
                            "{}{}  {} {}",
                            "│ ".repeat(nesting),
                            extra_space,
                            url_prefix.dimmed(),
                            branch_url.cyan().underline()
                        );
                    }
                }
            }
            for (j, commit) in branch.upstream_commits.iter().enumerate() {
                let time_string = chrono::DateTime::from_timestamp_millis(commit.created_at as i64)
                    .ok_or(anyhow::anyhow!("Could not parse timestamp"))?
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
                let state_str = "{upstream}";
                let extra_space = if stacked { "│ " } else { "  " };
                println!(
                    "{}{}● {}{} {} {} {}",
                    "│ ".repeat(nesting),
                    extra_space,
                    &commit.id.to_string()[..2].blue().underline(),
                    &commit.id.to_string()[2..7].blue(),
                    state_str.yellow(),
                    commit.author.name,
                    time_string.dimmed(),
                );
                println!(
                    "{}{}┊ {}",
                    "│ ".repeat(nesting),
                    extra_space,
                    commit.message.to_string().lines().next().unwrap_or("")
                );

                // Add URL for commit if remote is available
                if let Some(remote) = primary_remote {
                    if let Some(remote_url) = &remote.url {
                        if let Some(commit_url) = crate::url_utils::generate_commit_url(remote_url, &commit.id.to_string(), gerrit_mode) {
                            let url_prefix = if gerrit_mode { "Change:" } else { "Commit:" };
                            println!(
                                "{}{}┊ {} {}",
                                "│ ".repeat(nesting),
                                extra_space,
                                url_prefix.dimmed(),
                                commit_url.cyan().underline()
                            );
                        }
                    }
                }
                let bend = if stacked { "├" } else { "╭" };
                if j == branch.upstream_commits.len() - 1 {
                    println!("{}{}─╯", "│ ".repeat(nesting), bend);
                } else {
                    println!("{}  ┊", "│ ".repeat(nesting));
                }
            }
            for commit in branch.commits.iter() {
                let marked =
                    crate::mark::commit_marked(ctx, commit.id.to_string()).unwrap_or_default();
                let mark = if marked {
                    Some("◀ Marked ▶".red().bold())
                } else {
                    None
                };
                let state_str = match commit.state {
                    but_workspace::ui::CommitState::LocalOnly => "{local}".normal(),
                    but_workspace::ui::CommitState::LocalAndRemote(_) => "{pushed}".cyan(),
                    but_workspace::ui::CommitState::Integrated => "{integrated}".purple(),
                };
                let conflicted_str = if commit.has_conflicts {
                    "{conflicted}".red()
                } else {
                    "".normal()
                };
                let time_string = chrono::DateTime::from_timestamp_millis(commit.created_at as i64)
                    .ok_or(anyhow::anyhow!("Could not parse timestamp"))?
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
                println!(
                    "{}● {}{} {} {} {} {} {}",
                    "│ ".repeat(nesting),
                    &commit.id.to_string()[..2].blue().underline(),
                    &commit.id.to_string()[2..7].blue(),
                    state_str,
                    conflicted_str,
                    commit.author.name,
                    time_string.dimmed(),
                    mark.clone().unwrap_or_default()
                );
                println!(
                    "{}│ {}",
                    "│ ".repeat(nesting),
                    commit.message.to_string().lines().next().unwrap_or("")
                );

                // Add URL for commit if remote is available
                if let Some(remote) = primary_remote {
                    if let Some(remote_url) = &remote.url {
                        if let Some(commit_url) = crate::url_utils::generate_commit_url(remote_url, &commit.id.to_string(), gerrit_mode) {
                            let url_prefix = if gerrit_mode { "Change:" } else { "Commit:" };
                            println!(
                                "{}│ {} {}",
                                "│ ".repeat(nesting),
                                url_prefix.dimmed(),
                                commit_url.cyan().underline()
                            );
                        }
                    }
                }
                if i == stacks.len() - 1 {
                    if nesting == 0 {
                        println!("│");
                    }
                } else {
                    println!("{}│", "│ ".repeat(nesting));
                }
                stacked = true;
            }
        }
        nesting += 1;
    }
    if nesting > 0 {
        for _ in (0..nesting - 1).rev() {
            if nesting == 1 {
                println!("└─╯");
            } else {
                let prefix = "│ ".repeat(nesting - 2);
                println!("{prefix}├─╯");
            }
            nesting -= 1;
        }
    }

    let common_merge_base = gitbutler_stack::VirtualBranchesHandle::new(ctx.project().gb_dir())
        .get_default_target()?
        .sha
        .to_string()[..7]
        .to_string();
    println!("● {common_merge_base} (base)");

    Ok(())
}

pub(crate) fn all_commits(ctx: &CommandContext) -> anyhow::Result<Vec<CliId>> {
    let stacks = stacks(ctx)?
        .iter()
        .filter_map(|s| s.id.map(|id| stack_details(ctx, id)))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    for stack in stacks {
        for branch in &stack.branch_details {
            for commit in &branch.upstream_commits {
                matches.push(CliId::commit(commit.id));
            }
            for commit in &branch.commits {
                matches.push(CliId::commit(commit.id));
            }
        }
    }
    Ok(matches)
}

pub(crate) fn stacks(ctx: &CommandContext) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    if ctx.app_settings().feature_flags.ws3 {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::stacks_v3(&repo, &meta, StacksFilter::default(), None)
    } else {
        but_workspace::stacks(ctx, &ctx.project().gb_dir(), &repo, StacksFilter::default())
    }
}

pub(crate) fn stack_details(
    ctx: &CommandContext,
    stack_id: StackId,
) -> anyhow::Result<StackDetails> {
    if ctx.app_settings().feature_flags.ws3 {
        let repo = ctx.gix_repo_for_merging_non_persisting()?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::stack_details_v3(Some(stack_id), &repo, &meta)
    } else {
        but_workspace::stack_details(&ctx.project().gb_dir(), stack_id, ctx)
    }
}

fn output_json(stacks: Vec<but_workspace::ui::StackDetails>) -> anyhow::Result<()> {
    let json_output = serde_json::to_string_pretty(&stacks)?;
    println!("{json_output}");
    Ok(())
}
