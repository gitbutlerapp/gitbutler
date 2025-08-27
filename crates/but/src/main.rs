use anyhow::{Context, Result};

mod args;
use args::{Args, BranchSubcommands, CommandName, Subcommands, actions, claude};
use but_settings::AppSettings;
use metrics::{Event, Metrics, Props, metrics_if_configured};

use but_claude::hooks::OutputAsJson;
mod branch;
mod command;
mod commit;
mod config;
mod describe;
mod id;
mod log;
mod mcp;
mod mcp_internal;
mod metrics;
mod new;
mod oplog;
mod restore;
mod rub;
mod status;
mod undo;

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
        Subcommands::Log { short } => {
            let result = log::commit_graph(&args.current_dir, args.json, *short);
            metrics_if_configured(app_settings, CommandName::Log, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Status { base } => {
            let result = status::worktree(&args.current_dir, args.json, *base);
            metrics_if_configured(app_settings, CommandName::Status, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Config => {
            let result = config::show(&args.current_dir, &app_settings, args.json);
            metrics_if_configured(app_settings, CommandName::Config, props(start, &result)).ok();
            result
        }
        Subcommands::Oplog { since } => {
            let result = oplog::show_oplog(&args.current_dir, args.json, since.as_deref());
            metrics_if_configured(app_settings, CommandName::Oplog, props(start, &result)).ok();
            result
        }
        Subcommands::Undo => {
            let result = undo::undo_last_operation(&args.current_dir, args.json);
            metrics_if_configured(app_settings, CommandName::Undo, props(start, &result)).ok();
            result
        }
        Subcommands::Restore { oplog_sha } => {
            let result = restore::restore_to_oplog(&args.current_dir, args.json, oplog_sha);
            metrics_if_configured(app_settings, CommandName::Restore, props(start, &result)).ok();
            result
        }
        Subcommands::Commit { message, stack } => {
            let result = commit::commit(
                &args.current_dir,
                args.json,
                message.as_deref(),
                stack.as_deref(),
            );
            metrics_if_configured(app_settings, CommandName::Commit, props(start, &result)).ok();
            result
        }
        Subcommands::New { target } => {
            let result = new::insert_blank_commit(&args.current_dir, args.json, target);
            metrics_if_configured(app_settings, CommandName::New, props(start, &result)).ok();
            result
        }
        Subcommands::Describe { commit } => {
            let result = describe::edit_commit_message(&args.current_dir, args.json, commit);
            metrics_if_configured(app_settings, CommandName::Describe, props(start, &result)).ok();
            result
        }
        Subcommands::Branch { cmd } => match cmd {
            BranchSubcommands::New { branch_name, id } => {
                branch::create_branch(&args.current_dir, args.json, branch_name, id.as_deref())
            }
        },
        Subcommands::Rub { source, target } => {
            let result = rub::handle(&args.current_dir, args.json, source, target)
                .context("Rubbed the wrong way.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            Ok(())
        }
    }
}

fn props<E, T, R>(start: std::time::Instant, result: R) -> Props
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
    println!("A GitButler CLI tool");
    println!();
    println!("Usage: but [OPTIONS] <COMMAND>");
    println!();
    println!("INSPECTION:");
    println!("  log       Provides an overview of the Workspace commit graph");
    println!("  status    Overview of the oncommitted changes in the repository");
    println!();
    println!("BRANCH OPERATIONS:");
    println!("  commit    Commit changes to a stack");
    println!("  rub       Combines two entities together to perform an operation");
    println!(
        "  new       Insert a blank commit before the specified commit, or at the top of a stack"
    );
    println!("  describe  Edit the commit message of the specified commit");
    println!("  branch    Branch management operations");
    println!();
    println!("OPERATION HISTORY:");
    println!("  oplog     Show operation history (last 20 entries)");
    println!("  undo      Undo the last operation by reverting to the previous snapshot");
    println!("  restore   Restore to a specific oplog snapshot");
    println!();
    println!("MISC:");
    println!("  config    Display configuration information about the GitButler repository");
    println!("  help      Print this message or the help of the given subcommand(s)");
    println!();
    println!("Options:");
    println!(
        "  -C, --current-dir <PATH>  Run as if gitbutler-cli was started in PATH instead of the current working directory [default: .]"
    );
    println!("  -j, --json                Whether to use JSON output format");
    println!("  -h, --help                Print help");
}
