use std::collections::HashMap;

use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt};
use colored::Colorize;
use gitbutler_branch_actions::BranchListingFilter;

use crate::utils::OutputChannel;

#[expect(clippy::too_many_arguments)]
pub fn list(
    ctx: &mut but_ctx::Context,
    local: bool,
    remote: bool,
    all: bool,
    ahead: bool,
    review: bool,
    filter: Option<String>,
    out: &mut OutputChannel,
    check_merge: bool,
    show_empty: bool,
) -> Result<(), anyhow::Error> {
    let listing_filter = if local {
        Some(BranchListingFilter {
            local: Some(true),
            ..Default::default()
        })
    } else {
        None
    };

    let branch_review_map = if review {
        crate::command::legacy::forge::review::get_review_map(
            ctx,
            Some(but_forge::CacheConfig::CacheOnly),
        )?
    } else {
        HashMap::new()
    };

    let mut applied_stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Filter out empty branches unless --empty is requested
    let target_oid_for_filter: Option<git2::Oid> = if !show_empty {
        get_target_oid(ctx).ok()
    } else {
        None
    };

    if let Some(target_oid) = target_oid_for_filter {
        // For applied stacks: remove heads that have no commits on them.
        // heads[0] is topmost; head[i] is empty if its tip == heads[i+1].tip (non-last)
        // or tip == target_oid (last/bottommost head).
        for stack in &mut applied_stacks {
            let tips: Vec<git2::Oid> = stack.heads.iter().map(|h| h.tip.to_git2()).collect();
            let non_empty: Vec<bool> = tips
                .iter()
                .enumerate()
                .map(|(i, &tip)| {
                    let next_tip = tips.get(i + 1).copied().unwrap_or(target_oid);
                    tip != next_tip
                })
                .collect();
            let mut i = 0;
            stack.heads.retain(|_| {
                let keep = non_empty[i];
                i += 1;
                keep
            });
        }
        applied_stacks.retain(|stack| !stack.heads.is_empty());
    }

    // Apply name filter to applied stacks if provided
    if let Some(ref filter_str) = filter {
        let filter_lower = filter_str.to_lowercase();
        applied_stacks.retain(|stack| {
            stack
                .heads
                .iter()
                .any(|head| head.name.to_string().to_lowercase().contains(&filter_lower))
        });
        // Also filter the heads within each stack
        for stack in &mut applied_stacks {
            stack
                .heads
                .retain(|head| head.name.to_string().to_lowercase().contains(&filter_lower));
        }
    }

    let mut branches = but_api::legacy::virtual_branches::list_branches(ctx, listing_filter)?;

    // Filter out branches that are part of applied stacks
    let applied_stack_ids: Vec<_> = applied_stacks.iter().filter_map(|s| s.id).collect();
    branches.retain(|branch| {
        if let Some(stack_ref) = &branch.stack {
            !applied_stack_ids.contains(&stack_ref.id)
        } else {
            true
        }
    });

    // Apply local/remote filtering
    if local {
        branches.retain(|branch| branch.has_local);
    } else if remote {
        branches.retain(|branch| !branch.has_local);
    }

    // Apply name filter if provided
    if let Some(ref filter_str) = filter {
        let filter_lower = filter_str.to_lowercase();
        branches.retain(|branch| {
            branch
                .name
                .to_string()
                .to_lowercase()
                .contains(&filter_lower)
        });
    }

    // Filter out branches with no commits ahead of target unless --empty is requested.
    // A branch has 0 commits ahead when its tip equals the target, or when it is an
    // ancestor of the target (all its commits are already in the target).
    if let Some(target_oid) = target_oid_for_filter {
        let git2_repo = ctx.git2_repo.get()?;
        branches.retain(|branch| {
            if branch.head == target_oid {
                return false;
            }
            // graph_descendant_of(a, b) = "is a descended from b?"
            // So graph_descendant_of(target, branch.head) = "is branch.head an ancestor of target?"
            // If true, all branch commits are already in the target → 0 commits ahead.
            !git2_repo
                .graph_descendant_of(target_oid, branch.head)
                .unwrap_or(false)
        });
        drop(git2_repo);
    }

    // Filter out dependabot branches unless --all is specified
    if !all {
        branches.retain(|branch| {
            !branch
                .name
                .to_string()
                .to_lowercase()
                .contains("dependabot")
        });
    }

    // Sort all branches by last commit date (most recent first)
    branches.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    // Limit branches unless --all flag is set
    let (branches_to_show, more_count) = if all {
        (branches, 0)
    } else {
        const MAX_BRANCHES: usize = 20;
        if branches.len() > MAX_BRANCHES {
            let remaining = branches.len() - MAX_BRANCHES;
            (branches.into_iter().take(MAX_BRANCHES).collect(), remaining)
        } else {
            (branches, 0)
        }
    };

    // Calculate commits ahead if requested
    let commits_ahead_map: Option<HashMap<String, usize>> = if ahead {
        Some(calculate_commits_ahead(ctx, &branches_to_show)?)
    } else {
        None
    };

    // Check merge status if requested
    let merge_status_map: Option<HashMap<String, bool>> = if check_merge {
        Some(check_branches_merge_cleanly(
            ctx,
            &applied_stacks,
            &branches_to_show,
        )?)
    } else {
        None
    };

    if let Some(out) = out.for_json() {
        output_json(
            &applied_stacks,
            &branches_to_show,
            more_count,
            &branch_review_map,
            commits_ahead_map.as_ref(),
            merge_status_map.as_ref(),
            ctx,
            out,
        )?;
    } else if let Some(out) = out.for_human() {
        // Print applied branches section with header
        if !applied_stacks.is_empty() {
            writeln!(out, "{}", "Applied Branches".green())?;
            print_applied_branches_table(
                &applied_stacks,
                &branch_review_map,
                ctx,
                commits_ahead_map.as_ref(),
                merge_status_map.as_ref(),
                out,
            )?;
        }

        // Print unapplied branches section with header
        if !branches_to_show.is_empty() {
            if !applied_stacks.is_empty() {
                writeln!(out)?;
            }
            writeln!(out, "Unapplied Branches")?;
            print_branches_table(
                &branches_to_show,
                &branch_review_map,
                commits_ahead_map.as_ref(),
                merge_status_map.as_ref(),
                out,
            )?;
        }

        if more_count > 0 {
            writeln!(
                out,
                "\n... and {more_count} more branches (use --all to show all)"
            )?;
        }
    }
    Ok(())
}

#[expect(clippy::too_many_arguments)]
fn output_json(
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    branches: &[gitbutler_branch_actions::BranchListing],
    more_count: usize,
    branch_review_map: &HashMap<String, Vec<but_forge::ForgeReview>>,
    commits_ahead_map: Option<&HashMap<String, usize>>,
    merge_status_map: Option<&HashMap<String, bool>>,
    ctx: &Context,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    use crate::command::legacy::branch::json::*;

    // Open repo to get commit information
    let repo = &*ctx.git2_repo.get()?;

    let applied_stacks_output: Vec<StackOutput> = applied_stacks
        .iter()
        .map(|stack| {
            let heads: Vec<BranchHeadOutput> = stack
                .heads
                .iter()
                .map(|head| {
                    let reviews = get_reviews_json(&head.name.to_string(), branch_review_map);
                    let commits_ahead =
                        commits_ahead_map.and_then(|map| map.get(&head.name.to_string()).copied());
                    let merges_cleanly =
                        merge_status_map.and_then(|map| map.get(&head.name.to_string()).copied());

                    // Get commit information
                    let (last_commit_at, last_author) = match repo.find_commit(head.tip.to_git2()) {
                        Ok(commit) => {
                            let author = commit.author();
                            let timestamp_ms = (commit.time().seconds() * 1000) as u128;
                            let author_output = AuthorOutput {
                                name: author.name().map(|s| s.to_string()),
                                email: author.email().map(|s| s.to_string()),
                            };
                            (timestamp_ms, author_output)
                        }
                        Err(_) => (
                            0,
                            AuthorOutput {
                                name: None,
                                email: None,
                            },
                        ),
                    };

                    BranchHeadOutput {
                        name: head.name.to_string(),
                        reviews,
                        last_commit_at,
                        commits_ahead,
                        last_author,
                        merges_cleanly,
                    }
                })
                .collect();

            StackOutput {
                id: stack.id.map(|id| id.to_string()),
                heads,
            }
        })
        .collect();

    let branches_output: Vec<BranchOutput> = branches
        .iter()
        .map(|branch| {
            let reviews = get_reviews_json(&branch.name.to_string(), branch_review_map);
            let commits_ahead =
                commits_ahead_map.and_then(|map| map.get(&branch.name.to_string()).copied());
            let merges_cleanly =
                merge_status_map.and_then(|map| map.get(&branch.name.to_string()).copied());
            BranchOutput {
                name: branch.name.to_string(),
                reviews,
                has_local: branch.has_local,
                last_commit_at: branch.updated_at,
                commits_ahead,
                last_author: AuthorOutput {
                    name: branch.last_commiter.name.as_ref().map(|n| n.to_string()),
                    email: branch.last_commiter.email.as_ref().map(|e| e.to_string()),
                },
                merges_cleanly,
            }
        })
        .collect();

    let output = BranchListOutput {
        applied_stacks: applied_stacks_output,
        branches: branches_output,
        more_branches: if more_count > 0 {
            Some(more_count)
        } else {
            None
        },
    };

    out.write_value(output)?;
    Ok(())
}

fn get_reviews_json(
    branch_name: &str,
    branch_review_map: &HashMap<String, Vec<but_forge::ForgeReview>>,
) -> Vec<super::json::ReviewOutput> {
    if let Some(reviews) = branch_review_map.get(branch_name) {
        reviews
            .iter()
            .map(|r| super::json::ReviewOutput {
                number: r.number as u64,
                url: r.html_url.clone(),
            })
            .collect()
    } else {
        vec![]
    }
}

// TODO(perf): re-write this to use a graph that includes all branches from the listing, needs a listing that uses
//             the graph.
fn check_branches_merge_cleanly(
    ctx: &Context,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    branches: &[gitbutler_branch_actions::BranchListing],
) -> Result<HashMap<String, bool>, anyhow::Error> {
    use but_core::RepositoryExt;

    let git2_repo = &*ctx.git2_repo.get()?;
    let repo = ctx.clone_repo_for_merging_non_persisting()?;

    let stack = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
    let target = stack.get_default_target()?;

    // Try to find the remote tracking branch (e.g., refs/remotes/origin/master)
    let target_ref_name = format!(
        "refs/remotes/{}/{}",
        target.branch.remote(),
        target.branch.branch()
    );
    let target_commit = match repo.find_reference(&target_ref_name) {
        Ok(reference) => {
            let target_oid = reference.id();
            git2_repo.find_commit(target_oid.to_git2())?
        }
        Err(_) => {
            // Fallback to the stored SHA if remote branch doesn't exist
            git2_repo.find_commit(target.sha.to_git2())?
        }
    };

    let mut result = HashMap::new();

    // Check applied stacks
    for stack_entry in applied_stacks {
        for head in &stack_entry.heads {
            let branch_name = head.name.to_string();
            match git2_repo.find_commit(head.tip.to_git2()) {
                Ok(branch_commit) => {
                    // Find merge base
                    match git2_repo.merge_base(target_commit.id(), branch_commit.id()) {
                        Ok(merge_base) => {
                            let merge_base_commit = git2_repo.find_commit(merge_base)?;

                            // Check if branch merges cleanly into target
                            let merges_cleanly = repo
                                .merges_cleanly(
                                    merge_base_commit.tree_id().to_gix(),
                                    target_commit.tree_id().to_gix(),
                                    branch_commit.tree_id().to_gix(),
                                )
                                .unwrap_or(false);

                            result.insert(branch_name, merges_cleanly);
                        }
                        Err(_) => {
                            // Can't find merge base, assume conflict
                            result.insert(branch_name, false);
                        }
                    }
                }
                Err(_) => {
                    // Can't find branch commit, skip
                }
            }
        }
    }

    // Check unapplied branches
    for branch in branches {
        let branch_name = branch.name.to_string();
        match git2_repo.find_commit(branch.head) {
            Ok(branch_commit) => {
                // Find merge base
                match git2_repo.merge_base(target_commit.id(), branch_commit.id()) {
                    Ok(merge_base) => {
                        let merge_base_commit = git2_repo.find_commit(merge_base)?;

                        // Check if branch merges cleanly into target
                        let merges_cleanly = repo
                            .merges_cleanly(
                                merge_base_commit.tree_id().to_gix(),
                                target_commit.tree_id().to_gix(),
                                branch_commit.tree_id().to_gix(),
                            )
                            .unwrap_or(false);

                        result.insert(branch_name, merges_cleanly);
                    }
                    Err(_) => {
                        // Can't find merge base, assume conflict
                        result.insert(branch_name, false);
                    }
                }
            }
            Err(_) => {
                // Can't find branch commit, skip
            }
        }
    }

    Ok(result)
}

fn calculate_commits_ahead(
    ctx: &Context,
    branches: &[gitbutler_branch_actions::BranchListing],
) -> Result<HashMap<String, usize>, anyhow::Error> {
    use gix::prelude::ObjectIdExt as _;

    let repo = ctx.repo.get()?;
    let stack = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
    let target = stack.get_default_target()?;

    // Try to find the remote tracking branch (e.g., refs/remotes/origin/master)
    let target_ref_name = format!(
        "refs/remotes/{}/{}",
        target.branch.remote(),
        target.branch.branch()
    );
    let target_oid = match repo.find_reference(&target_ref_name) {
        Ok(mut reference) => reference.peel_to_commit()?.id,
        Err(_) => {
            // Fallback to the stored SHA if remote branch doesn't exist
            target.sha
        }
    };

    let mut result = HashMap::new();

    for branch in branches {
        let branch_oid = branch.head.to_gix();

        // Count commits ahead using merge base
        let merge_base = match repo.merge_base(branch_oid, target_oid) {
            Ok(base) => base,
            Err(_) => continue, // Skip if no merge base found
        };

        // Walk from branch head to merge base
        let traversal = match branch_oid
            .attach(&repo)
            .ancestors()
            .with_hidden(Some(merge_base))
            .all()
        {
            Ok(t) => t,
            Err(_) => continue,
        };

        let count = traversal.filter_map(Result::ok).count();
        result.insert(branch.name.to_string(), count);
    }

    Ok(result)
}

fn format_date_for_display(timestamp_ms: u128) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    // Calculate days ago
    let age_ms = now.saturating_sub(timestamp_ms);
    let days_ago = (age_ms / (1000 * 60 * 60 * 24)) as i64;

    if days_ago == 0 {
        "today".to_string()
    } else if days_ago == 1 {
        "yesterday".to_string()
    } else if days_ago < 7 {
        format!("{days_ago}d ago")
    } else if days_ago < 30 {
        let weeks_ago = days_ago / 7;
        format!("{weeks_ago}w ago")
    } else if days_ago < 365 {
        let months_ago = days_ago / 30;
        format!("{months_ago}mo ago")
    } else {
        let years_ago = days_ago / 365;
        format!("{years_ago}y ago")
    }
}

fn print_applied_branches_table(
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    branch_review_map: &HashMap<String, Vec<but_forge::ForgeReview>>,
    ctx: &Context,
    commits_ahead_map: Option<&HashMap<String, usize>>,
    merge_status_map: Option<&HashMap<String, bool>>,
    out: &mut (dyn std::fmt::Write + 'static),
) -> Result<(), anyhow::Error> {
    use crate::tui::{Table, table::Cell};

    if applied_stacks.is_empty() {
        return Ok(());
    }

    // Open repo to get commit information
    let repo = &*ctx.git2_repo.get()?;

    // Define column headers with fixed widths
    // BRANCH is marked no_truncate so the name is always fully visible.
    // AUTHOR is flexible and will shrink first when space is tight.
    let headers = vec![
        Cell::new("TYPE").with_width(7),
        Cell::new("BRANCH").no_truncate(),
        Cell::new("AHEAD").with_width(6),
        Cell::new("DATE").with_width(10),
        Cell::new("AUTHOR"),
    ];

    let mut table = Table::new(headers);

    for stack in applied_stacks {
        let first_branch = stack.heads.first();
        let last_branch = stack.heads.last();
        for branch in &stack.heads {
            let is_single_branch = stack.heads.len() == 1;

            // Get commit information
            let (author_str, date_str) = match repo.find_commit(branch.tip.to_git2()) {
                Ok(commit) => {
                    let author = commit.author();
                    let author_str = author
                        .name()
                        .or_else(|| author.email())
                        .unwrap_or("Unknown")
                        .to_string();
                    let timestamp_ms = (commit.time().seconds() * 1000) as u128;
                    let date_str = format_date_for_display(timestamp_ms);
                    (author_str, date_str)
                }
                Err(_) => ("Unknown".to_string(), "unknown".to_string()),
            };

            // Type column
            let type_str = "active".green().to_string();

            // Ahead column
            let ahead_str = commits_ahead_map
                .and_then(|map| map.get(&branch.name.to_string()))
                .map(|count| format!("↑{count}").bright_cyan().to_string())
                .unwrap_or_default();

            // Merge status indicator
            let merge_status_str = if let Some(map) = merge_status_map {
                match map.get(&branch.name.to_string()) {
                    Some(true) => "✓ ".green().to_string(),
                    Some(false) => "✗ ".red().to_string(),
                    None => String::new(),
                }
            } else {
                String::new()
            };

            // Branch name with tree prefix and merge status
            let branch_with_prefix = if is_single_branch {
                format!("{}{}{}", merge_status_str, "*".green(), branch.name)
            } else if let (Some(first), Some(last)) = (first_branch, last_branch) {
                if branch.name == first.name {
                    format!("{}{}{}", merge_status_str, "*-".green(), branch.name)
                } else if branch.name == last.name {
                    format!("{}{}{}", merge_status_str, "└─".green(), branch.name)
                } else {
                    format!("{}{}{}", merge_status_str, "├─".green(), branch.name)
                }
            } else {
                format!("{}{}", merge_status_str, branch.name)
            };

            // Get PR/review info
            let reviews_str = if let Some(reviews) = branch_review_map.get(&branch.name.to_string())
            {
                let review_numbers = reviews
                    .iter()
                    .map(|r| format!("{}{}", r.unit_symbol, r.number))
                    .collect::<Vec<String>>()
                    .join(", ");
                format!(" ({review_numbers})").blue().to_string()
            } else {
                String::new()
            };

            let branch_str = format!("{branch_with_prefix}{reviews_str}");

            table.add_row(vec![
                Cell::new(type_str),
                Cell::new(branch_str),
                Cell::new(ahead_str),
                Cell::new(date_str.dimmed().to_string()),
                Cell::new(author_str.dimmed().to_string()),
            ]);
        }
    }

    table.render(out)?;
    Ok(())
}

fn print_branches_table(
    branches: &[gitbutler_branch_actions::BranchListing],
    branch_review_map: &HashMap<String, Vec<but_forge::ForgeReview>>,
    commits_ahead_map: Option<&HashMap<String, usize>>,
    merge_status_map: Option<&HashMap<String, bool>>,
    out: &mut (dyn std::fmt::Write + 'static),
) -> Result<(), anyhow::Error> {
    use crate::tui::{Table, table::Cell};

    if branches.is_empty() {
        return Ok(());
    }

    // Define column headers with fixed widths
    // BRANCH is marked no_truncate so the name is always fully visible.
    // AUTHOR is flexible and will shrink first when space is tight.
    let headers = vec![
        Cell::new("TYPE").with_width(7),
        Cell::new("BRANCH").no_truncate(),
        Cell::new("AHEAD").with_width(6),
        Cell::new("DATE").with_width(10),
        Cell::new("AUTHOR"),
    ];

    let mut table = Table::new(headers);

    for branch in branches {
        // Type column
        let type_str = if branch.has_local {
            "local".normal().to_string()
        } else {
            "remote".dimmed().to_string()
        };

        // Ahead column
        let ahead_str = commits_ahead_map
            .and_then(|map| map.get(&branch.name.to_string()))
            .map(|count| format!("↑{count}").bright_cyan().to_string())
            .unwrap_or_default();

        // Merge status indicator
        let merge_status_str = if let Some(map) = merge_status_map {
            match map.get(&branch.name.to_string()) {
                Some(true) => "✓ ".green().to_string(),
                Some(false) => "✗ ".red().to_string(),
                None => String::new(),
            }
        } else {
            String::new()
        };

        // Date column
        let date_str = format_date_for_display(branch.updated_at);

        // Author column
        let author_str = branch
            .last_commiter
            .name
            .as_ref()
            .map(|n| n.to_string())
            .or_else(|| branch.last_commiter.email.as_ref().map(|e| e.to_string()))
            .unwrap_or_else(|| "Unknown".to_string());

        // Branch name with PR info and merge status
        let reviews_str = if let Some(reviews) = branch_review_map.get(&branch.name.to_string()) {
            let review_numbers = reviews
                .iter()
                .map(|r| format!("{}{}", r.unit_symbol, r.number))
                .collect::<Vec<String>>()
                .join(", ");
            format!(" ({review_numbers})").blue().to_string()
        } else {
            String::new()
        };

        let branch_str = format!("{}{}{}", merge_status_str, branch.name, reviews_str);

        table.add_row(vec![
            Cell::new(type_str),
            Cell::new(branch_str),
            Cell::new(ahead_str),
            Cell::new(date_str.dimmed().to_string()),
            Cell::new(author_str.dimmed().to_string()),
        ]);
    }

    table.render(out)?;
    Ok(())
}

fn get_target_oid(ctx: &Context) -> anyhow::Result<git2::Oid> {
    let handle = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
    let target = handle.get_default_target()?;
    let git2_repo = ctx.git2_repo.get()?;
    let target_ref = format!(
        "refs/remotes/{}/{}",
        target.branch.remote(),
        target.branch.branch()
    );
    match git2_repo.find_reference(&target_ref) {
        Ok(r) => Ok(r.peel(git2::ObjectType::Commit)?.id()),
        Err(_) => Ok(target.sha.to_git2()),
    }
}
