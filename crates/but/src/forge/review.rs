use anyhow::Context;
use but_api::forge::ListReviewsParams;
use but_settings::AppSettings;
use colored::{ColoredString, Colorize};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

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
            // TODO:
            anyhow::bail!("PUBLISHING ALL ACTIVE BRANCHES NOT IMPLEMENTED YET");
        }
    }
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

    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(project, app_settings)?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;

    if stack_entry.heads.len() > 1 {
        // TODO:
        anyhow::bail!("PUBLISHING MULTIPLE HEADS NOT SUPPORTED YET",);
    }

    if !json {
        println!(
            "Publishing review for branch '{}' targeting base branch '{}'",
            branch_name,
            base_branch.short_name()
        );
    }

    // Call push_stack
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
    }

    let published_review =
        publish_review_for_branch(project, branch_name, base_branch.short_name(), review_map)
            .await?;

    match published_review {
        PublishReviewResult::Published(review) if json => {
            let review_json = serde_json::to_string_pretty(&vec![review])?;
            println!("{}", review_json);
        }
        PublishReviewResult::Published(review) => {
            println!(
                "Published review {}{} for branch '{}': {}",
                review.unit_symbol, review.number, branch_name, review.html_url
            );
        }
        PublishReviewResult::AlreadyExists(reviews) if json => {
            let review_json = serde_json::to_string_pretty(&reviews)?;
            println!("{}", review_json);
        }
        PublishReviewResult::AlreadyExists(reviews) => {
            if reviews.len() > 1 {
                println!(
                    "Multiple reviews already exist for branch '{}':",
                    branch_name
                );
                for review in reviews {
                    println!(
                        "- {}{}: {}",
                        review.unit_symbol, review.number, review.html_url
                    );
                }
            } else if let Some(review) = reviews.first() {
                println!(
                    "A review already exists for branch '{}': {}{}: {}",
                    branch_name, review.unit_symbol, review.number, review.html_url
                );
            }
        }
    }

    Ok(())
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
