use anyhow::{Context, Result};

mod args;
use args::{Args, CommandName, Subcommands, actions, claude, cursor};
use but_settings::AppSettings;
use metrics::{Event, Metrics, Props, metrics_if_configured};

use but_claude::hooks::OutputAsJson;
mod base;
mod branch;
mod command;
mod id;
mod init;
mod log;
mod mcp;
mod mcp_internal;
mod metrics;
mod rub;
mod status;

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
        Subcommands::Status { show_files } => {
            let result = status::worktree(&args.current_dir, args.json, *show_files);
            metrics_if_configured(app_settings, CommandName::Status, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Stf => {
            let result = status::worktree(&args.current_dir, args.json, true);
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
