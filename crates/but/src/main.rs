use anyhow::{Context, Result};

mod args;
use args::{Args, BranchSubcommands, CommandName, Subcommands, actions, claude};
use but_settings::AppSettings;
use metrics::{Event, Metrics, Props, metrics_if_configured};

use but_claude::hooks::OutputAsJson;
mod branch;
mod command;
mod config;
mod id;
mod log;
mod mcp;
mod mcp_internal;
mod metrics;
mod oplog;
mod rub;
mod status;
mod undo;

#[tokio::main]
async fn main() -> Result<()> {
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
        Subcommands::Oplog => {
            let result = oplog::show_oplog(&args.current_dir, args.json);
            metrics_if_configured(app_settings, CommandName::Oplog, props(start, &result)).ok();
            result
        }
        Subcommands::Undo => {
            let result = undo::undo_last_operation(&args.current_dir, args.json);
            metrics_if_configured(app_settings, CommandName::Undo, props(start, &result)).ok();
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
