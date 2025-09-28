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
mod rub;
mod status;

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
    gitbutler_secret::secret::set_application_namespace(namespace);
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
                command::handle_changes(&args.current_dir, args.json, handler, description)
            }
            None => command::list_actions(&args.current_dir, args.json, 0, 10),
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
            let result = base::handle(cmd, &args.current_dir, args.json);
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
            let result = branch::handle(cmd, &args.current_dir, args.json);
            metrics_if_configured(app_settings, CommandName::BranchNew, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Log => {
            let result = log::commit_graph(&args.current_dir, args.json);
            metrics_if_configured(app_settings, CommandName::Log, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Status {
            show_files,
            verbose,
        } => {
            let result = status::worktree(&args.current_dir, args.json, *show_files, *verbose);
            metrics_if_configured(app_settings, CommandName::Status, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Stf { verbose } => {
            let result = status::worktree(&args.current_dir, args.json, true, *verbose);
            metrics_if_configured(app_settings, CommandName::Stf, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Rub { source, target } => {
            let result = rub::handle(&args.current_dir, args.json, source, target)
                .context("Rubbed the wrong way.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Mark { target, delete } => {
            let result = mark::handle(&args.current_dir, args.json, target, *delete)
                .context("Can't mark this. Taaaa-na-na-na. Can't mark this.");
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
            let result = commit::commit(
                &args.current_dir,
                args.json,
                message.as_deref(),
                branch.as_deref(),
                *only,
            );
            metrics_if_configured(app_settings, CommandName::Commit, props(start, &result)).ok();
            result
        }
        Subcommands::New { target } => {
            let result = commit::insert_blank_commit(&args.current_dir, args.json, target);
            metrics_if_configured(app_settings, CommandName::New, props(start, &result)).ok();
            result
        }
        Subcommands::Describe { commit } => {
            let result = describe::edit_commit_message(&args.current_dir, args.json, commit);
            metrics_if_configured(app_settings, CommandName::Describe, props(start, &result)).ok();
            result
        }
        Subcommands::Oplog { since } => {
            let result = oplog::show_oplog(&args.current_dir, args.json, since.as_deref());
            metrics_if_configured(app_settings, CommandName::Oplog, props(start, &result)).ok();
            result
        }
        Subcommands::Restore { oplog_sha } => {
            let result = oplog::restore_to_oplog(&args.current_dir, args.json, oplog_sha);
            metrics_if_configured(app_settings, CommandName::Restore, props(start, &result)).ok();
            result
        }
        Subcommands::Undo => {
            let result = oplog::undo_last_operation(&args.current_dir, args.json);
            metrics_if_configured(app_settings, CommandName::Undo, props(start, &result)).ok();
            result
        }
        Subcommands::Init { repo } => init::repo(&args.current_dir, args.json, *repo)
            .context("Failed to initialize GitButler project."),
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
            vec!["commit", "rub", "new", "describe", "branch"],
        ),
        (
            "Operation History".yellow(),
            vec!["oplog", "undo", "restore"],
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
