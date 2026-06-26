use anyhow::Context as _;
use bstr::{BStr, ByteSlice};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_workspace::ui::Commit;
use gitbutler_project::Project;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    CliId, IdMap,
    id::parser::parse_sources,
    legacy::workspace::HeadInfoStack,
    theme::{self, Paint},
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

    let review_ids = resolve_review_selection(ctx, selector, out)?;

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

    let review_ids = resolve_review_selection(ctx, selector, out)?;

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
        let template_options = available_templates
            .into_iter()
            .map(|template| {
                let label = if let Some(current) = &current_template {
                    if template == current.path {
                        format!("{template} (current)")
                    } else {
                        template.clone()
                    }
                } else {
                    template.clone()
                };
                (label, template)
            })
            .collect::<Vec<_>>();
        let template_options = nonempty::NonEmpty::from_vec(template_options)
            .context("No review templates available")?;
        let selected_template = {
            let mut input = out
                .prepare_for_terminal_input()
                .context("Human input required - run this in a terminal")?;
            input
                .prompt_select(
                    "Select a review template (and press Enter)",
                    &template_options,
                )?
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Could not determine selected review template"))?
        };
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
#[expect(clippy::too_many_arguments)]
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

    // Publishing reviews is a write path, so use fresh forge state. A stale cache can
    // make stacked publication try to recreate a dependency review that already exists.
    let review_map = get_review_map_strict(ctx, Some(but_forge::CacheConfig::NoCache))?;
    let applied_stacks = crate::legacy::workspace::applied_stacks(ctx)?;

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
        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(
            ctx,
            ctx.shared_worktree_access().read_permission(),
        )?;
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
    applied_stacks: &[HeadInfoStack],
) -> anyhow::Result<Vec<String>> {
    let mut branches_without_prs = Vec::new();
    for stack_entry in applied_stacks {
        for branch in &stack_entry.branches {
            let branch_name = &branch.name;
            if !review_map.contains_key(branch_name)
                || review_map
                    .get(branch_name)
                    .map(|v| v.is_empty())
                    .unwrap_or(true)
            {
                // This means that there are no associated reviews that are open, but that doesn't mean that there are
                // no associated reviews.
                // Check whether there's an associated forge review.
                if branch.review_id.is_none() {
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
    let id_map = IdMap::legacy_new_from_context(&mut ctx, None)?;
    let branch_ids = id_map
        .parse_using_context(branch_id, &ctx)?
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

#[expect(clippy::too_many_arguments)]
pub async fn handle_multiple_branches_in_workspace(
    ctx: &mut Context,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[HeadInfoStack],
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
            .branches
            .iter()
            .find(|branch| selected_branches.contains(&branch.name))
        else {
            continue;
        };

        let outcome = publish_reviews_for_branch_and_dependents(
            ctx,
            &top_most_selected_head.name,
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

/// Prompt the user to select branches to publish.
fn prompt_for_branch_selection(
    ctx: &Context,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    applied_stacks: &[HeadInfoStack],
    out: &mut OutputChannel,
) -> anyhow::Result<Vec<String>> {
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(
        ctx,
        ctx.shared_worktree_access().read_permission(),
    )?;
    let base_branch_id = base_branch.current_sha;
    let repo = &*ctx.repo.get()?;

    let mut branch_options = Vec::new();
    for stack_entry in applied_stacks {
        for branch in &stack_entry.branches {
            let mut branch_ref = repo.find_reference(branch.reference.as_ref())?;
            let branch_id = branch_ref.peel_to_id()?;
            let commits = but_workspace::local_commits_for_branch(branch_id, base_branch_id)?;
            let reviews = review_map
                .get(&branch.name)
                .map(|reviews| {
                    reviews
                        .iter()
                        .map(|r| format!("{}{}", r.unit_symbol, r.number))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let review_str = if reviews.is_empty() {
                String::new()
            } else {
                format!(" ({})", reviews.join(", "))
            };
            branch_options.push((
                format!(
                    "{branch} - {num_commits} commit{plural}{review}",
                    branch = branch.name,
                    num_commits = commits.len(),
                    plural = if commits.len() == 1 { "" } else { "s" },
                    review = review_str,
                ),
                branch.name.clone(),
            ));
        }
    }

    let Some(branch_options) = nonempty::NonEmpty::from_vec(branch_options) else {
        if let Some(out) = out.for_human() {
            writeln!(out, "No branches available to publish.")?;
        }
        return Ok(vec![]);
    };

    let mut input = out
        .prepare_for_terminal_input()
        .context("Terminal input not available. Please specify branches to publish using command line arguments.")?;
    let selected_branches = input
        .prompt_multi_select("Select branches to publish", &branch_options)?
        .ok_or_else(|| anyhow::anyhow!("No branches selected. Aborting."))?
        .into_iter()
        .cloned()
        .collect();

    Ok(selected_branches)
}

#[expect(clippy::too_many_arguments)]
async fn publish_reviews_for_branch_and_dependents(
    ctx: &mut Context,
    branch_name: &str,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    stack_entry: &HeadInfoStack,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default_message: bool,
    draft: bool,
    message: Option<&ForgeReviewMessage>,
    out: &mut OutputChannel,
) -> Result<PublishReviewsOutcome, anyhow::Error> {
    let t = theme::get();
    let base_branch = {
        let guard = ctx.shared_worktree_access();
        gitbutler_branch_actions::base::get_base_branch_data(ctx, guard.read_permission())?
    };
    let all_branches_up_to_subject = stack_entry
        .branches
        .iter()
        .rev()
        .take_while(|branch| branch.name != branch_name)
        .collect::<Vec<_>>();

    if let Some(out) = out.for_human() {
        if !all_branches_up_to_subject.is_empty() {
            writeln!(
                out,
                "Pushing {} with {} dependent branch(es)...",
                t.local_branch.paint(branch_name),
                all_branches_up_to_subject.len()
            )?;
        } else {
            writeln!(out, "Pushing {}...", t.local_branch.paint(branch_name))?;
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
            t.sym().success,
            t.remote_branch.paint(&result.remote)
        )?;
    }

    let mut newly_published = Vec::new();
    let mut already_existing = Vec::new();
    let mut all_reviews_in_order = Vec::new();
    let mut current_target_branch = base_branch.short_name.as_str();
    for branch in stack_entry.branches.iter().rev() {
        if let Some(out) = out.for_human() {
            let draftiness = if draft { "draft " } else { "" };
            writeln!(
                out,
                "Creating {}review for {} {} {}...",
                draftiness,
                t.local_branch.paint(&branch.name),
                t.sym().arrow.info(),
                t.remote_branch.paint(current_target_branch)
            )?;
        }

        let message_plan =
            review_message_plan_for_branch(&branch.name, branch_name, default_message, message);
        let published_review = publish_review_for_branch(
            ctx,
            stack_entry.id,
            &branch.name,
            branch.review_id,
            current_target_branch,
            review_map,
            message_plan.default_message,
            draft,
            message_plan.message,
        )
        .await?;
        match published_review {
            PublishReviewResult::Published(review) => {
                newly_published.push(*review.clone());
                all_reviews_in_order.push((*review, current_target_branch.to_string()));
            }
            PublishReviewResult::AlreadyExists(reviews) => {
                if let Some(review) = reviews.first() {
                    all_reviews_in_order.push((review.clone(), current_target_branch.to_string()));
                }
                already_existing.extend(reviews);
            }
        }

        current_target_branch = &branch.name;

        if branch.name == branch_name {
            break;
        }
    }

    // Update footers and fix any drifted target branches in a single pass.
    let review_updates: Vec<but_forge::ForgeReviewUpdate> = all_reviews_in_order
        .into_iter()
        .map(|(review, expected_target)| {
            let mut update: but_forge::ForgeReviewUpdate = review.into();
            // Only send a target update when it has drifted.
            if update.target_branch.as_ref() == Some(&expected_target) {
                update.target_branch = None;
            } else {
                update.target_branch = Some(expected_target);
            }
            update
        })
        .collect();
    but_api::legacy::forge::update_review_footers(ctx.to_sync(), review_updates).await?;

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
    let t = theme::get();
    writeln!(
        out,
        "{} {} {}{}",
        t.sym().success,
        t.success.paint("Created review"),
        t.pr_number.paint(&review.unit_symbol),
        t.pr_number.paint(review.number.to_string())
    )?;
    writeln!(out, "  {} {}", t.hint.paint("Title:"), &review.title)?;
    writeln!(
        out,
        "  {} {}",
        t.hint.paint("Branch:"),
        t.remote_branch.paint(&review.source_branch)
    )?;
    writeln!(
        out,
        "  {} {}",
        t.hint.paint("URL:"),
        t.link.paint(&review.html_url)
    )?;
    if review.draft {
        writeln!(out, "  {}", t.hint.paint("Draft only"))?;
    }

    Ok(())
}

/// Print information about an existing PR
fn print_existing_pr_info(
    review: &but_forge::ForgeReview,
    out: &mut dyn std::fmt::Write,
) -> std::fmt::Result {
    let t = theme::get();
    writeln!(
        out,
        "{} {} {}{}",
        t.attention.paint("PR already exists for"),
        t.remote_branch.paint(&review.source_branch),
        t.pr_number.paint(&review.unit_symbol),
        t.pr_number.paint(review.number.to_string())
    )?;
    writeln!(
        out,
        "  {} {}",
        t.hint.paint("URL:"),
        t.link.paint(&review.html_url)
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

struct ReviewMessagePlan<'a> {
    default_message: bool,
    message: Option<&'a ForgeReviewMessage>,
}

fn review_message_plan_for_branch<'a>(
    branch_name: &str,
    selected_branch_name: &str,
    default_message: bool,
    selected_branch_message: Option<&'a ForgeReviewMessage>,
) -> ReviewMessagePlan<'a> {
    let is_selected_branch = branch_name == selected_branch_name;
    ReviewMessagePlan {
        default_message: default_message
            || (!is_selected_branch && selected_branch_message.is_some()),
        message: is_selected_branch
            .then_some(selected_branch_message)
            .flatten(),
    }
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

#[expect(clippy::too_many_arguments)]
async fn publish_review_for_branch(
    ctx: &mut Context,
    stack_id: Option<StackId>,
    branch_name: &str,
    associated_review_id: Option<usize>,
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
    if let Some(review_id) = associated_review_id
        && let Ok(review) = but_api::legacy::forge::get_review(ctx, review_id)
        && review.is_open()
        && review_source_branch_matches(&review, branch_name)
    {
        return Ok(PublishReviewResult::AlreadyExists(vec![review]));
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
    let commits = branch_commits(ctx, stack_id, branch_name)?;
    let commit = if commits.len() == 1 {
        commits.into_iter().next()
    } else {
        None
    };

    Ok(commit)
}

fn branch_commits(
    ctx: &Context,
    stack_id: Option<StackId>,
    branch_name: &str,
) -> anyhow::Result<Vec<Commit>> {
    let stacks = stack_id.map_or_else(
        || crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx),
        |stack_id| {
            crate::legacy::workspace::applied_stack_with_expensive_commit_info(ctx, Some(stack_id))
                .map(|stack| vec![stack])
        },
    )?;
    Ok(stacks
        .iter()
        .find_map(|stack| stack.branch(branch_name))
        .map(|branch| branch.commits.clone())
        .unwrap_or_default())
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
    if let Ok(commits) = branch_commits(ctx, stack_id, branch_name)
        && !commits.is_empty()
    {
        instructions.push_str("\n# Commits in this PR:\n\n");

        // Get the repository for diff operations
        let repo = ctx.repo.get()?;
        for (idx, commit) in commits.iter().enumerate() {
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

    let content = get_text::from_editor("pr_message", &template, None, ".md")?.to_string();
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
    ctx: &Context,
    cache_config: Option<but_forge::CacheConfig>,
) -> anyhow::Result<std::collections::HashMap<String, Vec<but_forge::ForgeReview>>> {
    let reviews = but_api::legacy::forge::list_reviews(ctx, cache_config).unwrap_or_default();
    Ok(review_map_from_reviews(reviews))
}

fn get_review_map_strict(
    ctx: &Context,
    cache_config: Option<but_forge::CacheConfig>,
) -> anyhow::Result<std::collections::HashMap<String, Vec<but_forge::ForgeReview>>> {
    let reviews = but_api::legacy::forge::list_reviews(ctx, cache_config)?;
    Ok(review_map_from_reviews(reviews))
}

fn review_map_from_reviews(
    reviews: Vec<but_forge::ForgeReview>,
) -> std::collections::HashMap<String, Vec<but_forge::ForgeReview>> {
    reviews
        .into_iter()
        .fold(std::collections::HashMap::new(), |mut acc, r| {
            let clean_branch_name = clean_review_source_branch(&r).to_string();
            acc.entry(clean_branch_name)
                .or_insert_with(Vec::new)
                .push(r);
            acc
        })
}

fn clean_review_source_branch(review: &but_forge::ForgeReview) -> &str {
    // GitHub can report forked PR heads as `owner:branch`.
    review
        .source_branch
        .split(':')
        .next_back()
        .unwrap_or(&review.source_branch)
}

fn review_source_branch_matches(review: &but_forge::ForgeReview, branch_name: &str) -> bool {
    clean_review_source_branch(review) == branch_name
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
) -> String {
    if let Some(reviews) = branch_review_map.get(branch_name) {
        let review_numbers = reviews
            .iter()
            .map(|r| format!("{}{}", r.unit_symbol, r.number))
            .collect::<Vec<String>>()
            .join(", ");

        format!(" ({review_numbers})")
    } else if let Some(pr_number) = associated_review_number {
        format!(" (#{pr_number})")
    } else {
        "".to_string()
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
    out: &mut OutputChannel,
) -> anyhow::Result<Vec<usize>> {
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;
    let applied_stacks = crate::legacy::workspace::applied_stacks(ctx)?;
    let target_review_ids = if let Some(selector) = selector {
        // Extract any review IDs that match any of the associated reviews in the workspace.
        let review_ids = applied_stacks
            .iter()
            .flat_map(review_ids_for_stack)
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
        interactive_review_id_selection(&applied_stacks, out)?
    };
    Ok(target_review_ids)
}

fn interactive_review_id_selection(
    applied_stacks: &[HeadInfoStack],
    out: &mut OutputChannel,
) -> anyhow::Result<Vec<usize>> {
    let branch_reviews = applied_stacks
        .iter()
        .flat_map(|stack| {
            stack.branches.iter().filter_map(|branch| {
                let review_id = branch.review_id?;
                Some((format!("{} ({})", branch.name, review_id), review_id))
            })
        })
        .collect::<Vec<_>>();
    let branch_reviews =
        nonempty::NonEmpty::from_vec(branch_reviews).context("No reviews available to select")?;
    let mut input = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal")?;

    let selected_reviews = input
        .prompt_multi_select(
            "Please select the reviews you want to target.",
            &branch_reviews,
        )?
        .ok_or_else(|| anyhow::anyhow!("Unable to determine which reviews to target"))?
        .into_iter()
        .copied()
        .collect::<Vec<_>>();

    Ok(selected_reviews)
}

fn resolve_cli_ids_to_review_ids(
    ctx: &mut Context,
    selector: &str,
    applied_stacks: &[HeadInfoStack],
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
                        stack.branch(&name).and_then(|branch| branch.review_id)
                    } else {
                        None
                    }
                })
                .map(|r| vec![r]),
            CliId::Stack { stack_id, .. } => applied_stacks.iter().find_map(|stack| {
                if stack.id == Some(stack_id) {
                    Some(review_ids_for_stack(stack).collect())
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

fn review_ids_for_stack(stack: &HeadInfoStack) -> impl Iterator<Item = usize> + '_ {
    stack.branches.iter().filter_map(|branch| branch.review_id)
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

    fn review_message() -> ForgeReviewMessage {
        ForgeReviewMessage {
            title: "Top branch title".to_string(),
            body: "Top branch body".to_string(),
        }
    }

    fn forge_review(source_branch: &str) -> but_forge::ForgeReview {
        but_forge::ForgeReview {
            html_url: "https://example.com/review/1".to_string(),
            number: 1,
            title: "Review".to_string(),
            body: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: source_branch.to_string(),
            target_branch: "main".to_string(),
            sha: "abc123".to_string(),
            integration_commit_shas: vec![],
            created_at: None,
            modified_at: None,
            merged_at: None,
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            head_repo_is_fork: false,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }

    #[test]
    fn clean_review_source_branch_strips_fork_owner_prefix() {
        let review = forge_review("contributor:feature");

        assert_eq!(clean_review_source_branch(&review), "feature");
    }

    #[test]
    fn review_source_branch_matches_normalized_branch_name() {
        let review = forge_review("contributor:feature");

        assert!(review_source_branch_matches(&review, "feature"));
        assert!(!review_source_branch_matches(&review, "other"));
    }

    #[test]
    fn review_map_from_reviews_keys_by_normalized_source_branch() {
        let reviews = review_map_from_reviews(vec![forge_review("contributor:feature")]);

        assert!(reviews.contains_key("feature"));
        assert!(!reviews.contains_key("contributor:feature"));
    }

    #[test]
    fn review_message_plan_uses_explicit_message_for_selected_branch() {
        let message = review_message();

        let plan = review_message_plan_for_branch("top", "top", false, Some(&message));

        assert!(!plan.default_message);
        assert_eq!(plan.message.unwrap().title, "Top branch title");
    }

    #[test]
    fn review_message_plan_defaults_dependencies_when_selected_branch_has_message() {
        let message = review_message();

        let plan = review_message_plan_for_branch("dependency", "top", false, Some(&message));

        assert!(plan.default_message);
        assert!(plan.message.is_none());
    }

    #[test]
    fn review_message_plan_keeps_interactive_dependency_behavior_without_message_or_default() {
        let plan = review_message_plan_for_branch("dependency", "top", false, None);

        assert!(!plan.default_message);
        assert!(plan.message.is_none());
    }

    #[test]
    fn review_message_plan_applies_default_flag_to_selected_branch() {
        let plan = review_message_plan_for_branch("top", "top", true, None);

        assert!(plan.default_message);
        assert!(plan.message.is_none());
    }

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
