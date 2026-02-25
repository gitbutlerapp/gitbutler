use anyhow::Context as _;
use bstr::{BStr, ByteSlice};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_oxidize::OidExt;
use but_workspace::ui::Commit;
use cli_prompts::DisplayPrompt;
use colored::{ColoredString, Colorize};
use gitbutler_project::Project;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    CliId, IdMap,
    command::legacy::rub::parse_sources,
    tui::get_text::{self, HTML_COMMENT_END_MARKER, HTML_COMMENT_START_MARKER},
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

/// Automatically merge the review once all prerequisites are met.
pub async fn enable_auto_merge(
    ctx: &mut Context,
    selector: Option<String>,
    off: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Fail fast if no forge user is authenticated, before pushing or prompting.
    ensure_forge_authentication(ctx).await?;

    let review_ids = resolve_review_selection(ctx, selector)?;

    if review_ids.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No reviews selected")?;
        }
        return Ok(());
    }
    let review_count = review_ids.len();

    let action = if off { "disabled" } else { "enabled" };
    let mut skipped_reviews = 0;

    // Iterate over the reviews that need to be mutated. Skip if their in an invalid state.
    for review_id in review_ids {
        // Fetch the latest information about the review
        let review = but_api::legacy::forge::get_review(ctx, review_id).ok();

        if let Some(review) = review {
            if review.draft {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "Skipping review ({}{}) {}. Review is still draft.",
                        review.unit_symbol, review.number, review.title
                    )?;
                }
                skipped_reviews += 1;
                continue;
            }

            if !review.is_open() {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "Skipping review ({}{}) {}. Review is not open.",
                        review.unit_symbol, review.number, review.title
                    )?;
                }
                skipped_reviews += 1;
                continue;
            }

            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "Auto-merge {} for review ({}{}) {}",
                    action, review.unit_symbol, review.number, review.title
                )?;
            }
        } else if let Some(out) = out.for_human() {
            writeln!(out, "Auto-merge {action} for review {review_id}")?;
        }

        but_api::legacy::forge::set_review_auto_merge(ctx.to_sync(), review_id, !off).await?;
    }

    if let Some(out) = out.for_human() {
        let actual_reviews_modified = review_count - skipped_reviews;
        if actual_reviews_modified > 0 {
            let review_word = if actual_reviews_modified == 1 {
                "review"
            } else {
                "reviews"
            };
            writeln!(out, "Auto-merge {action} for {review_count} {review_word}")?;
        }

        if skipped_reviews > 0 {
            let review_word = if skipped_reviews == 1 {
                "review"
            } else {
                "reviews"
            };
            writeln!(
                out,
                "Skipped {review_count} {review_word} because of reasons.\nOnce those reasons have been addressed, run `but fetch` to refetch the data and try again."
            )?;
        }
    }

    Ok(())
}

/// Set the draftiness of or or multiple reviews.
pub async fn set_draftiness(
    ctx: &mut Context,
    selector: Option<String>,
    draft: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Fail fast if no forge user is authenticated, before pushing or prompting.
    ensure_forge_authentication(ctx).await?;

    let review_ids = resolve_review_selection(ctx, selector)?;

    if review_ids.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No reviews selected")?;
        }
        return Ok(());
    }
    let review_count = review_ids.len();
    let mut skipped_reviews = 0;

    // Iterate over the reviews and validate the state, before mutating.
    for review_id in review_ids {
        // Fetch the latest information about the review
        let review = but_api::legacy::forge::get_review(ctx, review_id).ok();

        if let Some(review) = review {
            if !review.is_open() {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "Skipping review ({}{}) {}. Review is not open.",
                        review.unit_symbol, review.number, review.title
                    )?;
                }
                skipped_reviews += 1;
                continue;
            }

            if draft && review.draft {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "Skipping review ({}{}) {}. Review is already draft.",
                        review.unit_symbol, review.number, review.title
                    )?;
                }
                skipped_reviews += 1;
                continue;
            }

            if !draft && !review.draft {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "Skipping review ({}{}) {}. Review is already ready for review.",
                        review.unit_symbol, review.number, review.title
                    )?;
                }
                skipped_reviews += 1;
                continue;
            }

            if let Some(out) = out.for_human() {
                let action = if draft {
                    "Set as draft"
                } else {
                    "Set as ready"
                };
                writeln!(
                    out,
                    "{} review ({}{}) {}",
                    action, review.unit_symbol, review.number, review.title
                )?;
            }
        } else if let Some(out) = out.for_human() {
            let action = if draft {
                "Set as draft"
            } else {
                "Set as ready"
            };
            writeln!(out, "{action} review {review_id}")?;
        }

        but_api::legacy::forge::set_review_draftiness(ctx.to_sync(), review_id, draft).await?;
    }

    if let Some(out) = out.for_human() {
        let action = if draft {
            "set as draft"
        } else {
            "set as ready"
        };
        let actual_reviews_modified = review_count - skipped_reviews;

        if actual_reviews_modified > 0 {
            let review_word = if actual_reviews_modified == 1 {
                "review"
            } else {
                "reviews"
            };
            writeln!(out, "{actual_reviews_modified} {review_word} {action}.")?;
        }

        if skipped_reviews > 0 {
            let review_word = if skipped_reviews == 1 {
                "review"
            } else {
                "reviews"
            };
            writeln!(
                out,
                "Skipped {skipped_reviews} {review_word} because review state is incompatible with this action.\nOnce those reasons have been addressed, run `but fetch` to refetch the data and try again."
            )?;
        }
    }

    Ok(())
}

/// Set the review template for the given project.
pub fn set_review_template(
    ctx: &mut Context,
    template_path: Option<String>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    if let Some(path) = template_path {
        let message = format!("Set review template path to: {}", &path);
        but_api::legacy::forge::set_review_template(ctx, Some(path))?;
        if let Some(out) = out.for_human() {
            writeln!(out, "{message}")?;
        }
    } else {
        let current_template = but_api::legacy::forge::review_template(ctx)?;
        let available_templates = but_api::legacy::forge::list_available_review_templates(ctx)?;
        let template_prompt = cli_prompts::prompts::Selection::new_with_transformation(
            "Select a review template (and press Enter)",
            available_templates.into_iter(),
            |s| {
                if let Some(current) = &current_template {
                    if s == &current.path {
                        format!("{s} (current)")
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
        but_api::legacy::forge::set_review_template(ctx, Some(selected_template.clone()))?;
        if let Some(out) = out.for_human() {
            writeln!(out, "{message}")?;
        }
    }

    Ok(())
}

/// Create a new forge review for a branch.
/// If no branch is specified, prompts the user to select one.
/// If there is only one branch without a an acco, asks for confirmation.
#[allow(clippy::too_many_arguments)]
pub async fn create_review(
    ctx: &mut Context,
    branch: Option<String>,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default: bool,
    draft: bool,
    message: Option<ForgeReviewMessage>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Fail fast if no forge user is authenticated, before pushing or prompting.
    ensure_forge_authentication(ctx).await?;

    let review_map = get_review_map(ctx, Some(but_forge::CacheConfig::CacheOnly))?;
    let applied_stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // If branch is specified, resolve it
    let maybe_branch_names = if let Some(branch_id) = branch {
        Some(get_branch_names(&ctx.legacy_project, &branch_id)?)
    } else {
        // Find branches without PRs
        let branches_without_prs = get_branches_without_prs(&review_map, &applied_stacks)?;

        if branches_without_prs.is_empty() {
            if let Some(out) = out.for_human() {
                writeln!(out, "All branches already have reviews.")?;
            }
            return Ok(());
        } else if branches_without_prs.len() == 1 {
            // If there's only one branch without a PR, ask for confirmation
            let branch_name = &branches_without_prs[0];
            let mut inout = out
                .prepare_for_terminal_input()
                .context("Terminal input not available. Please specify a branch using command line arguments.")?;
            let draftiness = if draft { "draft " } else { "" };
            if inout.confirm(
                format!("Do you want to open a new {draftiness}review on branch '{branch_name}'?"),
                ConfirmDefault::Yes,
            )? == Confirm::Yes
            {
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
        ctx,
        &review_map,
        &applied_stacks,
        skip_force_push_protection,
        with_force,
        run_hooks,
        default,
        draft,
        message.as_ref(),
        out,
        maybe_branch_names,
    )
    .await
}

/// Make sure that the account that is about to be used in this repository's forge is correctly authenticated.
async fn ensure_forge_authentication(ctx: &mut Context) -> Result<(), anyhow::Error> {
    let (storage, forge_repo_info, preferred_forge_user) = {
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
        let forge_repo_info = but_forge::derive_forge_repo_info(&base_branch.remote_url);
        (
            but_forge_storage::Controller::from_path(but_path::app_data_dir()?),
            forge_repo_info,
            ctx.legacy_project.preferred_forge_user.clone(),
        )
    };

    let forge_repo_info = forge_repo_info.ok_or_else(|| {
        anyhow::anyhow!(
            "Unable to determine the forge for this project. Is target branch associated with a supported forge?"
        )
    })?;

    let account_validity =
        but_forge::check_forge_account_is_valid(preferred_forge_user, &forge_repo_info, &storage)
            .await?;

    let forge_display_name = match forge_repo_info.forge {
        but_forge::ForgeName::Azure => {
            anyhow::bail!("Azure is unsupported at the minute. Sorry 😞.");
        }
        but_forge::ForgeName::Bitbucket => {
            anyhow::bail!("Bitbucket is unsupported at the minute. Sorry 😞.");
        }
        but_forge::ForgeName::GitHub => "GitHub",
        but_forge::ForgeName::GitLab => "GitLab",
    };

    match account_validity {
        but_forge::ForgeAccountValidity::Invalid => Err(anyhow::anyhow!(
            "Known account is not correctly authenticated.\nRun '{}' to authenticate with {}.",
            "but config forge auth",
            forge_display_name
        )),
        but_forge::ForgeAccountValidity::NoCredentials => Err(anyhow::anyhow!(
            "No authenticated forge users found.\nRun '{}' to authenticate with {}.",
            "but config forge auth",
            forge_display_name
        )),
        but_forge::ForgeAccountValidity::Valid => {
            // All good, continue
            Ok(())
        }
    }
}

/// Get list of branch names that don't have PRs yet.
fn get_branches_without_prs(
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
) -> anyhow::Result<Vec<String>> {
    let mut branches_without_prs = Vec::new();
    for stack_entry in applied_stacks {
        for head in &stack_entry.heads {
            let branch_name = &head.name.to_string();
            if !review_map.contains_key(branch_name)
                || review_map
                    .get(branch_name)
                    .map(|v| v.is_empty())
                    .unwrap_or(true)
            {
                // This means that there are no associated reviews that are open, but that doesn't mean that there are
                // no associated reviews.
                // Check whether there's an associated forge review.
                if head.review_id.is_none() {
                    // If there's no associated review, the append the branch
                    branches_without_prs.push(branch_name.to_owned());
                }
            }
        }
    }
    Ok(branches_without_prs)
}

fn get_branch_names(project: &Project, branch_id: &str) -> anyhow::Result<Vec<String>> {
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let id_map = IdMap::new_from_context(&mut ctx, None)?;
    let branch_ids = id_map
        .parse_using_context(branch_id, &mut ctx)?
        .iter()
        .filter_map(|clid| match clid {
            CliId::Branch { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if branch_ids.is_empty() {
        anyhow::bail!("No branch found for ID: {branch_id}");
    }

    Ok(branch_ids)
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_multiple_branches_in_workspace(
    ctx: &mut Context,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default_message: bool,
    draft: bool,
    message: Option<&ForgeReviewMessage>,
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
        prompt_for_branch_selection(ctx, review_map, applied_stacks, out)?
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
            ctx,
            top_most_selected_head.name.to_str()?,
            review_map,
            stack_entry,
            skip_force_push_protection,
            with_force,
            run_hooks,
            default_message,
            draft,
            message,
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
    ctx: &Context,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    out: &mut OutputChannel,
) -> anyhow::Result<Vec<String>> {
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
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
                    println!("Warning: Ignoring invalid branch number: {num}");
                }
            } else {
                println!("Warning: Ignoring invalid input: {part}");
            }
        }
        selected
    };

    Ok(selected_branches)
}

#[allow(clippy::too_many_arguments)]
async fn publish_reviews_for_branch_and_dependents(
    ctx: &mut Context,
    branch_name: &str,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    stack_entry: &but_workspace::legacy::ui::StackEntry,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default_message: bool,
    draft: bool,
    message: Option<&ForgeReviewMessage>,
    out: &mut OutputChannel,
) -> Result<PublishReviewsOutcome, anyhow::Error> {
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(ctx)?;
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
        ctx,
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
    let mut all_reviews_in_order = Vec::new();
    let mut current_target_branch = base_branch.short_name();
    for head in stack_entry.heads.iter().rev() {
        if let Some(out) = out.for_human() {
            let draftiness = if draft { "draft " } else { "" };
            write!(out, "{} ", "→".cyan())?;
            writeln!(
                out,
                "Creating {}review for {} {} {}...",
                draftiness,
                head.name.to_string().green().bold(),
                "→".dimmed(),
                current_target_branch.cyan()
            )?;
        }

        let message_for_head = if head.name == branch_name {
            message
        } else {
            None
        };
        let published_review = publish_review_for_branch(
            ctx,
            stack_entry.id,
            head.name.to_str()?,
            current_target_branch,
            review_map,
            default_message,
            draft,
            message_for_head,
        )
        .await?;
        match published_review {
            PublishReviewResult::Published(review) => {
                newly_published.push(*review.clone());
                all_reviews_in_order.push(*review);
            }
            PublishReviewResult::AlreadyExists(reviews) => {
                if let Some(review) = reviews.first() {
                    // Ignore other existing reviews for ordering
                    all_reviews_in_order.push(review.clone());
                }
                already_existing.extend(reviews);
            }
        }

        current_target_branch = head.name.to_str()?;

        if head.name == branch_name {
            break;
        }
    }

    // Update the PR descriptions to have the footers
    but_api::legacy::forge::update_review_footers(
        ctx.to_sync(),
        all_reviews_in_order.into_iter().map(Into::into).collect(),
    )
    .await?;

    let outcome = PublishReviewsOutcome {
        published: newly_published,
        already_existing,
    };

    Ok(outcome)
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
        "Created review".green(),
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
    if review.draft {
        writeln!(out, "  {}", "Draft only".dimmed())?;
    }

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

#[derive(Clone, Debug)]
pub struct ForgeReviewMessage {
    pub title: String,
    pub body: String,
}

pub fn parse_review_message(content: &str) -> anyhow::Result<ForgeReviewMessage> {
    let mut lines = content.lines();
    let title = lines.next().unwrap_or("").trim().to_string();

    if title.is_empty() {
        anyhow::bail!("Aborting due to empty PR title");
    }

    // Skip any leading blank lines after the title, then collect the rest as description
    let body = lines
        .skip_while(|l| l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    Ok(ForgeReviewMessage { title, body })
}

#[allow(clippy::too_many_arguments)]
async fn publish_review_for_branch(
    ctx: &mut Context,
    stack_id: Option<StackId>,
    branch_name: &str,
    target_branch: &str,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    default_message: bool,
    draft: bool,
    message: Option<&ForgeReviewMessage>,
) -> anyhow::Result<PublishReviewResult> {
    // Check if a review already exists for the branch.
    // If it does, skip publishing a new review.
    let existing_reviews = review_map.get(branch_name);
    if let Some(reviews) = existing_reviews
        && !reviews.is_empty()
    {
        return Ok(PublishReviewResult::AlreadyExists(reviews.clone()));
    }

    let commit = default_commit(ctx, stack_id, branch_name)?;
    let (title, body) = if let Some(message) = message {
        (message.title.clone(), message.body.clone())
    } else if default_message {
        let title = extract_commit_title(commit.as_ref())
            .map(|t| t.to_string())
            .unwrap_or(branch_name.to_string());
        let body = extract_commit_description(commit.as_ref())
            .map(|b| b.join("\n"))
            .unwrap_or_default();
        (title, body)
    } else {
        get_pr_title_and_body_from_editor(ctx, stack_id, commit.as_ref(), branch_name)?
    };

    // Publish a new review for the branch
    but_api::legacy::forge::publish_review(
        ctx.to_sync(),
        but_forge::CreateForgeReviewParams {
            title,
            body,
            source_branch: branch_name.to_string(),
            target_branch: target_branch.to_string(),
            draft,
        },
    )
    .await
    .map(|review| {
        if let Some(stack_id) = stack_id {
            let review_number = review.number.try_into().ok();
            but_api::legacy::stack::update_branch_pr_number(
                ctx,
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
    ctx: &Context,
    stack_id: Option<StackId>,
    branch_name: &str,
) -> Result<Option<Commit>, anyhow::Error> {
    let stack_details = but_api::legacy::workspace::stack_details(ctx, stack_id)?;
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
    ctx: &Context,
    stack_id: Option<StackId>,
    commit: Option<&Commit>,
    branch_name: &str,
) -> anyhow::Result<(String, String)> {
    let mut template = String::new();

    // Use the first line of the commit message as the default title if available
    let commit_title =
        extract_commit_title(commit).map(|s| s.replace(HTML_COMMENT_START_MARKER, "<\\!--"));
    if let Some(commit_title) = commit_title {
        template.push_str(&commit_title);
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
    } else if let Some(review_template) = but_api::legacy::forge::review_template(ctx)? {
        template.push_str(&review_template.content);
        template.push('\n');
    }

    // Add instructions as comments
    let mut instructions = format!(
        "
# Creating PR for branch: {branch_name}

Save and exit this file to create a PR for branch {branch_name} with the commits detailed below.

The FIRST LINE becomes the title. Leaving it empty ABORTS the operation.
EVERYTHING ELSE becomes the description.
HTML comments are stripped before submit.
"
    );

    // Add commit list with modified files as context
    if let Ok(stack_details) = but_api::legacy::workspace::stack_details(ctx, stack_id)
        && let Some(branch) = stack_details
            .branch_details
            .into_iter()
            .find(|h| h.name.to_str().unwrap_or("") == branch_name)
        && !branch.commits.is_empty()
    {
        instructions.push_str("\n# Commits in this PR:\n\n");

        // Get the repository for diff operations
        let repo = ctx.repo.get()?;
        for (idx, commit) in branch.commits.iter().enumerate() {
            // Extract commit title (first line of message)
            let commit_title = commit
                .message
                .lines()
                .next()
                .and_then(|l| l.to_str().ok())
                .unwrap_or("")
                // Note: Must strip away any comment end markers to prevent prematurely closing the
                // instructions comment
                .replace(HTML_COMMENT_END_MARKER, "--\\>");
            instructions.push_str(&format!(
                "{}. {} ({})\n",
                idx + 1,
                commit_title,
                commit.id.to_hex_with_len(7)
            ));

            // Get the files modified in this commit
            let parent = commit.parent_ids.first().copied();
            if let Ok(changes) = but_core::diff::TreeChanges::from_trees(&repo, parent, commit.id) {
                let mut changes: Vec<String> = changes
                    .0
                    .iter()
                    .map(|change| {
                        let tree_change: but_core::TreeChange = change.clone().into();
                        let status = match tree_change.status.kind() {
                            but_core::TreeStatusKind::Addition => "A",
                            but_core::TreeStatusKind::Deletion => "D",
                            but_core::TreeStatusKind::Modification => "M",
                            but_core::TreeStatusKind::Rename => "R",
                        };
                        format!("{status} {}", tree_change.path)
                    })
                    .collect();
                changes.sort();

                if !changes.is_empty() {
                    for change in changes.iter().take(10) {
                        instructions.push_str(&format!("    - {change}\n"));
                    }
                    if changes.len() > 10 {
                        instructions
                            .push_str(&format!("    ... and {} more files\n", changes.len() - 10));
                    }
                }
            }
        }
    }

    template.push_str(&format!("<!-- GITBUTLER INSTRUCTIONS{instructions}-->"));

    let content = get_text::from_editor("pr_message", &template, ".md")?.to_string();
    let content_without_comments = get_text::strip_html_comments(&content);
    let message = parse_review_message(&content_without_comments)?;
    Ok((message.title, message.body))
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
#[instrument(skip(ctx))]
pub fn get_review_map(
    ctx: &mut Context,
    cache_config: Option<but_forge::CacheConfig>,
) -> anyhow::Result<std::collections::HashMap<String, Vec<but_forge::ForgeReview>>> {
    let reviews = but_api::legacy::forge::list_reviews(ctx, cache_config).unwrap_or_default();
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
    branch_name: &BStr,
    pr_number: Option<usize>,
) -> Option<but_forge::ForgeReview> {
    review_map
        .get(&branch_name.to_string())
        .and_then(|rs| {
            pr_number
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

        format!(" ({review_numbers})").blue()
    } else if let Some(pr_number) = associated_review_number {
        format!(" (#{pr_number})").blue()
    } else {
        "".to_string().normal()
    }
}

/// Given a string selector, resolve the selection of review IDs to manipulate.
///
/// This function accepts one or a combination of:
/// - Review IDs (like PR or MR number), as long as they are associated with branches in the workspace.
/// - CLI IDs, as long as they are for branches or stacks.
fn resolve_review_selection(
    ctx: &mut Context,
    selector: Option<String>,
) -> anyhow::Result<Vec<usize>> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let applied_stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;
    let target_review_ids = if let Some(selector) = selector {
        // Extract any review IDs that match any of the associated reviews in the workspace.
        let review_ids = applied_stacks
            .iter()
            .flat_map(|stack| stack.review_ids())
            .collect::<Vec<_>>();
        let mut unique_review_ids = parse_review_ids(&selector, &review_ids);
        // Concatenate any review IDs associated with the selected CliIDs.
        unique_review_ids.extend(resolve_cli_ids_to_review_ids(
            ctx,
            &selector,
            &applied_stacks,
            &id_map,
        ));
        unique_review_ids.sort();
        unique_review_ids.dedup();
        unique_review_ids
    } else {
        interactive_review_id_selection(&applied_stacks)?
    };
    Ok(target_review_ids)
}

fn interactive_review_id_selection(
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
) -> anyhow::Result<Vec<usize>> {
    use cli_prompts::DisplayPrompt;

    #[derive(Debug, Clone)]
    struct BranchReview<'a> {
        branch_name: &'a str,
        review_id: usize,
    }

    impl From<BranchReview<'_>> for String {
        fn from(value: BranchReview) -> Self {
            format!("{} ({})", value.branch_name, value.review_id)
        }
    }

    let mut branch_reviews = vec![];
    for stack in applied_stacks {
        for head in &stack.heads {
            if let Some(review_id) = head.review_id {
                let branch_name = head.name.to_str()?;
                branch_reviews.push(BranchReview {
                    branch_name,
                    review_id,
                });
            }
        }
    }

    let review_selection_prompt = cli_prompts::prompts::Multiselect::new(
        "Please select the reviews you want to target.",
        branch_reviews.into_iter(),
    );

    let selected_reviews = review_selection_prompt
        .display()
        .map_err(|_| anyhow::anyhow!("Unable to determine which reviews to target"))?
        .iter()
        .map(|b| b.review_id)
        .collect::<Vec<_>>();

    Ok(selected_reviews)
}

fn resolve_cli_ids_to_review_ids(
    ctx: &mut Context,
    selector: &str,
    applied_stacks: &[but_workspace::legacy::ui::StackEntry],
    id_map: &IdMap,
) -> Vec<usize> {
    parse_sources(ctx, id_map, selector)
        .ok()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|cli_id| match cli_id {
            CliId::Branch { name, stack_id, .. } => applied_stacks
                .iter()
                .find_map(|stack| {
                    if stack.id == stack_id {
                        stack.review_for_head(&name)
                    } else {
                        None
                    }
                })
                .map(|r| vec![r]),
            CliId::Stack { stack_id, .. } => applied_stacks.iter().find_map(|stack| {
                if stack.id == Some(stack_id) {
                    Some(stack.review_ids())
                } else {
                    None
                }
            }),
            // Other selectors simply don't make sense here. I'm truly sorry.
            // No, but seriously: Only selecting branches or whole stacks make sense, for now.
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>()
}

fn parse_review_ids(selector: &str, review_ids: &[usize]) -> Vec<usize> {
    let valid_review_id_selectors = extract_valid_ids(selector);

    review_ids
        .iter()
        .cloned()
        .filter(|review_id| valid_review_id_selectors.contains(review_id))
        .collect::<Vec<_>>()
}

fn extract_valid_ids(selector: &str) -> Vec<usize> {
    selector
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pr_message_title_only() {
        let msg = parse_review_message("My PR Title").unwrap();
        assert_eq!(msg.title, "My PR Title");
        assert_eq!(msg.body, "");
    }

    #[test]
    fn parse_pr_message_title_and_body() {
        let msg = parse_review_message("My PR Title\n\nThis is the body.").unwrap();
        assert_eq!(msg.title, "My PR Title");
        assert_eq!(msg.body, "This is the body.");
    }

    #[test]
    fn parse_pr_message_multiline_body() {
        let msg = parse_review_message("Title\n\nLine 1\nLine 2\nLine 3").unwrap();
        assert_eq!(msg.title, "Title");
        assert_eq!(msg.body, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn parse_pr_message_skips_blank_lines_between_title_and_body() {
        let msg = parse_review_message("Title\n\n\n\nBody starts here").unwrap();
        assert_eq!(msg.title, "Title");
        assert_eq!(msg.body, "Body starts here");
    }

    #[test]
    fn parse_pr_message_trims_whitespace() {
        let msg = parse_review_message("  Title with spaces  \n\n  Body with spaces  ").unwrap();
        assert_eq!(msg.title, "Title with spaces");
        assert_eq!(msg.body, "Body with spaces");
    }

    #[test]
    fn parse_pr_message_empty_string_fails() {
        let result = parse_review_message("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty PR title"));
    }

    #[test]
    fn parse_pr_message_whitespace_only_fails() {
        let result = parse_review_message("   \n\n   ");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty PR title"));
    }

    #[test]
    fn parse_pr_message_blank_first_line_fails() {
        let result = parse_review_message("\nActual title on second line");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty PR title"));
    }

    #[test]
    fn extract_valid_ids_parses_comma_separated_numbers() {
        let ids = extract_valid_ids("1,2,3");
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn extract_valid_ids_trims_whitespace() {
        let ids = extract_valid_ids(" 1,  2 ,   3 ");
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn extract_valid_ids_ignores_invalid_entries() {
        let ids = extract_valid_ids("1,abc,3,-5,4.2,,0");
        assert_eq!(ids, vec![1, 3, 0]);
    }

    #[test]
    fn extract_valid_ids_empty_input_returns_empty_vec() {
        let ids = extract_valid_ids("");
        assert!(ids.is_empty());
    }

    #[test]
    fn parse_review_ids_returns_matching_ids_in_review_order() {
        let review_ids = vec![42, 7, 13, 99];

        let parsed = parse_review_ids("7,99", &review_ids);

        assert_eq!(parsed, vec![7, 99]);
    }

    #[test]
    fn parse_review_ids_ignores_non_matching_and_invalid_selector_values() {
        let review_ids = vec![1, 2, 3, 4];

        let parsed = parse_review_ids("2,abc,999,-1,4.2", &review_ids);

        assert_eq!(parsed, vec![2]);
    }

    #[test]
    fn parse_review_ids_handles_whitespace_and_duplicate_selector_ids() {
        let review_ids = vec![10, 20, 30];

        let parsed = parse_review_ids(" 20, 20 , 30 ", &review_ids);

        assert_eq!(parsed, vec![20, 30]);
    }

    #[test]
    fn parse_review_ids_empty_selector_returns_empty_vec() {
        let review_ids = vec![1, 2, 3];

        let parsed = parse_review_ids("", &review_ids);

        assert!(parsed.is_empty());
    }
}
