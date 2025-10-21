use colored::{ColoredString, Colorize};
use gitbutler_branch_actions::BranchListingFilter;
use gitbutler_project::Project;

pub async fn list(project: &Project, local: bool) -> Result<(), anyhow::Error> {
    let filter = if local {
        Some(BranchListingFilter {
            local: Some(true),
            ..Default::default()
        })
    } else {
        None
    };

    let reviews = but_api::forge::list_reviews_cmd(project.id)
        .await
        .unwrap_or_default();

    let branch_review_map = reviews
        .iter()
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

    let applied_stacks =
        but_api::workspace::stacks(project.id, Some(but_workspace::StacksFilter::InWorkspace))?;
    print_applied_branches(&applied_stacks, &branch_review_map);
    let branches = but_api::virtual_branches::list_branches(project.id, filter)?;
    let (branches, remote_only_branches): (Vec<_>, Vec<_>) =
        branches.into_iter().partition(|b| b.has_local);
    for branch in branches {
        // Skip branches that are part of applied stacks
        if let Some(stack_ref) = branch.stack
            && applied_stacks.iter().any(|s| s.id == Some(stack_ref.id))
        {
            continue;
        }

        let reviews = get_review_numbers(&branch.name.to_string(), &branch_review_map);

        println!("{} {}", branch.name, reviews);
    }

    for branch in remote_only_branches {
        let reviews = get_review_numbers(&branch.name.to_string(), &branch_review_map);
        println!("{} {} {}", "(remote)".dimmed(), branch.name, reviews);
    }
    Ok(())
}

fn get_review_numbers(
    branch_name: &String,
    branch_review_map: &std::collections::HashMap<
        String,
        Vec<&gitbutler_forge::review::ForgeReview>,
    >,
) -> ColoredString {
    if let Some(reviews) = branch_review_map.get(branch_name) {
        let review_numbers = reviews
            .iter()
            .map(|r| format!("{}{}", r.unit_symbol, r.number))
            .collect::<Vec<String>>()
            .join(", ");

        format!("({})", review_numbers).blue()
    } else {
        "".to_string().normal()
    }
}

fn print_applied_branches(
    applied_stacks: &[but_workspace::ui::StackEntry],
    branch_review_map: &std::collections::HashMap<
        String,
        Vec<&gitbutler_forge::review::ForgeReview>,
    >,
) {
    for stack in applied_stacks {
        let first_branch = stack.heads.first();
        let last_branch = stack.heads.last();
        for branch in &stack.heads {
            let is_single_branch = stack.heads.len() == 1;
            if is_single_branch {
                let branch_entry = format!("* {}", branch.name);
                let reviews = get_review_numbers(&branch.name.to_string(), branch_review_map);
                println!("{} {}", branch_entry.green(), reviews);
                continue;
            }

            let Some(first_branch) = first_branch else {
                continue;
            };

            let Some(last_branch) = last_branch else {
                continue;
            };

            let branch_entry = if branch.name == first_branch.name {
                format!("*- {}", branch.name)
            } else if branch.name == last_branch.name {
                format!("└─ {}", branch.name)
            } else {
                format!("├─ {}", branch.name)
            };

            let reviews = get_review_numbers(&branch.name.to_string(), branch_review_map);

            println!("{} {}", branch_entry.green(), reviews);
        }
    }
}
