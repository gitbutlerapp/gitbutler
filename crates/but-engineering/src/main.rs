//! but-engineering binary entry point.
//!
//! A coordination system for coding agents working in the same repository.

use std::path::PathBuf;

use clap::Parser;

use but_engineering::args::{Args, HookEvent, Subcommands};
use but_engineering::command;
use but_engineering::db::DbHandle;
use but_engineering::session;
use but_engineering::types::ErrorResponse;

fn main() {
    let args = Args::parse();

    // Lurk takes over the terminal — handle it outside the JSON path.
    if matches!(args.cmd, Subcommands::Lurk) {
        let db_path = match find_db_path() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = command::lurk::execute(&db_path) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Unified hook dispatch: `eval <hook-event>` and backwards-compatible aliases.
    // All hooks are best-effort: any failure exits silently (code 0) so hooks
    // never block the user's session.
    let hook_event = match &args.cmd {
        Subcommands::Eval { hook } => Some(hook.clone()),
        Subcommands::EvalPrompt => Some(HookEvent::UserPromptSubmit),
        _ => None,
    };

    if let Some(event) = hook_event {
        run_hook(event);
        return;
    }

    match run(args) {
        Ok(json) => {
            println!("{json}");
        }
        Err(e) => {
            let error = ErrorResponse::new(e.to_string());
            let json = serde_json::to_string_pretty(&error)
                .unwrap_or_else(|_| format!(r#"{{"error": "{}"}}"#, e.to_string().replace('"', "\\\"")));
            eprintln!("{json}");
            std::process::exit(1);
        }
    }
}

/// Dispatch a hook event. Best-effort: any failure is logged to stderr
/// but the process always exits with code 0 so hooks never block the user's session.
fn run_hook(event: HookEvent) {
    // Parse stdin JSON once — some hooks need it, others discard.
    let input = command::hook_common::parse_stdin_json();

    // Distinguish "not a git repo" (silent exit) from "DB open failed" (log error).
    let db_path = match find_db_path() {
        Ok(p) => p,
        Err(_) => return, // Not a git repo or no working dir. Exit silently.
    };
    let db = match DbHandle::new_at_path(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("but-engineering: failed to open DB: {e:#}");
            return;
        }
    };

    let result = match event {
        HookEvent::UserPromptSubmit => command::eval_prompt::execute(&db),
        HookEvent::PreToolUse => command::eval_tool::execute(&db, &input),
    };

    if let Err(e) = result {
        eprintln!("but-engineering: hook {event:?} failed: {e:#}");
    }
}

fn run(args: Args) -> anyhow::Result<String> {
    let db_path = find_db_path()?;

    // Register session: map the Claude Code ancestor PID to this agent-id
    // so hooks can identify which agent they belong to.
    if let Some(agent_id) = args.cmd.agent_id()
        && let Some(claude_pid) = session::find_claude_ancestor()
        && let Ok(db) = DbHandle::new_at_path(&db_path)
    {
        let _ = db.register_session(claude_pid, agent_id, chrono::Utc::now());
    }

    match args.cmd {
        Subcommands::Post { content, agent_id } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let message = command::post::execute(&db, content, agent_id)?;
            Ok(serde_json::to_string_pretty(&message)?)
        }

        Subcommands::Read {
            agent_id,
            since,
            unread,
            wait,
            timeout,
        } => {
            let messages = command::read::execute(&db_path, agent_id, since, unread, wait, timeout)?;
            Ok(serde_json::to_string_pretty(&messages)?)
        }

        Subcommands::Status {
            agent_id,
            status_message,
            clear,
        } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let agent = command::status::execute(&db, agent_id, status_message, clear)?;
            Ok(serde_json::to_string_pretty(&agent)?)
        }

        Subcommands::Agents { active_within } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let agents = command::agents::execute(&db, active_within)?;
            Ok(serde_json::to_string_pretty(&agents)?)
        }

        Subcommands::Claim { paths, agent_id } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let claims = command::claim::execute(&db, paths, agent_id)?;
            Ok(serde_json::to_string_pretty(&claims)?)
        }

        Subcommands::Release { paths, agent_id, all } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let result = command::release::execute(&db, paths, agent_id, all)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }

        Subcommands::Claims { active_within } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let claims = command::claims::execute(&db, active_within)?;
            Ok(serde_json::to_string_pretty(&claims)?)
        }

        Subcommands::Check {
            file_path,
            agent_id,
            include_stack,
            intent_branch,
        } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let result = command::check::execute(&db, file_path, agent_id, include_stack, intent_branch)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }

        Subcommands::Plan {
            agent_id,
            plan_message,
            clear,
        } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let agent = command::plan::execute(&db, agent_id, plan_message, clear)?;
            Ok(serde_json::to_string_pretty(&agent)?)
        }

        Subcommands::Discover { content, agent_id } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let message = command::discover::execute(&db, content, agent_id)?;
            Ok(serde_json::to_string_pretty(&message)?)
        }

        Subcommands::Done { summary, agent_id } => {
            let db = DbHandle::new_at_path(&db_path)?;
            let result = command::done::execute(&db, summary, agent_id)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }

        Subcommands::Lurk | Subcommands::Eval { .. } | Subcommands::EvalPrompt => {
            unreachable!("handled before run()")
        }
    }
}

fn find_db_path() -> anyhow::Result<PathBuf> {
    let current_dir = std::env::current_dir()?;

    // Use gix to discover the git repository
    let repo = gix::discover(&current_dir).map_err(|e| anyhow::anyhow!("not a git repository: {e}"))?;

    let git_dir = repo.git_dir().to_path_buf();
    let db_dir = git_dir.join("gitbutler");

    if !db_dir.exists() {
        std::fs::create_dir_all(&db_dir)?;
    }

    Ok(db_dir.join("but-engineering.db"))
}
