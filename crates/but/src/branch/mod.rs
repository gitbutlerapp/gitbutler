use crate::LegacyProject;
use crate::utils::{Output, OutputFormat, into_json_value, we_need_proper_json_output_here};
use anyhow::bail;
use but_core::ref_metadata::StackId;
use but_settings::AppSettings;
use but_workspace::legacy::ui::StackEntry;
use gitbutler_command_context::CommandContext;
use std::io::{self, Write};

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

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Creates a new branch in the workspace
    New {
        /// Name of the new branch
        branch_name: Option<String>,
        /// Anchor point - either a commit ID or branch name to create the new branch from
        #[clap(long, short = 'a')]
        anchor: Option<String>,
    },
    /// Deletes a branch from the workspace
    #[clap(short_flag = 'd')]
    Delete {
        /// Name of the branch to delete
        branch_name: String,
        /// Force deletion without confirmation
        #[clap(long, short = 'f')]
        force: bool,
    },
    /// List the branches in the repository
    List {
        /// Filter branches by name (case-insensitive substring match)
        filter: Option<String>,
        /// Show only local branches
        #[clap(long, short = 'l', conflicts_with = "remote")]
        local: bool,
        /// Show only remote branches
        #[clap(long, short = 'r', conflicts_with = "local")]
        remote: bool,
        /// Show all branches (not just active + 20 most recent)
        #[clap(long, short = 'a')]
        all: bool,
        /// Don't calculate and show number of commits ahead of base (faster)
        #[clap(long)]
        no_ahead: bool,
        /// Fetch and display review information (PRs, MRs, etc.)
        #[clap(long)]
        review: bool,
        /// Don't check if each branch merges cleanly into upstream
        #[clap(long)]
        no_check: bool,
    },
    /// Show commits ahead of base for a specific branch
    Show {
        /// CLI ID or name of the branch to show
        branch_id: String,
        /// Fetch and display review information
        #[clap(short, long)]
        review: bool,
        /// Disable pager output
        #[clap(long)]
        no_pager: bool,
        /// Show files modified in each commit with line counts
        #[clap(short, long)]
        files: bool,
        /// Generate AI summary of the branch changes
        #[clap(long)]
        ai: bool,
        /// Check if the branch merges cleanly into upstream and identify conflicting commits
        #[clap(long)]
        check: bool,
    },
    /// Apply a branch to the workspace
    Apply {
        /// Name of the branch to apply
        branch_name: String,
    },
    /// Unapply a branch from the workspace
    Unapply {
        /// Name of the branch to unapply
        branch_name: String,
        /// Force unapply without confirmation
        #[clap(long, short = 'f')]
        force: bool,
    },
}

pub async fn handle(
    cmd: Option<Subcommands>,
    ctx: &but_ctx::Context,
    out: &mut Output,
    json: bool,
) -> anyhow::Result<serde_json::Value> {
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
                json,
                check,
            )
            .await?;
            Ok(we_need_proper_json_output_here())
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
                json,
                check,
            )
            .await?;
            Ok(we_need_proper_json_output_here())
        }
        Some(Subcommands::Show {
            branch_id,
            review,
            no_pager,
            files,
            ai,
            check,
        }) => {
            show::show(
                legacy_project,
                &branch_id,
                json,
                review,
                !no_pager,
                files,
                ai,
                check,
            )
            .await?;
            Ok(we_need_proper_json_output_here())
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
            let branch_name = branch_name
                .map(Ok::<_, but_api::error::Error>)
                .unwrap_or_else(|| but_api::workspace::canned_branch_name(legacy_project.id))?;

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
                        Some(but_api::stack::create_reference::Anchor::AtCommit {
                            commit_id: (*oid).into(),
                            position: but_workspace::branch::create_reference::Position::Above,
                        })
                    }
                    crate::id::CliId::Branch { name } => {
                        Some(but_api::stack::create_reference::Anchor::AtReference {
                            short_name: name.clone(),
                            position: but_workspace::branch::create_reference::Position::Above,
                        })
                    }
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
            but_api::stack::create_reference(
                legacy_project.id,
                but_api::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor,
                },
            )?;

            let json = json::BranchNewOutput {
                branch: branch_name.clone(),
                anchor: anchor_for_json,
            };
            match out.format {
                OutputFormat::Human => {
                    writeln!(out, "Created branch {branch_name}").ok();
                }
                OutputFormat::Shell => {
                    writeln!(out, "{branch_name}").ok();
                }
            }
            Ok(into_json_value(json))
        }
        Some(Subcommands::Delete { branch_name, force }) => {
            let stacks = but_api::workspace::stacks(
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
                    return confirm_branch_deletion(legacy_project, sid, &branch_name, force);
                }
            }

            writeln!(out, "Branch '{}' not found in any stack", branch_name).ok();
            Ok(we_need_proper_json_output_here())
        }
        Some(Subcommands::Apply { branch_name }) => {
            apply::apply(ctx, &branch_name, out).map(into_json_value)
        }
        Some(Subcommands::Unapply { branch_name, force }) => {
            let stacks = but_api::workspace::stacks(
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
                    return confirm_unapply_stack(legacy_project, sid, stack_entry, force);
                }
            }

            writeln!(out, "Branch '{}' not found in any stack", branch_name).ok();
            Ok(we_need_proper_json_output_here())
        }
    }
}

fn confirm_unapply_stack(
    project: &LegacyProject,
    sid: StackId,
    stack_entry: &StackEntry,
    force: bool,
) -> Result<serde_json::Value, anyhow::Error> {
    let mut stdout = io::stdout();
    let branches = stack_entry
        .heads
        .iter()
        .map(|head| head.name.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if !force {
        writeln!(
            stdout,
            "Are you sure you want to unapply stack with branches '{}'? [y/N]:",
            branches
        )
        .ok();

        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            bail!("Aborted unapply operation.");
        }
    }

    but_api::virtual_branches::unapply_stack(project.id, sid)?;
    writeln!(
        stdout,
        "Unapplied stack with branches '{}' from workspace",
        branches
    )
    .ok();
    Ok(we_need_proper_json_output_here())
}

fn confirm_branch_deletion(
    project: &LegacyProject,
    sid: StackId,
    branch_name: &str,
    force: bool,
) -> Result<serde_json::Value, anyhow::Error> {
    let mut stdout = io::stdout();
    if !force {
        writeln!(
            stdout,
            "Are you sure you want to delete branch '{}'? [y/N]:",
            branch_name
        )
        .ok();

        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            bail!("Aborted branch deletion.");
        }
    }

    but_api::stack::remove_branch(project.id, sid, branch_name.to_owned())?;
    writeln!(stdout, "Deleted branch {branch_name}").ok();
    Ok(we_need_proper_json_output_here())
}
