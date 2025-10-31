use std::collections::BTreeMap;

use anyhow::Context;
use bstr::ByteSlice;
use but_api::forge::ListReviewsParams;
use but_settings::AppSettings;
use colored::{ColoredString, Colorize};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use serde::{Deserialize, Serialize};

pub async fn publish_reviews(
    project: &Project,
    branch: &Option<String>,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    json: bool,
) -> anyhow::Result<()> {
    let review_map = get_review_map(project).await?;
    let applied_stacks =
        but_api::workspace::stacks(project.id, Some(but_workspace::StacksFilter::InWorkspace))?;
    match branch {
        Some(branch_name) => {
            handle_specific_branch_publish(
                project,
                branch_name,
                &review_map,
                &applied_stacks,
                skip_force_push_protection,
                with_force,
                run_hooks,
                json,
            )
            .await
        }
        None => {
            handle_all_branches_in_workspace(
                project,
                &review_map,
                &applied_stacks,
                skip_force_push_protection,
                with_force,
                run_hooks,
                json,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_all_branches_in_workspace(
    project: &Project,
    review_map: &std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>,
    applied_stacks: &[but_workspace::ui::StackEntry],
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    json: bool,
) -> anyhow::Result<()> {
    let mut overall_outcome = PublishReviewsOutcome {
        published: vec![],
        already_existing: vec![],
    };
    for stack_entry in applied_stacks {
        let Some(top_head) = stack_entry.heads.first() else {
            // Should not happen, but just in case
            println!(
                "Stack entry '{}' has no heads, skipping",
                stack_entry
                    .id
                    .map(|id| id.to_string())
                    .unwrap_or("-no stack id-".to_string())
            );
            continue;
        };

        let outcome = publish_reviews_for_branch_and_dependents(
            project,
            top_head.name.to_str()?,
            review_map,
            stack_entry,
            skip_force_push_protection,
            with_force,
            run_hooks,
            json,
        )
        .await?;

        overall_outcome.published.extend(outcome.published);
        overall_outcome
            .already_existing
            .extend(outcome.already_existing);
    }

    if json {
        let outcome_json = serde_json::to_string_pretty(&overall_outcome)?;
        println!("{}", outcome_json);
    } else {
        println!();
        display_review_publication_summary(overall_outcome);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_specific_branch_publish(
    project: &Project,
    branch_name: &str,
    review_map: &std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>,
    applied_stacks: &[but_workspace::ui::StackEntry],
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    json: bool,
) -> anyhow::Result<()> {
    let Some(stack_entry) = applied_stacks
        .iter()
        .find(|entry| entry.heads.iter().any(|h| h.name == branch_name))
    else {
        anyhow::bail!(
            "Branch '{}' is not part of any applied stack in the workspace.",
            branch_name
        );
    };

    let outcome = publish_reviews_for_branch_and_dependents(
        project,
        branch_name,
        review_map,
        stack_entry,
        skip_force_push_protection,
        with_force,
        run_hooks,
        json,
    )
    .await?;

    if json {
        let outcome_json = serde_json::to_string_pretty(&outcome)?;
        println!("{}", outcome_json);
    } else {
        println!();
        display_review_publication_summary(outcome);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn publish_reviews_for_branch_and_dependents(
    project: &Project,
    branch_name: &str,
    review_map: &std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>,
    stack_entry: &but_workspace::ui::StackEntry,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    json: bool,
) -> Result<PublishReviewsOutcome, anyhow::Error> {
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    let all_branches_up_to_subject = stack_entry
        .heads
        .iter()
        .rev()
        .take_while(|h| h.name != branch_name)
        .collect::<Vec<_>>();

    if !json && !all_branches_up_to_subject.is_empty() {
        println!(
            "Pushing branch '{}' with {} dependent branch(es) first",
            branch_name,
            stack_entry.heads.len() - 1,
        );
    } else if !json {
        println!("Pushing branch '{}'", branch_name);
    }

    let result = but_api::stack::push_stack(
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

    if !json {
        println!("Push completed successfully");
        println!("Pushed to remote: {}", result.remote);
        if !result.branch_to_remote.is_empty() {
            for (branch, remote_ref) in &result.branch_to_remote {
                println!("  {} -> {}", branch, remote_ref);
            }
        }
        println!();
    }

    let mut newly_published = Vec::new();
    let mut already_existing = Vec::new();
    let mut current_target_branch = base_branch.short_name();
    for head in stack_entry.heads.iter().rev() {
        println!(
            "Publishing review for branch '{}' targetting '{}",
            head.name, current_target_branch
        );

        let published_review = publish_review_for_branch(
            project,
            head.name.to_str()?,
            current_target_branch,
            review_map,
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

/// Display a summary of published and already existing reviews
fn display_review_publication_summary(outcome: PublishReviewsOutcome) {
    // Group published reviews by branch name
    let mut published_by_branch: BTreeMap<&str, Vec<&gitbutler_forge::review::ForgeReview>> =
        BTreeMap::new();
    for review in &outcome.published {
        published_by_branch
            .entry(review.source_branch.as_str())
            .or_default()
            .push(review);
    }
    for (branch, reviews) in published_by_branch {
        println!("Published reviews for branch '{}':", branch);
        for review in reviews {
            print_review_information(review);
        }
    }

    // Group already existing reviews by branch name
    let mut existing_by_branch: BTreeMap<&str, Vec<&gitbutler_forge::review::ForgeReview>> =
        BTreeMap::new();
    for review in &outcome.already_existing {
        existing_by_branch
            .entry(review.source_branch.as_str())
            .or_default()
            .push(review);
    }
    for (branch, reviews) in existing_by_branch {
        println!("Review(s) already exist for branch '{}':", branch);
        for review in reviews {
            print_review_information(review);
        }
    }
}

/// Print review information in a formatted way
fn print_review_information(review: &gitbutler_forge::review::ForgeReview) {
    println!(
        "  '{}' ({}{}): {}",
        review.title.bold(),
        review.unit_symbol.blue(),
        review.number.to_string().blue(),
        review.html_url.underline()
    );
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishReviewsOutcome {
    published: Vec<gitbutler_forge::review::ForgeReview>,
    already_existing: Vec<gitbutler_forge::review::ForgeReview>,
}

enum PublishReviewResult {
    Published(Box<gitbutler_forge::review::ForgeReview>),
    AlreadyExists(Vec<gitbutler_forge::review::ForgeReview>),
}

async fn publish_review_for_branch(
    project: &Project,
    branch_name: &str,
    target_branch: &str,
    review_map: &std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>,
) -> anyhow::Result<PublishReviewResult> {
    // Check if a review already exists for the branch.
    // If it does, skip publishing a new review.
    let existing_reviews = review_map.get(branch_name);
    if let Some(reviews) = existing_reviews
        && !reviews.is_empty()
    {
        return Ok(PublishReviewResult::AlreadyExists(reviews.clone()));
    }

    // TODO: Determine title and body based on input/template/commits

    // Publish a new review for the branch
    but_api::forge::publish_review_cmd(but_api::forge::PublishReviewParams {
        project_id: project.id,
        params: gitbutler_forge::review::CreateForgeReviewParams {
            title: branch_name.to_string(),
            body: "".to_string(),
            source_branch: branch_name.to_string(),
            target_branch: target_branch.to_string(),
            draft: false,
        },
    })
    .await
    .map_err(Into::into)
    .map(|review| PublishReviewResult::Published(Box::new(review)))
}

pub async fn get_review_map(
    project: &Project,
) -> anyhow::Result<std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>> {
    let reviews = but_api::forge::list_reviews_cmd(ListReviewsParams {
        project_id: project.id,
    })
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
    branch_review_map: &std::collections::HashMap<
        String,
        Vec<gitbutler_forge::review::ForgeReview>,
    >,
) -> ColoredString {
    if let Some(reviews) = branch_review_map.get(branch_name) {
        let review_numbers = reviews
            .iter()
            .map(|r| format!("{}{}", r.unit_symbol, r.number))
            .collect::<Vec<String>>()
            .join(", ");

        format!(" ({})", review_numbers).blue()
    } else {
        "".to_string().normal()
    }
}
