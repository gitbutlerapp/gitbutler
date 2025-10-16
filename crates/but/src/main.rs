use anyhow::{Context, Result};

mod args;
use args::{Args, CommandName, Subcommands, actions, claude, cursor};
use but_settings::AppSettings;
use colored::Colorize;
use metrics::{Event, Metrics, Props, metrics_if_configured};

use but_claude::hooks::OutputAsJson;
mod base;
mod branch;
mod command;
mod commit;
mod describe;
mod id;
mod init;
mod log;
mod mark;
mod mcp;
mod mcp_internal;
mod metrics;
mod oplog;
mod push;
mod rub;
mod status;
mod worktree;

#[tokio::main]
async fn main() -> Result<()> {
    // Check if help is requested with no subcommand
    if std::env::args().len() == 1
        || std::env::args().any(|arg| arg == "--help" || arg == "-h") && std::env::args().len() == 2
    {
        print_grouped_help();
        return Ok(());
    }

    let args: Args = clap::Parser::parse();
    let app_settings = AppSettings::load_from_default_path_creating()?;

    let namespace = option_env!("IDENTIFIER").unwrap_or("com.gitbutler.app");
    but_secret::secret::set_application_namespace(namespace);
    let start = std::time::Instant::now();

    match &args.cmd {
        Subcommands::Mcp { internal } => {
            if *internal {
                mcp_internal::start(app_settings).await
            } else {
                mcp::start(app_settings).await
            }
        }
        Subcommands::Actions(actions::Platform { cmd }) => match cmd {
            Some(actions::Subcommands::HandleChanges {
                description,
                handler,
            }) => {
                let handler = *handler;
                let project = get_or_init_project(&args.current_dir)?;
                command::handle_changes(&project, args.json, handler, description)
            }
            None => {
                let project = get_or_init_project(&args.current_dir)?;
                command::list_actions(&project, args.json, 0, 10)
            }
        },
        Subcommands::Metrics {
            command_name,
            props,
        } => {
            let event = &mut Event::new((*command_name).into());
            if let Ok(props) = Props::from_json_string(props) {
                props.update_event(event);
            }
            Metrics::capture_blocking(&app_settings, event.clone()).await;
            Ok(())
        }
        Subcommands::Claude(claude::Platform { cmd }) => match cmd {
            claude::Subcommands::PreTool => {
                let result = but_claude::hooks::handle_pre_tool_call();
                let p = props(start, &result);
                result.out_json();
                metrics_if_configured(app_settings, CommandName::ClaudePreTool, p).ok();
                Ok(())
            }
            claude::Subcommands::PostTool => {
                let result = but_claude::hooks::handle_post_tool_call();
                let p = props(start, &result);
                result.out_json();
                metrics_if_configured(app_settings, CommandName::ClaudePostTool, p).ok();
                Ok(())
            }
            claude::Subcommands::Stop => {
                let result = but_claude::hooks::handle_stop().await;
                let p = props(start, &result);
                result.out_json();
                metrics_if_configured(app_settings, CommandName::ClaudeStop, p).ok();
                Ok(())
            }
            claude::Subcommands::PermissionPromptMcp => {
                but_claude::mcp::start(&args.current_dir).await
            }
        },
        Subcommands::Cursor(cursor::Platform { cmd }) => match cmd {
            cursor::Subcommands::AfterEdit => {
                let result = but_cursor::handle_after_edit().await;
                let p = props(start, &result);
                println!("{}", serde_json::to_string(&result?)?);
                metrics_if_configured(app_settings, CommandName::CursorStop, p).ok();
                Ok(())
            }
            cursor::Subcommands::Stop { nightly } => {
                let result = but_cursor::handle_stop(*nightly).await;
                let p = props(start, &result);
                println!("{}", serde_json::to_string(&result?)?);
                metrics_if_configured(app_settings, CommandName::CursorStop, p).ok();
                Ok(())
            }
        },
        Subcommands::Base(base::Platform { cmd }) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = base::handle(cmd, &project, args.json);
            metrics_if_configured(
                app_settings,
                match cmd {
                    base::Subcommands::Check => CommandName::BaseCheck,
                    base::Subcommands::Update => CommandName::BaseUpdate,
                },
                props(start, &result),
            )
            .ok();
            Ok(())
        }
        Subcommands::Branch(branch::Platform { cmd }) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = branch::handle(cmd, &project, args.json);
            metrics_if_configured(app_settings, CommandName::BranchNew, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Worktree(worktree::Platform { cmd }) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = worktree::handle(cmd, &project, args.json);
            metrics_if_configured(app_settings, CommandName::Worktree, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Log => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = log::commit_graph(&project, args.json);
            metrics_if_configured(app_settings, CommandName::Log, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Status {
            show_files,
            verbose,
        } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = status::worktree(&project, args.json, *show_files, *verbose);
            metrics_if_configured(app_settings, CommandName::Status, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Stf { verbose } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = status::worktree(&project, args.json, true, *verbose);
            metrics_if_configured(app_settings, CommandName::Stf, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Rub { source, target } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result =
                rub::handle(&project, args.json, source, target).context("Rubbed the wrong way.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Mark { target, delete } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = mark::handle(&project, args.json, target, *delete)
                .context("Can't mark this. Taaaa-na-na-na. Can't mark this.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Unmark => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = mark::unmark(&project, args.json)
                .context("Can't unmark this. Taaaa-na-na-na. Can't unmark this.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Commit {
            message,
            branch,
            only,
        } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = commit::commit(
                &project,
                args.json,
                message.as_deref(),
                branch.as_deref(),
                *only,
            );
            metrics_if_configured(app_settings, CommandName::Commit, props(start, &result)).ok();
            result
        }
        Subcommands::Push(push_args) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = push::handle(push_args, &project, args.json);
            metrics_if_configured(app_settings, CommandName::Push, props(start, &result)).ok();
            result
        }
        Subcommands::New { target } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = commit::insert_blank_commit(&project, args.json, target);
            metrics_if_configured(app_settings, CommandName::New, props(start, &result)).ok();
            result
        }
        Subcommands::Describe { target } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = describe::describe_target(&project, args.json, target);
            metrics_if_configured(app_settings, CommandName::Describe, props(start, &result)).ok();
            result
        }
        Subcommands::Oplog { since } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::show_oplog(&project, args.json, since.as_deref());
            metrics_if_configured(app_settings, CommandName::Oplog, props(start, &result)).ok();
            result
        }
        Subcommands::Restore { oplog_sha, force } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::restore_to_oplog(&project, args.json, oplog_sha, *force);
            metrics_if_configured(app_settings, CommandName::Restore, props(start, &result)).ok();
            result
        }
        Subcommands::Undo => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::undo_last_operation(&project, args.json);
            metrics_if_configured(app_settings, CommandName::Undo, props(start, &result)).ok();
            result
        }
        Subcommands::Snapshot { message } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::create_snapshot(&project, args.json, message.as_deref());
            metrics_if_configured(app_settings, CommandName::Snapshot, props(start, &result)).ok();
            result
        }
        Subcommands::Init { repo } => init::repo(&args.current_dir, args.json, *repo)
            .context("Failed to initialize GitButler project."),
    }
}

fn get_or_init_project(
    current_dir: &std::path::Path,
) -> anyhow::Result<gitbutler_project::Project> {
    let repo = gix::discover(current_dir)?;
    if let Some(path) = repo.workdir() {
        let project = match gitbutler_project::Project::find_by_path(path) {
            Ok(p) => Ok(p),
            Err(_e) => {
                crate::init::repo(path, false, false)?;
                gitbutler_project::Project::find_by_path(path)
            }
        }?;
        Ok(project)
    } else {
        let error_desc = "Bare repositories are not supported.";
        println!("{error_desc}");
        anyhow::bail!(error_desc);
    }
}

pub(crate) fn props<E, T, R>(start: std::time::Instant, result: R) -> Props
where
    R: std::ops::Deref<Target = Result<T, E>>,
    E: std::fmt::Display,
{
    let error = result.as_ref().err().map(|e| e.to_string());
    let mut props = Props::new();
    props.insert("durationMs", start.elapsed().as_millis());
    props.insert("error", error);
    props
}

fn print_grouped_help() {
    use clap::CommandFactory;
    use std::collections::HashSet;

    let cmd = Args::command();
    let subcommands: Vec<_> = cmd.get_subcommands().collect();

    // Define command groupings and their order (excluding MISC)
    let groups = [
        ("Inspection".yellow(), vec!["log", "status"]),
        (
            "Stack Operation".yellow(),
            vec!["commit", "push", "rub", "new", "describe", "branch"],
        ),
        (
            "Operation History".yellow(),
            vec!["oplog", "undo", "restore", "snapshot"],
        ),
    ];

    println!("{}", "The GitButler CLI change control system".red());
    println!();
    println!("Usage: but [OPTIONS] <COMMAND>");
    println!();

    // Keep track of which commands we've already printed
    let mut printed_commands = HashSet::new();

    // Print grouped commands
    for (group_name, command_names) in &groups {
        println!("{group_name}:");
        for cmd_name in command_names {
            if let Some(subcmd) = subcommands.iter().find(|c| c.get_name() == *cmd_name) {
                let about = subcmd.get_about().unwrap_or_default();
                println!("  {:<10}{about}", cmd_name.green());
                printed_commands.insert(cmd_name.to_string());
            }
        }
        println!();
    }

    // Collect any remaining commands not in the explicit groups
    let misc_commands: Vec<_> = subcommands
        .iter()
        .filter(|subcmd| !printed_commands.contains(subcmd.get_name()) && !subcmd.is_hide_set())
        .collect();

    // Print MISC section if there are any ungrouped commands
    if !misc_commands.is_empty() {
        println!("{}:", "Other Commands".yellow());
        for subcmd in misc_commands {
            let about = subcmd.get_about().unwrap_or_default();
            println!("  {:<10}{}", subcmd.get_name().green(), about);
        }
        println!();
    }

    println!("{}:", "Options".yellow());
    println!(
        "  -C, --current-dir <PATH>  Run as if but was started in PATH instead of the current working directory [default: .]"
    );
    println!("  -j, --json                Whether to use JSON output format");
    println!("  -h, --help                Print help");
}
