use anyhow::Context as _;
use bstr::ByteSlice;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_oxidize::OidExt;
use but_settings::AppSettings;
use but_workspace::ui::Commit;
use cli_prompts::DisplayPrompt;
use colored::{ColoredString, Colorize};
use gitbutler_project::{Project, ProjectId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

/// Publish reviews for active branches in the workspace.
pub async fn publish_reviews(
    ctx: &mut Context,
    branch: Option<String>,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let review_map = get_review_map(&ctx.legacy_project).await?;
    let applied_stacks = but_api::legacy::workspace::stacks(
        ctx.legacy_project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;
    let maybe_branch_names = branch
        .map(|branch_id| get_branch_names(&ctx.legacy_project, &branch_id))
        .transpose()?;
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

fn get_branch_names(project: &Project, branch_id: &str) -> anyhow::Result<Vec<String>> {
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let mut id_map = IdMap::new_from_context(&ctx)?;
    id_map.add_file_info_from_context(&mut ctx, None)?;
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
        if !all_branches_up_to_subject.is_empty() {
            writeln!(
                out,
                "Pushing branch '{}' with {} dependent branch(es) first",
                branch_name,
                all_branches_up_to_subject.len()
            )?;
        } else {
            writeln!(out, "Pushing branch '{}'", branch_name)?;
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
        writeln!(out, "Push completed successfully")?;
        writeln!(out, "Pushed to remote: {}", result.remote)?;
        if !result.branch_to_remote.is_empty() {
            for (branch, remote_ref) in &result.branch_to_remote {
                writeln!(out, "  {} -> {}", branch, remote_ref)?;
            }
        }
        writeln!(out)?;
    }

    let mut newly_published = Vec::new();
    let mut already_existing = Vec::new();
    let mut current_target_branch = base_branch.short_name();
    for head in stack_entry.heads.iter().rev() {
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "Publishing review for branch '{}' targeting '{}",
                head.name, current_target_branch
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
    // Group published reviews by branch name
    let mut published_by_branch: BTreeMap<&str, Vec<&but_forge::ForgeReview>> = BTreeMap::new();
    for review in &outcome.published {
        published_by_branch
            .entry(review.source_branch.as_str())
            .or_default()
            .push(review);
    }
    for (branch, reviews) in published_by_branch {
        writeln!(out, "Published reviews for branch '{}':", branch)?;
        for review in reviews {
            print_review_information(review, out)?;
        }
    }

    // Group already existing reviews by branch name
    let mut existing_by_branch: BTreeMap<&str, Vec<&but_forge::ForgeReview>> = BTreeMap::new();
    for review in &outcome.already_existing {
        existing_by_branch
            .entry(review.source_branch.as_str())
            .or_default()
            .push(review);
    }
    for (branch, reviews) in existing_by_branch {
        writeln!(out, "Review(s) already exist for branch '{}':", branch)?;
        for review in reviews {
            print_review_information(review, out)?;
        }
    }

    Ok(())
}

/// Print review information in a formatted way
fn print_review_information(
    review: &but_forge::ForgeReview,
    out: &mut dyn std::fmt::Write,
) -> std::fmt::Result {
    writeln!(
        out,
        "  '{}' ({}{}): {}",
        review.title.bold(),
        review.unit_symbol.blue(),
        review.number.to_string().blue(),
        review.html_url.underline()
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
        let title = get_review_title_from_editor(commit.as_ref(), branch_name)?;
        let body = get_review_body_from_editor(project.id, commit.as_ref(), branch_name, &title)?;
        (title, body)
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

/// Prompt the user to enter the review body using their default editor.
/// Pre-fills the editor with the commit description if available.
fn get_review_body_from_editor(
    project_id: ProjectId,
    commit: Option<&Commit>,
    branch_name: &str,
    title: &str,
) -> anyhow::Result<String> {
    let mut template = String::new();

    let commit_description = extract_commit_description(commit);

    // Use commit description as template if available
    if let Some(commit_description) = commit_description {
        for line in commit_description {
            template.push_str(line);
            template.push('\n');
        }
    } else if let Some(review_template) = but_api::legacy::forge::review_template(project_id)? {
        template.push_str(&review_template.content);
    } else {
        template.push_str(branch_name);
    }

    template.push_str("\n# This is the review description for:");
    template.push_str("\n# '");
    template.push_str(title);
    template.push_str("' \n");
    template.push_str("\n# Optionally, enter the review body above. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty body is allowed.\n");
    template.push_str("#\n");

    let lossy_body = get_text::from_editor_no_comments("review_body", &template)?.to_string();
    Ok(lossy_body)
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

/// Prompt the user to enter the review title using their default editor.
/// Pre-fills the editor with the commit title if available.
fn get_review_title_from_editor(
    commit: Option<&Commit>,
    branch_name: &str,
) -> anyhow::Result<String> {
    let mut template = String::new();

    // Use the first line of the commit message as the default title if available
    let commit_title = extract_commit_title(commit);
    if let Some(commit_title) = commit_title {
        template.push_str(commit_title);
    } else {
        template.push_str(branch_name);
    }

    template.push_str("\n# Please enter the review title above. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty title aborts the operation.\n");
    template.push_str("#\n");

    let lossy_title = get_text::from_editor_no_comments("review_title", &template)?.to_string();

    if lossy_title.is_empty() {
        anyhow::bail!("Aborting due to empty review title");
    }

    Ok(lossy_title)
}

/// Extract the commit title from the commit message (first line).
fn extract_commit_title(commit: Option<&Commit>) -> Option<&str> {
    commit.and_then(|c| c.message.lines().next().and_then(|l| l.to_str().ok()))
}

/// Get a mapping from branch names to their associated reviews.
#[instrument(skip(project))]
pub async fn get_review_map(
    project: &Project,
) -> anyhow::Result<std::collections::HashMap<String, Vec<but_forge::ForgeReview>>> {
    let reviews = but_api::legacy::forge::list_reviews(project.id)
        .await
        .unwrap_or_default();

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
