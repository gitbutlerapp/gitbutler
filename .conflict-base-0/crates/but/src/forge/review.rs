use std::collections::BTreeMap;

use anyhow::Context;
use bstr::ByteSlice;
use but_api::forge::ListReviewsParams;
use but_settings::AppSettings;
use but_workspace::StackId;
use colored::{ColoredString, Colorize};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::OidExt;
use gitbutler_project::Project;
use serde::{Deserialize, Serialize};

use crate::{
    editor::get_text_from_editor_no_comments,
    ui::{SimpleBranch, SimpleStack},
};

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Publish review requests for active branches in your workspace.
    /// By default, publishes reviews for all active branches.
    Publish {
        /// Publish reviews only for the specified branch.
        #[clap(long, short = 'b')]
        branch: Option<String>,
        /// Force push even if it's not fast-forward (defaults to true).
        #[clap(long, short = 'f', default_value_t = true)]
        with_force: bool,
        /// Skip force push protection checks
        #[clap(long, short = 's')]
        skip_force_push_protection: bool,
        /// Run pre-push hooks (defaults to true).
        #[clap(long, short = 'r', default_value_t = true)]
        run_hooks: bool,
        /// Whether to use just the branch name as the review title, without opening an editor.
        #[clap(long, short = 't', default_value_t = false)]
        default: bool,
    },
}

pub async fn publish_reviews(
    project: &Project,
    branch: &Option<String>,
    skip_force_push_protection: bool,
    with_force: bool,
    run_hooks: bool,
    default: bool,
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
                default,
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
                default,
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
    default_message: bool,
    json: bool,
) -> anyhow::Result<()> {
    let mut overall_outcome = PublishReviewsOutcome {
        published: vec![],
        already_existing: vec![],
    };

    let simple_stacks = generate_simple_stacks(project, review_map, applied_stacks)?;

    // Run the branches selector UI to let the user choose which branches to publish reviews for.
    let selected_branches = crate::ui::run_branch_selector_ui(simple_stacks)?;

    if selected_branches.is_empty() {
        if !json {
            println!("No branches selected for review publication. Aborting.");
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

fn generate_simple_stacks(
    project: &Project,
    review_map: &std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>,
    applied_stacks: &[but_workspace::ui::StackEntry],
) -> Result<Vec<SimpleStack>, anyhow::Error> {
    let mut simple_stacks = vec![];
    let (base_branch, repo) = get_base_branch_and_repo(project)?;
    let base_branch_id = base_branch.current_sha.to_gix();
    for stack_entry in applied_stacks {
        let mut simple_stack = SimpleStack { branches: vec![] };
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

            let simple_branch = SimpleBranch {
                name: head.name.to_string(),
                commits: commits.into_iter().map(Into::into).collect(),
                reviews,
            };
            simple_stack.branches.push(simple_branch);
        }
        simple_stacks.push(simple_stack);
    }
    Ok(simple_stacks)
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
    default_message: bool,
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
        default_message,
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
    default_message: bool,
    json: bool,
) -> Result<PublishReviewsOutcome, anyhow::Error> {
    let (base_branch, _) = get_base_branch_and_repo(project)?;
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
            all_branches_up_to_subject.len()
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

fn get_base_branch_and_repo(
    project: &Project,
) -> Result<(gitbutler_branch_actions::BaseBranch, gix::Repository), anyhow::Error> {
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = CommandContext::open(project, app_settings)?;
    let repo = ctx.gix_repo()?;
    let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
    Ok((base_branch, repo))
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
    stack_id: Option<StackId>,
    branch_name: &str,
    target_branch: &str,
    review_map: &std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>,
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

    let (title, body) = if default_message {
        (branch_name.to_string(), String::new())
    } else {
        let title = get_review_title_from_editor(branch_name)?;
        let body = get_review_body_from_editor(&title)?;
        (title, body)
    };

    // Publish a new review for the branch
    but_api::forge::publish_review_cmd(but_api::forge::PublishReviewParams {
        project_id: project.id,
        params: gitbutler_forge::review::CreateForgeReviewParams {
            title,
            body,
            source_branch: branch_name.to_string(),
            target_branch: target_branch.to_string(),
            draft: false,
        },
    })
    .await
    .map_err(Into::into)
    .map(|review| {
        if let Some(stack_id) = stack_id {
            let review_number = review.number.try_into().ok();
            but_api::stack::update_branch_pr_number(
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

fn get_review_body_from_editor(title: &str) -> anyhow::Result<String> {
    let mut template = String::new();
    template.push_str("\n# This is the review description for:");
    template.push_str("\n# '");
    template.push_str(title);
    template.push_str("' \n");
    template.push_str("\n# Optionally, enter the review body above. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty body is allowed.\n");
    template.push_str("#\n");

    let body = get_text_from_editor_no_comments("but_review_body", &template)?;
    Ok(body)
}

fn get_review_title_from_editor(branch_name: &str) -> anyhow::Result<String> {
    let mut template = String::new();
    template.push_str(branch_name);
    template.push_str("\n# Please enter the review title above. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty title aborts the operation.\n");
    template.push_str("#\n");

    let title = get_text_from_editor_no_comments("but_review_title", &template)?;

    if title.is_empty() {
        anyhow::bail!("Aborting due to empty review title");
    }

    Ok(title)
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
