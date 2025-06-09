use but_graph::VirtualBranchesTomlMetadata;
use but_settings::AppSettings;
use but_workspace::{
    StackId, StacksFilter,
    ui::{StackDetails, StackEntry},
};
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use std::path::Path;

pub(crate) fn commit_graph(repo_path: &Path, _json: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::default())?;
    let stacks = stacks(ctx)?
        .iter()
        .map(|s| stack_details(ctx, s.id))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut nesting = 0;
    for (i, stack) in stacks.iter().enumerate() {
        for branch in stack.branch_details.iter() {
            println!(
                "{}  [{}]",
                "│ ".repeat(nesting),
                branch.name.to_string().blue().underline()
            );
            for (j, commit) in branch.upstream_commits.iter().enumerate() {
                let time_string = chrono::DateTime::from_timestamp_millis(commit.created_at as i64)
                    .ok_or(anyhow::anyhow!("Could not parse timestamp"))?
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
                let state_str = "{upstream}";
                println!(
                    "{}  ● {} {} {} {}",
                    "│ ".repeat(nesting),
                    &commit.id.to_string()[..7].green(),
                    state_str.yellow(),
                    commit.author.name,
                    time_string
                );
                println!(
                    "{}  ┊ {}",
                    "│ ".repeat(nesting),
                    commit.message.to_string().lines().next().unwrap_or("")
                );
                if j == branch.upstream_commits.len() - 1 {
                    println!("{}  ┴", "│ ".repeat(nesting));
                } else {
                    println!("{}  ┊", "│ ".repeat(nesting));
                }
            }
            for commit in branch.commits.iter() {
                let state_str = match commit.state {
                    but_workspace::ui::CommitState::LocalOnly => "{local}".normal(),
                    but_workspace::ui::CommitState::LocalAndRemote(_) => "{pushed}".blue(),
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
                    "{}● {} {} {} {} {}",
                    "│ ".repeat(nesting),
                    &commit.id.to_string()[..7].green(),
                    state_str,
                    conflicted_str,
                    commit.author.name,
                    time_string.dimmed()
                );
                println!(
                    "{}│ {}",
                    "│ ".repeat(nesting),
                    commit.message.to_string().lines().next().unwrap_or("")
                );
                if i == stacks.len() - 1 {
                    if nesting == 0 {
                        println!("│");
                    }
                } else {
                    println!("{}│", "│ ".repeat(nesting));
                }
            }
            if !branch.commits.is_empty() {
                nesting += 1;
            }
        }
    }
    if nesting > 0 {
        for _ in (0..nesting - 1).rev() {
            if nesting == 1 {
                println!("└─╯");
            } else {
                let prefix = "│ ".repeat(nesting - 2);
                println!("{}├─╯", prefix);
            }
            nesting -= 1;
        }
    }

    let common_merge_base = gitbutler_stack::VirtualBranchesHandle::new(ctx.project().gb_dir())
        .get_default_target()?
        .sha
        .to_string()[..7]
        .to_string();
    println!("● {} (base)", common_merge_base);

    Ok(())
}

pub(crate) fn stacks(ctx: &CommandContext) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    if ctx.app_settings().feature_flags.ws3 {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::stacks_v3(&repo, &meta, StacksFilter::default())
    } else {
        but_workspace::stacks(ctx, &ctx.project().gb_dir(), &repo, StacksFilter::default())
    }
}

fn stack_details(ctx: &CommandContext, stack_id: StackId) -> anyhow::Result<StackDetails> {
    if ctx.app_settings().feature_flags.ws3 {
        let repo = ctx.gix_repo_for_merging_non_persisting()?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::stack_details_v3(stack_id, &repo, &meta)
    } else {
        but_workspace::stack_details(&ctx.project().gb_dir(), stack_id, ctx)
    }
}
