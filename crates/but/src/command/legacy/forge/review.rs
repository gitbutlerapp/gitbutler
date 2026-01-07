use anyhow::Context as _;
use bstr::ByteSlice;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_oxidize::OidExt;
use but_settings::AppSettings;
use but_workspace::ui::{BranchDetails, Commit};
use cli_prompts::DisplayPrompt;
use colored::{ColoredString, Colorize};
use gitbutler_project::Project;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{CliId, IdMap, tui::get_text, utils::OutputChannel};

/// Set the review template for the given project.
pub fn set_review_template(
    ctx: &mut Context,
    template_path: Option<String>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    if let Some(path) = template_path {
        let message = format!("Set review template path to: {}", &path);
        but_api::legacy::forge::set_review_template(ctx.legacy_project.id, Some(path))?;
        if let Some(out) = out.for_human() {
            writeln!(out, "{}", message)?;
        }
    } else {
        let current_template = but_api::legacy::forge::review_template(ctx.legacy_project.id)?;
        let available_templates =
            but_api::legacy::forge::list_available_review_templates(ctx.legacy_project.id)?;
        let template_prompt = cli_prompts::prompts::Selection::new_with_transformation(
            "Select a review template (and press Enter)",
            available_templates.into_iter(),
            |s| {
                if let Some(current) = &current_template {
                    if s == &current.path {
                        format!("{} (current)", s)
                    } else {
                        s.clone()
                    }
                } else {
                    s.clone()
                }
            },
        );

        let selected_template = template_prompt
            .display()
            .map_err(|_| anyhow::anyhow!("Could not determine selected review template"))?;
        let message = format!("Set review template path to: {}", &selected_template);
        but_api::legacy::forge::set_review_template(
            ctx.legacy_project.id,
            Some(selected_template.clone()),
        )?;
        if let Some(out) = out.for_human() {
            writeln!(out, "{}", message)?;
        }
    }

    Ok(())
}

/// Create a new PR for a branch.
/// If no branch is specified, prompts the user to select one.
/// If there is only one branch without a PR, asks for confirmation.
pub async fn create_pr(
    ctx: &mut Context,
    branch: Option<String>,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let review_map = get_review_map(&ctx.legacy_project, Some(but_forge::CacheConfig::CacheOnly))?;
    let applied_stacks = but_api::legacy::workspace::stacks(
        ctx.legacy_project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // If branch is specified, resolve it
    let maybe_branch_names = if let Some(branch_id) = branch {
        Some(get_branch_names(&ctx.legacy_project, &branch_id)?)
    } else {
        // Find branches without PRs
        let branches_without_prs = get_branches_without_prs(&review_map, &applied_stacks);

        if branches_without_prs.is_empty() {
            if let Some(out) = out.for_human() {
                writeln!(out, "All branches already have PRs.")?;
            }
            return Ok(());
        } else if branches_without_prs.len() == 1 {
            // If there's only one branch without a PR, ask for confirmation
            let branch_name = &branches_without_prs[0];
            let mut inout = out.prepare_for_terminal_input().context(
                "Terminal input not available. Please specify a branch using command line arguments.",
            )?;
            let response = inout
                .prompt(format!(
                    "Do you want to open a new PR on branch '{}'? [y/n]",
                    branch_name
                ))?
                .context("Aborted.")?
                .to_lowercase();
            if response == "y" || response == "yes" {
                Some(vec![branch_name.clone()])
            } else {
                return Ok(());
            }
        } else {
            // Multiple branches without PRs - let the prompt handle it
            None
        }
    };

    handle_multiple_branches_in_workspace(
        &ctx.legacy_project,
        &review_map,
        &applied_stacks,
        skip_force_push_protection,
        with_force,
        run_hooks,
        default,
        out,
        maybe_branch_names,
    )
    .await
}

/// Get list of branch names that don't have PRs yet.
fn get_branches_without_prs(
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
) -> Vec<String> {
    let mut branches_without_prs = Vec::new();
    for stack_entry in applied_stacks {
        for head in &stack_entry.heads {
            let branch_name = head.name.to_string();
            if !review_map.contains_key(&branch_name)
                || review_map
                    .get(&branch_name)
                    .map(|v| v.is_empty())
                    .unwrap_or(true)
            {
                branches_without_prs.push(branch_name);
            }
        }
    }
    branches_without_prs
}

fn get_branch_names(project: &Project, branch_id: &str) -> anyhow::Result<Vec<String>> {
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let mut id_map = IdMap::new_from_context(&mut ctx, None)?;
    id_map.add_committed_file_info_from_context(&mut ctx)?;
    let branch_ids = id_map
        .resolve_entity_to_ids(branch_id)?
        .iter()
        .filter_map(|clid| match clid {
            CliId::Branch { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if branch_ids.is_empty() {
        anyhow::bail!("No branch found for ID: {}", branch_id);
    }

    Ok(branch_ids)
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_multiple_branches_in_workspace(
    project: &Project,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default_message: bool,
    out: &mut OutputChannel,
    selected_branches: Option<Vec<String>>,
) -> anyhow::Result<()> {
    let mut overall_outcome = PublishReviewsOutcome {
        published: vec![],
        already_existing: vec![],
    };

    let selected_branches = if let Some(branches) = selected_branches {
        branches
    } else {
        prompt_for_branch_selection(project, review_map, applied_stacks, out)?
    };

    if selected_branches.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "No branches selected for review publication. Aborting."
            )?;
        }
        return Ok(());
    }

    for stack_entry in applied_stacks {
        let Some(top_most_selected_head) = stack_entry
            .heads
            .iter()
            .find(|h| selected_branches.contains(&h.name.to_string()))
        else {
            continue;
        };

        let outcome = publish_reviews_for_branch_and_dependents(
            project,
            top_most_selected_head.name.to_str()?,
            review_map,
            stack_entry,
            skip_force_push_protection,
            with_force,
            run_hooks,
            default_message,
            out,
        )
        .await?;

        overall_outcome.published.extend(outcome.published);
        overall_outcome
            .already_existing
            .extend(outcome.already_existing);
    }

    if let Some(out) = out.for_json() {
        out.write_value(overall_outcome)?;
    } else if let Some(out) = out.for_human() {
        display_review_publication_summary(overall_outcome, out)?;
    }

    Ok(())
}

/// Prompt the user to select branches to publish from a numbered list.
fn prompt_for_branch_selection(
    project: &Project,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    out: &mut OutputChannel,
) -> anyhow::Result<Vec<String>> {
    let (base_branch, ctx) = get_base_branch_and_context(project)?;
    let base_branch_id = base_branch.current_sha.to_gix();
    let repo = &*ctx.repo.get()?;

    // Collect all branches with their information
    let mut all_branches: Vec<(String, usize, Vec<String>)> = Vec::new();

    for stack_entry in applied_stacks {
        for head in &stack_entry.heads {
            let mut branch_ref = repo.find_reference(head.name.as_bstr())?;
            let branch_id = branch_ref.peel_to_id()?;
            let commits = but_workspace::local_commits_for_branch(branch_id, base_branch_id)?;
            let reviews = review_map
                .get(&head.name.to_string())
                .map(|reviews| {
                    reviews
                        .iter()
                        .map(|r| format!("{}{}", r.unit_symbol, r.number))
                        .collect()
                })
                .unwrap_or_default();

            all_branches.push((head.name.to_string(), commits.len(), reviews));
        }
    }

    if all_branches.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No branches available to publish.")?;
        }
        return Ok(vec![]);
    }

    use std::fmt::Write;
    let mut inout = out
        .prepare_for_terminal_input()
        .context("Terminal input not available. Please specify branches to publish using command line arguments.")?;

    // Display branches with numbers
    writeln!(inout, "\nAvailable branches to publish:\n")?;
    for (idx, (name, commit_count, reviews)) in all_branches.iter().enumerate() {
        let review_str = if !reviews.is_empty() {
            format!(" ({})", reviews.join(", "))
        } else {
            String::new()
        };
        writeln!(
            inout,
            "  {}. {} - {} commit{}{}",
            idx + 1,
            name.bold(),
            commit_count,
            if *commit_count == 1 { "" } else { "s" },
            review_str.blue()
        )?;
    }

    let input = inout
        .prompt("\nEnter branch numbers to publish (comma-separated, or 'all' for all branches):")?
        .context("No branches selected. Aborting.")?;

    // Parse selection
    let selected_branches: Vec<String> = if input.eq_ignore_ascii_case("all") {
        all_branches.into_iter().map(|(name, _, _)| name).collect()
    } else {
        let mut selected = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            if let Ok(num) = part.parse::<usize>() {
                if num > 0 && num <= all_branches.len() {
                    selected.push(all_branches[num - 1].0.clone());
                } else {
                    println!("Warning: Ignoring invalid branch number: {}", num);
                }
            } else {
                println!("Warning: Ignoring invalid input: {}", part);
            }
        }
        selected
    };

    Ok(selected_branches)
}

#[allow(clippy::too_many_arguments)]
async fn publish_reviews_for_branch_and_dependents(
    project: &Project,
    branch_name: &str,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    stack_entry: &but_workspace::legacy::ui::StackEntry,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default_message: bool,
    out: &mut OutputChannel,
) -> Result<PublishReviewsOutcome, anyhow::Error> {
    let (base_branch, _) = get_base_branch_and_context(project)?;
    let all_branches_up_to_subject = stack_entry
        .heads
        .iter()
        .rev()
        .take_while(|h| h.name != branch_name)
        .collect::<Vec<_>>();

    if let Some(out) = out.for_human() {
        write!(out, "{} ", "→".cyan())?;
        if !all_branches_up_to_subject.is_empty() {
            writeln!(
                out,
                "Pushing {} with {} dependent branch(es)...",
                branch_name.green().bold(),
                all_branches_up_to_subject.len().to_string().yellow()
            )?;
        } else {
            writeln!(out, "Pushing {}...", branch_name.green().bold())?;
        }
    }

    let result = but_api::legacy::stack::push_stack(
        project.id,
        stack_entry
            .id
            .context("BUG: Stack entry is missing ID for push")?,
        with_force,
        skip_force_push_protection,
        branch_name.to_string(),
        run_hooks,
        vec![],
    )?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {} Pushed to {}",
            "✓".green().bold(),
            result.remote.cyan()
        )?;
    }

    let mut newly_published = Vec::new();
    let mut already_existing = Vec::new();
    let mut current_target_branch = base_branch.short_name();
    for head in stack_entry.heads.iter().rev() {
        if let Some(out) = out.for_human() {
            write!(out, "{} ", "→".cyan())?;
            writeln!(
                out,
                "Creating PR for {} {} {}...",
                head.name.to_string().green().bold(),
                "→".dimmed(),
                current_target_branch.cyan()
            )?;
        }

        let published_review = publish_review_for_branch(
            project,
            stack_entry.id,
            head.name.to_str()?,
            current_target_branch,
            review_map,
            default_message,
        )
        .await?;
        match published_review {
            PublishReviewResult::Published(review) => {
                newly_published.push(*review);
            }
            PublishReviewResult::AlreadyExists(reviews) => {
                already_existing.extend(reviews);
            }
        }

        current_target_branch = head.name.to_str()?;

        if head.name == branch_name {
            break;
        }
    }

    let outcome = PublishReviewsOutcome {
        published: newly_published,
        already_existing,
    };

    Ok(outcome)
}

fn get_base_branch_and_context(
    project: &Project,
) -> Result<(gitbutler_branch_actions::BaseBranch, but_ctx::Context), anyhow::Error> {
    let app_settings = AppSettings::load_from_default_path_creating_without_customization()?;
    let ctx = Context::new_from_legacy_project_and_settings(project, app_settings);
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    Ok((base_branch, ctx))
}

/// Display a summary of published and already existing reviews for humans
fn display_review_publication_summary(
    outcome: PublishReviewsOutcome,
    out: &mut dyn std::fmt::Write,
) -> std::fmt::Result {
    // Show newly published PRs
    if !outcome.published.is_empty() {
        writeln!(out)?;
        for review in &outcome.published {
            print_new_pr_info(review, out)?;
        }
    }

    // Show already existing PRs
    if !outcome.already_existing.is_empty() {
        writeln!(out)?;
        for review in &outcome.already_existing {
            print_existing_pr_info(review, out)?;
        }
    }

    Ok(())
}

/// Print information about a newly created PR
fn print_new_pr_info(
    review: &but_forge::ForgeReview,
    out: &mut dyn std::fmt::Write,
) -> std::fmt::Result {
    writeln!(
        out,
        "{} {} {}{}",
        "✓".green().bold(),
        "Created PR".green(),
        review.unit_symbol.cyan(),
        review.number.to_string().cyan().bold()
    )?;
    writeln!(out, "  {} {}", "Title:".dimmed(), review.title.bold())?;
    writeln!(
        out,
        "  {} {}",
        "Branch:".dimmed(),
        review.source_branch.green()
    )?;
    writeln!(
        out,
        "  {} {}",
        "URL:".dimmed(),
        review.html_url.underline().blue()
    )?;

    Ok(())
}

/// Print information about an existing PR
fn print_existing_pr_info(
    review: &but_forge::ForgeReview,
    out: &mut dyn std::fmt::Write,
) -> std::fmt::Result {
    writeln!(
        out,
        "{} {} {} {}{}",
        "•".yellow(),
        "PR already exists for".yellow(),
        review.source_branch.green().bold(),
        review.unit_symbol.cyan(),
        review.number.to_string().cyan().bold()
    )?;
    writeln!(
        out,
        "  {} {}",
        "URL:".dimmed(),
        review.html_url.underline().blue()
    )?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishReviewsOutcome {
    published: Vec<but_forge::ForgeReview>,
    already_existing: Vec<but_forge::ForgeReview>,
}

enum PublishReviewResult {
    Published(Box<but_forge::ForgeReview>),
    AlreadyExists(Vec<but_forge::ForgeReview>),
}

async fn publish_review_for_branch(
    project: &Project,
    stack_id: Option<StackId>,
    branch_name: &str,
    target_branch: &str,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    default_message: bool,
) -> anyhow::Result<PublishReviewResult> {
    // Check if a review already exists for the branch.
    // If it does, skip publishing a new review.
    let existing_reviews = review_map.get(branch_name);
    if let Some(reviews) = existing_reviews
        && !reviews.is_empty()
    {
        return Ok(PublishReviewResult::AlreadyExists(reviews.clone()));
    }

    let commit = default_commit(project, stack_id, branch_name)?;
    let (title, body) = if default_message {
        let title = extract_commit_title(commit.as_ref())
            .map(|t| t.to_string())
            .unwrap_or(branch_name.to_string());
        let body = extract_commit_description(commit.as_ref())
            .map(|b| b.join("\n"))
            .unwrap_or_default();
        (title, body)
    } else {
        get_pr_title_and_body_from_editor(project, stack_id, commit.as_ref(), branch_name)?
    };

    // Publish a new review for the branch
    but_api::legacy::forge::publish_review(
        project.id,
        but_forge::CreateForgeReviewParams {
            title,
            body,
            source_branch: branch_name.to_string(),
            target_branch: target_branch.to_string(),
            draft: false,
        },
    )
    .await
    .map(|review| {
        if let Some(stack_id) = stack_id {
            let review_number = review.number.try_into().ok();
            but_api::legacy::stack::update_branch_pr_number(
                project.id,
                stack_id,
                branch_name.to_string(),
                review_number,
            )
            .ok();
        }
        PublishReviewResult::Published(Box::new(review))
    })
}

/// Get the default commit for the branch, if it has exactly one commit.
fn default_commit(
    project: &Project,
    stack_id: Option<StackId>,
    branch_name: &str,
) -> Result<Option<Commit>, anyhow::Error> {
    let stack_details = but_api::legacy::workspace::stack_details(project.id, stack_id)?;
    let branch = stack_details
        .branch_details
        .into_iter()
        .find(|h| h.name.to_str().unwrap_or("") == branch_name);
    let commit = if let Some(branch) = &branch
        && branch.commits.len() == 1
    {
        branch.commits.first()
    } else {
        None
    };

    Ok(commit.cloned())
}

/// Prompt the user to enter the PR title and description using their default editor.
/// Opens a single file where the first line is the title and the rest is the description.
/// Pre-fills with commit message if available and includes commit list with files.
fn get_pr_title_and_body_from_editor(
    project: &Project,
    stack_id: Option<StackId>,
    commit: Option<&Commit>,
    branch_name: &str,
) -> anyhow::Result<(String, String)> {
    let mut template = String::new();

    // Use the first line of the commit message as the default title if available
    let commit_title = extract_commit_title(commit);
    if let Some(commit_title) = commit_title {
        template.push_str(commit_title);
    } else {
        template.push_str(branch_name);
    }
    template.push('\n');

    // Add a blank line between title and description
    template.push('\n');

    // Use commit description as template if available
    let commit_description = extract_commit_description(commit);
    if let Some(commit_description) = commit_description {
        for line in commit_description {
            template.push_str(line);
            template.push('\n');
        }
    } else if let Some(review_template) = but_api::legacy::forge::review_template(project.id)? {
        template.push_str(&review_template.content);
        template.push('\n');
    }

    // Add instructions as comments
    template.push_str("\n# PR Title and Description for branch: ");
    template.push_str(branch_name);
    template.push_str("\n#\n");
    template.push_str("# The FIRST LINE of this file will be the PR title.\n");
    template.push_str("# Everything AFTER the first line will be the PR description.\n");
    template.push_str("#\n");
    template.push_str("# Lines starting with '#' will be ignored.\n");
    template.push_str("# An empty title (first line) aborts the operation.\n");
    template.push_str("#\n");

    // Add commit list with modified files as context
    if let Ok(stack_details) = but_api::legacy::workspace::stack_details(project.id, stack_id)
        && let Some(branch) = stack_details
            .branch_details
            .into_iter()
            .find(|h| h.name.to_str().unwrap_or("") == branch_name)
        && !branch.commits.is_empty()
    {
        template.push_str("# Commits in this PR:\n");
        template.push_str("#\n");

        // Get the repository for diff operations
        if let Ok(ctx) = but_ctx::Context::new_from_legacy_project(project.clone())
            && let Ok(repo) = ctx.repo.get()
        {
            for (idx, commit) in branch.commits.iter().enumerate() {
                // Extract commit title (first line of message)
                let commit_title = commit
                    .message
                    .lines()
                    .next()
                    .and_then(|l| l.to_str().ok())
                    .unwrap_or("");
                template.push_str(&format!(
                    "# {}. {} ({})\n",
                    idx + 1,
                    commit_title,
                    commit.id.to_hex_with_len(7)
                ));

                // Get the files modified in this commit
                let parent = commit.parent_ids.first().copied();
                if let Ok(changes) =
                    but_core::diff::TreeChanges::from_trees(&repo, parent, commit.id)
                {
                    let mut file_paths: Vec<String> = changes
                        .0
                        .iter()
                        .map(|change| {
                            let tree_change: but_core::TreeChange = change.clone().into();
                            tree_change.path.to_string()
                        })
                        .collect();
                    file_paths.sort();

                    if !file_paths.is_empty() {
                        template.push_str("#    Modified files:\n");
                        for file in file_paths.iter().take(10) {
                            template.push_str(&format!("#      - {}\n", file));
                        }
                        if file_paths.len() > 10 {
                            template.push_str(&format!(
                                "#      ... and {} more files\n",
                                file_paths.len() - 10
                            ));
                        }
                    }
                }
                template.push_str("#\n");
            }
        }
    }
    template.push_str("#\n");

    let content = get_text::from_editor_no_comments("pr_message", &template)?.to_string();

    // Split into title (first line) and body (rest)
    let mut lines = content.lines();
    let title = lines.next().unwrap_or("").trim().to_string();

    if title.is_empty() {
        anyhow::bail!("Aborting due to empty PR title");
    }

    // Skip any leading blank lines after the title, then collect the rest as description
    let body: String = lines
        .skip_while(|l| l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    Ok((title, body))
}

/// Extract the commit description (body) from the commit message, skipping the first line (title).
fn extract_commit_description(commit: Option<&Commit>) -> Option<Vec<&str>> {
    commit.and_then(|c| {
        let desc_lines: Vec<&str> = c
            .message
            .lines()
            .skip(1)
            .skip_while(|l| l.trim().is_empty())
            .map(|l| l.to_str().ok())
            .collect::<Option<Vec<&str>>>()?;
        if desc_lines.is_empty() {
            None
        } else {
            Some(desc_lines)
        }
    })
}

/// Extract the commit title from the commit message (first line).
fn extract_commit_title(commit: Option<&Commit>) -> Option<&str> {
    commit.and_then(|c| c.message.lines().next().and_then(|l| l.to_str().ok()))
}

/// Get a mapping from branch names to their associated reviews.
#[instrument(skip(project))]
pub fn get_review_map(
    project: &Project,
    cache_config: Option<but_forge::CacheConfig>,
) -> anyhow::Result<std::collections::HashMap<String, Vec<but_forge::ForgeReview>>> {
    let reviews =
        but_api::legacy::forge::list_reviews(project.id, cache_config).unwrap_or_default();

    let branch_review_map =
        reviews
            .into_iter()
            .fold(std::collections::HashMap::new(), |mut acc, r| {
                // TODO: Handle forks properly
                let clean_branch_name = r
                    .source_branch
                    .split(':')
                    .next_back()
                    .unwrap_or(&r.source_branch)
                    .to_string();
                acc.entry(clean_branch_name)
                    .or_insert_with(Vec::new)
                    .push(r);
                acc
            });

    Ok(branch_review_map)
}

pub(crate) fn from_branch_details(
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    details: &BranchDetails,
) -> Option<but_forge::ForgeReview> {
    review_map
        .get(&details.name.to_string())
        .and_then(|rs| {
            details
                .pr_number
                .and_then(|pr| rs.iter().find(|r| r.number == pr as i64))
                .or_else(|| rs.first())
        })
        .cloned()
}

pub fn get_review_numbers(
    branch_name: &str,
    associated_review_number: &Option<usize>,
    branch_review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
) -> ColoredString {
    if let Some(reviews) = branch_review_map.get(branch_name) {
        let review_numbers = reviews
            .iter()
            .map(|r| format!("{}{}", r.unit_symbol, r.number))
            .collect::<Vec<String>>()
            .join(", ");

        format!(" ({})", review_numbers).blue()
    } else if let Some(pr_number) = associated_review_number {
        format!(" (#{})", pr_number).blue()
    } else {
        "".to_string().normal()
    }
}
