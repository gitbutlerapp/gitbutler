use anyhow::bail;
pub use branch::{Platform, Subcommands};
use but_core::ref_metadata::StackId;
use but_settings::AppSettings;
use but_workspace::legacy::ui::StackEntry;
use gitbutler_command_context::CommandContext;

use crate::{LegacyProject, args::branch, utils::OutputChannel};

mod apply;
mod list;
mod show;

mod json {
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct BranchNewOutput {
        pub branch: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub anchor: Option<String>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BranchListOutput {
        pub applied_stacks: Vec<StackOutput>,
        pub branches: Vec<BranchOutput>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub more_branches: Option<usize>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StackOutput {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        pub heads: Vec<BranchHeadOutput>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BranchHeadOutput {
        pub name: String,
        pub reviews: Vec<ReviewOutput>,
        /// Last commit timestamp in milliseconds since epoch
        pub last_commit_at: u128,
        /// Number of commits ahead of the base branch
        pub commits_ahead: Option<usize>,
        pub last_author: AuthorOutput,
        /// Whether the branch merges cleanly into upstream
        #[serde(skip_serializing_if = "Option::is_none")]
        pub merges_cleanly: Option<bool>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BranchOutput {
        pub name: String,
        pub reviews: Vec<ReviewOutput>,
        pub has_local: bool,
        /// Last commit timestamp in milliseconds since epoch
        pub last_commit_at: u128,
        /// Number of commits ahead of the base branch
        pub commits_ahead: Option<usize>,
        pub last_author: AuthorOutput,
        /// Whether the branch merges cleanly into upstream
        #[serde(skip_serializing_if = "Option::is_none")]
        pub merges_cleanly: Option<bool>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AuthorOutput {
        pub name: Option<String>,
        pub email: Option<String>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ReviewOutput {
        pub number: u64,
        pub url: String,
    }
}

pub async fn handle(
    cmd: Option<Subcommands>,
    ctx: &but_ctx::Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let legacy_project = &ctx.legacy_project;
    match cmd {
        None => {
            let local = false;
            let remote = false;
            let all = false;
            let ahead = true; // Calculate ahead by default
            let review = false; // Don't fetch reviews by default
            let check = true; // Check merge by default
            list::list(
                legacy_project,
                local,
                remote,
                all,
                ahead,
                review,
                None,
                out,
                check,
            )
            .await?;
            Ok(())
        }
        Some(Subcommands::List {
            filter,
            local,
            remote,
            all,
            no_ahead,
            review,
            no_check,
        }) => {
            let ahead = !no_ahead; // Invert the flag
            let check = !no_check; // Invert the flag
            list::list(
                legacy_project,
                local,
                remote,
                all,
                ahead,
                review,
                filter,
                out,
                check,
            )
            .await?;
            Ok(())
        }
        Some(Subcommands::Show {
            branch_id,
            review,
            files,
            ai,
            check,
        }) => {
            show::show(legacy_project, &branch_id, out, review, files, ai, check).await?;
            Ok(())
        }
        Some(Subcommands::New {
            branch_name,
            anchor,
        }) => {
            let ctx = CommandContext::open(
                legacy_project,
                AppSettings::load_from_default_path_creating()?,
            )?;
            // Get branch name or use canned name
            let branch_name = branch_name.map(Ok).unwrap_or_else(|| {
                but_api::legacy::workspace::canned_branch_name(legacy_project.id)
            })?;

            // Store anchor string for JSON output
            let anchor_for_json = anchor.clone();

            let anchor = if let Some(anchor_str) = anchor {
                // Use the new create_reference API when anchor is provided
                let mut ctx = ctx; // Make mutable for CliId resolution

                // Resolve the anchor string to a CliId
                let anchor_ids = crate::id::CliId::from_str(&mut ctx, &anchor_str)?;
                if anchor_ids.is_empty() {
                    return Err(anyhow::anyhow!("Could not find anchor: {}", anchor_str));
                }
                if anchor_ids.len() > 1 {
                    return Err(anyhow::anyhow!(
                        "Ambiguous anchor '{}', matches multiple items",
                        anchor_str
                    ));
                }
                let anchor_id = &anchor_ids[0];

                // Create the anchor for create_reference
                // as dependent branch
                match anchor_id {
                    crate::id::CliId::Commit { oid } => {
                        Some(but_api::legacy::stack::create_reference::Anchor::AtCommit {
                            commit_id: (*oid).into(),
                            position: but_workspace::branch::create_reference::Position::Above,
                        })
                    }
                    crate::id::CliId::Branch { name, .. } => Some(
                        but_api::legacy::stack::create_reference::Anchor::AtReference {
                            short_name: name.clone(),
                            position: but_workspace::branch::create_reference::Position::Above,
                        },
                    ),
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Invalid anchor type: {}, expected commit or branch",
                            anchor_id.kind()
                        ));
                    }
                }
            } else {
                // Create an independent branch
                None
            };
            but_api::legacy::stack::create_reference(
                legacy_project.id,
                but_api::legacy::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor,
                },
            )?;

            if let Some(out) = out.for_human() {
                writeln!(out, "Created branch {branch_name}")?;
            } else if let Some(out) = out.for_shell() {
                writeln!(out, "{branch_name}")?;
            } else if let Some(out) = out.for_json() {
                let value = json::BranchNewOutput {
                    branch: branch_name.clone(),
                    anchor: anchor_for_json,
                };
                out.write_value(value)?;
            }
            Ok(())
        }
        Some(Subcommands::Delete { branch_name, force }) => {
            let stacks = but_api::legacy::workspace::stacks(
                legacy_project.id,
                Some(but_workspace::legacy::StacksFilter::InWorkspace),
            )?;

            // Find which stack this branch belongs to
            for stack_entry in &stacks {
                if stack_entry.heads.iter().all(|b| b.name != *branch_name) {
                    // Not found in this stack,
                    continue;
                }

                if let Some(sid) = stack_entry.id {
                    return confirm_branch_deletion(legacy_project, sid, &branch_name, force, out);
                }
            }

            if let Some(out) = out.for_human() {
                writeln!(out, "Branch '{}' not found in any stack", branch_name)?;
            }
            Ok(())
        }
        Some(Subcommands::Apply { branch_name }) => apply::apply(ctx, &branch_name, out),
        Some(Subcommands::Unapply { branch_name, force }) => {
            let stacks = but_api::legacy::workspace::stacks(
                legacy_project.id,
                Some(but_workspace::legacy::StacksFilter::InWorkspace),
            )?;

            // Find which stack this branch belongs to
            for stack_entry in &stacks {
                if stack_entry.heads.iter().all(|b| b.name != *branch_name) {
                    // Not found in this stack,
                    continue;
                }

                if let Some(sid) = stack_entry.id {
                    return confirm_unapply_stack(legacy_project, sid, stack_entry, force, out);
                }
            }

            if let Some(out) = out.for_human() {
                writeln!(out, "Branch '{}' not found in any stack", branch_name)?;
            }
            Ok(())
        }
    }
}

fn confirm_unapply_stack(
    project: &LegacyProject,
    sid: StackId,
    stack_entry: &StackEntry,
    force: bool,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    let branches = stack_entry
        .heads
        .iter()
        .map(|head| head.name.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if !force && out.for_human().is_some() {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        writeln!(
            stdout,
            "Are you sure you want to unapply stack with branches '{}'? [y/N]:",
            branches
        )?;

        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            bail!("Aborted unapply operation.");
        }
    }

    if force {
        but_api::legacy::virtual_branches::unapply_stack(project.id, sid)?;
    } else {
        bail!("Refusing to unapply stack without --force");
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Unapplied stack with branches '{}' from workspace",
            branches
        )?;
    }
    Ok(())
}

fn confirm_branch_deletion(
    project: &LegacyProject,
    sid: StackId,
    branch_name: &str,
    force: bool,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    if !force && out.for_human().is_some() {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        writeln!(
            stdout,
            "Are you sure you want to delete branch '{}'? [y/N]:",
            branch_name
        )?;

        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            bail!("Aborted branch deletion.");
        }
    }

    if force {
        but_api::legacy::stack::remove_branch(project.id, sid, branch_name.to_owned())?;
    } else {
        bail!(
            "Refusing to remove branch '{}' from workspace without --force",
            branch_name
        );
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "Deleted branch {branch_name}")?;
    }
    Ok(())
}
