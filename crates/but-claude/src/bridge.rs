//! Claude bridge.
//!
//! The goal of this module is to provide the frontend with a way of talking
//! claude code.
//!
//! There have been three different methods for building this proposed:
//!
//! Streamed input & output
//! - This might give us a little bit more control and have the ability to send
//!   stop signals that are more graceful than just aborting the process.
//! - This does require the management of long lived child processes.
//! - **This is currently broken**
//!
//! Streamed output
//! - It would be curious how this plays into features like queuing multiple
//!   messages.
//!
//! Streamed output and managing tool call output
//! - This might give us more flexabiity in the long run, but initially seems
//!   more complex with more unknowns.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, PipeReader, Read as _},
    process::ExitStatus,
    sync::Arc,
};

use anyhow::{Result, bail};
use but_action::cli::get_cli_path;
use but_core::ref_metadata::StackId;
use but_ctx::{Context, ThreadSafeContext};
use gitbutler_stack::VirtualBranchesHandle;
use gix::bstr::ByteSlice;
use serde::Serialize;
use tokio::{
    fs,
    process::{Child, Command},
    sync::{
        Mutex,
        mpsc::{UnboundedSender, unbounded_channel},
    },
};

use crate::{
    Broadcaster, ClaudeMessage, ClaudeOutput, ClaudeUserParams, MessagePayload, PermissionMode,
    PromptAttachment, SystemMessage, ThinkingLevel, Transcript, UserInput,
    broadcaster::FrontendEvent,
    claude_config::fmt_claude_settings,
    claude_mcp::{BUT_SECURITY_MCP, ClaudeMcpConfig},
    claude_settings::ClaudeSettings,
    db::{self, list_messages_by_session},
    rules::{create_claude_assignment_rule, list_claude_assignment_rules},
    send_claude_message,
};

/// Holds the CC instances. Currently keyed by stackId, since our current model
/// assumes one CC per stack at any given time.
pub struct Claudes {
    /// A set that contains all the currently running requests
    pub(crate) requests: Mutex<HashMap<StackId, Arc<Claude>>>,
}

pub struct Claude {
    pub(crate) kill: UnboundedSender<()>,
}

impl Claudes {
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
        }
    }

    pub async fn send_message(
        &self,
        ctx: ThreadSafeContext,
        broadcaster: Arc<Mutex<Broadcaster>>,
        stack_id: StackId,
        user_params: ClaudeUserParams,
    ) -> Result<()> {
        if self.requests.lock().await.contains_key(&stack_id) {
            bail!(
                "Claude is currently thinking, please wait for it to complete before sending another message.\n\nIf claude is stuck thinking, try restarting the application."
            );
        } else {
            self.spawn_claude(ctx.clone(), broadcaster.clone(), stack_id, user_params)
                .await;
            let _ = self.maybe_compact_context(ctx, broadcaster, stack_id).await;
        };

        Ok(())
    }

    pub async fn compact_history(
        &self,
        ctx: ThreadSafeContext,
        broadcaster: Arc<Mutex<Broadcaster>>,
        stack_id: StackId,
    ) -> Result<()> {
        if self.requests.lock().await.contains_key(&stack_id) {
            bail!(
                "Claude is currently thinking, please wait for it to complete before sending another message.\n\nIf claude is stuck thinking, try restarting the application."
            )
        } else {
            self.compact(ctx, broadcaster, stack_id).await
        };

        Ok(())
    }

    pub fn get_messages(&self, ctx: &mut Context, stack_id: StackId) -> Result<Vec<ClaudeMessage>> {
        let rule = list_claude_assignment_rules(ctx)?
            .into_iter()
            .find(|rule| rule.stack_id == stack_id);
        if let Some(rule) = rule {
            let messages = db::list_messages_by_session(ctx, rule.session_id)?;
            Ok(messages)
        } else {
            Ok(vec![])
        }
    }

    /// Cancel a running Claude session for the given stack
    pub async fn cancel_session(&self, stack_id: StackId) -> Result<bool> {
        let requests = self.requests.lock().await;
        if let Some(claude) = requests.get(&stack_id) {
            // Send the kill signal
            claude
                .kill
                .send(())
                .map_err(|_| anyhow::anyhow!("Failed to send kill signal"))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if there is an active Claude session for the given stack ID
    pub async fn is_stack_active(&self, stack_id: StackId) -> bool {
        let requests = self.requests.lock().await;
        requests.contains_key(&stack_id)
    }

    async fn spawn_claude(
        &self,
        sync_ctx: ThreadSafeContext,
        broadcaster: Arc<Mutex<Broadcaster>>,
        stack_id: StackId,
        user_params: ClaudeUserParams,
    ) -> () {
        let res = self
            .spawn_claude_inner(sync_ctx.clone(), broadcaster.clone(), stack_id, user_params)
            .await;
        if let Err(res) = res {
            self.requests.lock().await.remove(&stack_id);

            let rule = {
                let mut ctx = sync_ctx.clone().into_thread_local();
                list_claude_assignment_rules(&mut ctx)
                    .ok()
                    .and_then(|rules| rules.into_iter().find(|rule| rule.stack_id == stack_id))
            };

            if let Some(rule) = rule {
                let _ = send_claude_message(
                    sync_ctx,
                    broadcaster.clone(),
                    rule.session_id,
                    stack_id,
                    MessagePayload::System(crate::SystemMessage::UnhandledException {
                        message: format!("{res}"),
                    }),
                )
                .await;
            }
        };
    }

    async fn spawn_claude_inner(
        &self,
        sync_ctx: ThreadSafeContext,
        broadcaster: Arc<Mutex<Broadcaster>>,
        stack_id: StackId,
        user_params: ClaudeUserParams,
    ) -> Result<()> {
        // Capture the start time to filter messages created during this session
        let session_start_time = chrono::Utc::now().naive_utc();

        let (send_kill, mut recv_kill) = unbounded_channel();
        self.requests
            .lock()
            .await
            .insert(stack_id, Arc::new(Claude { kill: send_kill }));

        // We're also making the bold assumption that if we can find the
        // transcript, that a session was created. This is _not_ the best
        // way to do this.
        //
        // https://github.com/anthropics/claude-code/issues/5161 could
        // simplify this
        let (summary_to_resume, session_id, session) = {
            let mut ctx = sync_ctx.clone().into_thread_local();
            let rule = {
                list_claude_assignment_rules(&mut ctx)?
                    .into_iter()
                    .find(|rule| rule.stack_id == stack_id)
            };

            let session_id = rule.map(|r| r.session_id).unwrap_or(uuid::Uuid::new_v4());

            let session = upsert_session(&mut ctx, session_id, stack_id)?;
            let mut ctx = sync_ctx.clone().into_thread_local();
            let messages = list_messages_by_session(&mut ctx, session.id)?;

            let summary = if let Some(ClaudeMessage { payload, .. }) = messages.last() {
                match payload {
                    MessagePayload::System(SystemMessage::CompactFinished { summary }) => {
                        Some(summary.clone())
                    }
                    _ => None,
                }
            } else {
                None
            };
            (summary, session_id, session)
        };

        // Store the original message for UI display (without inlined file content)
        // while Claude gets the enhanced message with file content inlined
        send_claude_message(
            sync_ctx.clone(),
            broadcaster.clone(),
            session_id,
            stack_id,
            MessagePayload::User(UserInput {
                message: user_params.message.clone(), // Original user message for display
                attachments: user_params.attachments.clone(),
            }),
        )
        .await?;

        let (read_stdout, writer) = std::io::pipe()?;
        let response_streamer = spawn_response_streaming(
            sync_ctx.clone(),
            broadcaster.clone(),
            read_stdout,
            session_id,
            stack_id,
        );

        let (read_stderr, write_stderr) = std::io::pipe()?;
        // Clone so the reference to ctx can be immediately dropped
        let project_workdir = sync_ctx.legacy_project.worktree_dir()?.to_owned();
        let mut handle = spawn_command(
            writer,
            write_stderr,
            session,
            project_workdir,
            sync_ctx.clone(),
            user_params,
            summary_to_resume,
            stack_id,
        )
        .await?;
        let cmd_exit = tokio::select! {
            status = handle.wait() => Exit::WithStatus(status),
            _ = recv_kill.recv() => Exit::ByUser
        };
        // My understanding is that it is not great to abort things like this,
        // but it's "good enough" for now.
        response_streamer.abort();
        self.requests.lock().await.remove(&stack_id);

        handle_exit(
            sync_ctx.clone(),
            broadcaster.clone(),
            stack_id,
            session_id,
            read_stderr,
            handle,
            cmd_exit,
        )
        .await?;

        // Broadcast system any messages created during this Claude session
        // (e.g., commit created notification from the Stop hook)
        let project_id = sync_ctx.legacy_project.id;
        let all_messages = {
            let mut ctx = sync_ctx.clone().into_thread_local();
            db::list_messages_by_session(&mut ctx, session_id)
        };
        if let Ok(all_messages) = all_messages {
            let new_messages: Vec<_> = all_messages
                .into_iter()
                .filter(|msg| matches!(msg.payload, MessagePayload::GitButler(_)))
                .filter(|msg| msg.created_at > session_start_time)
                .collect();

            // Broadcast each new message
            for message in new_messages {
                broadcaster.lock().await.send(FrontendEvent {
                    name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
                    payload: serde_json::json!(message),
                });
            }
        }

        // Send completion notification
        if let Err(e) = crate::notifications::notify_completion(&sync_ctx.settings) {
            tracing::warn!("Failed to send completion notification: {}", e);
        }

        Ok(())
    }
}

async fn handle_exit(
    ctx: ThreadSafeContext,
    broadcaster: Arc<Mutex<Broadcaster>>,
    stack_id: StackId,
    session_id: uuid::Uuid,
    mut read_stderr: PipeReader,
    mut handle: Child,
    cmd_exit: Exit,
) -> Result<(), anyhow::Error> {
    match cmd_exit {
        Exit::WithStatus(exit_status) => {
            let exit_status = exit_status?;
            let mut buf = String::new();
            read_stderr.read_to_string(&mut buf)?;
            send_claude_message(
                ctx,
                broadcaster.clone(),
                session_id,
                stack_id,
                MessagePayload::System(crate::SystemMessage::ClaudeExit {
                    code: exit_status.code().unwrap_or(0),
                    message: buf.clone(),
                }),
            )
            .await?;
        }
        Exit::ByUser => {
            // On *nix try to kill claude more gently.
            #[cfg(unix)]
            {
                use nix::{
                    sys::signal::{self, Signal},
                    unistd::Pid,
                };
                if let Some(pid) = handle.id() {
                    signal::kill(Pid::from_raw(pid as i32), Signal::SIGINT)?;
                    handle.wait().await?;
                } else {
                    handle.kill().await?;
                }
            }
            #[cfg(not(unix))]
            {
                handle.kill().await?;
            }
            send_claude_message(
                ctx,
                broadcaster.clone(),
                session_id,
                stack_id,
                MessagePayload::System(crate::SystemMessage::UserAbort),
            )
            .await?;
        }
    }
    Ok(())
}

enum Exit {
    WithStatus(std::io::Result<ExitStatus>),
    ByUser,
}

/// Spawns the actual claude code command
#[allow(clippy::too_many_arguments)]
async fn spawn_command(
    writer: std::io::PipeWriter,
    write_stderr: std::io::PipeWriter,
    session: crate::ClaudeSession,
    project_path: std::path::PathBuf,
    sync_ctx: ThreadSafeContext,
    user_params: ClaudeUserParams,
    summary_to_resume: Option<String>,
    stack_id: StackId,
) -> Result<Child> {
    // Write and obtain our own claude hooks path.
    let settings = fmt_claude_settings()?;

    let claude_executable = sync_ctx.settings.claude.executable.clone();
    let cc_settings = ClaudeSettings::open(&project_path).await;

    // Determine what session ID Claude will use - needed for MCP server configuration
    let transcript_current_id =
        Transcript::current_valid_session_id(&project_path, &session).await?;
    let claude_session_id = if summary_to_resume.is_some() {
        // If resuming after compaction, Claude will use a new random ID
        uuid::Uuid::new_v4()
    } else if let Some(current_id) = transcript_current_id {
        // If resuming, Claude will use the existing current_id
        current_id
    } else {
        // If starting new, Claude will use the stable session.id
        session.id
    };

    let mcp_config = ClaudeMcpConfig::open(&cc_settings, &project_path).await;
    let disabled_mcp_servers = user_params
        .disabled_mcp_servers
        .iter()
        .filter(|f| *f != BUT_SECURITY_MCP)
        .map(String::as_str)
        .collect::<Vec<&str>>();
    let mcp_config = &mcp_config
        .mcp_servers_with_security(claude_session_id)
        .exclude(&disabled_mcp_servers);
    tracing::info!(
        "spawn_command mcp_servers: {:?}",
        mcp_config.mcp_servers.keys()
    );
    let mcp_config = serde_json::to_string(mcp_config)?;
    let mut command = Command::new(claude_executable);

    // Don't create a terminal window on windows.
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.stdout(writer);
    command.stderr(write_stderr);
    command.current_dir(&project_path);

    command.envs(cc_settings.env());

    command.args(["--settings", &settings]);

    // Mcp configuration. We now use --strict-mcp-config because we collect the
    // set of MCP configurations ourselves so we can then filter out ones that
    // we don't want in a given call.
    command.args(["--mcp-config", &mcp_config]);
    command.args(["--strict-mcp-config"]);

    command.args(["--output-format", "stream-json"]);

    // Only add --model if useConfiguredModel is false
    if !sync_ctx.settings.clone().claude.use_configured_model {
        command.args(["--model", user_params.model.to_cli_string()]);
    }

    command.args(["--verbose"]);

    if sync_ctx
        .settings
        .clone()
        .claude
        .dangerously_allow_all_permissions
    {
        command.arg("--dangerously-skip-permissions");
    } else {
        command.args([
            "--permission-prompt-tool",
            "mcp__but-security__approval_prompt",
        ]);
        // Set permission mode based on interaction mode
        match user_params.permission_mode {
            PermissionMode::Default => {
                command.args(["--permission-mode", "default"]);
            }
            PermissionMode::Plan => {
                command.args(["--permission-mode", "plan"]);
            }
            PermissionMode::AcceptEdits => {
                command.args(["--permission-mode", "acceptEdits"]);
            }
        };
    }

    // Pass the session ID to Claude Code
    // We've already determined claude_session_id earlier based on whether we're resuming or starting new
    if summary_to_resume.is_some() {
        // After compaction, start with a new session ID
        command.args(["--session-id", &format!("{}", claude_session_id)]);
    } else if transcript_current_id.is_some() {
        // Resume existing session
        command.args(["--resume", &format!("{}", claude_session_id)]);
    } else {
        // Start new session - ensure there isn't an existing invalid transcript
        let path = Transcript::get_transcript_path(&project_path, session.id)?;
        if fs::try_exists(&path).await? {
            fs::remove_file(&path).await?;
        }
        command.args(["--session-id", &format!("{}", claude_session_id)]);
    }

    // Format branch information for the system prompt
    let branch_info = {
        let mut ctx = sync_ctx.clone().into_thread_local();
        format_branch_info(&mut ctx, stack_id)
    };
    let system_prompt = format!("{}\n\n{}", system_prompt(), branch_info);
    command.args(["--append-system-prompt", &system_prompt]);

    if !user_params.add_dirs.is_empty() {
        command.arg("--add-dir");
        command.args(user_params.add_dirs);
    }

    command.arg("-p");

    if user_params.message.starts_with("/") {
        command.arg(&user_params.message);
    } else {
        let message = if let Some(attachments) = &user_params.attachments {
            format_message_with_attachments(&user_params.message, attachments).await?
        } else {
            user_params.message
        };

        if let Some(summary_to_resume) = summary_to_resume {
            command.arg(format_message_with_summary(
                &summary_to_resume,
                &message,
                user_params.thinking_level,
            ));
        } else {
            command.arg(format_message(&message, user_params.thinking_level));
        }
    }
    tracing::info!("spawn_command: {:?}", command);
    Ok(command.spawn()?)
}

fn system_prompt() -> String {
    let but_path = get_cli_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "but".to_string());

    format!(
        "<git-usage>
CRITICAL: You are working on a project that is managed by GitButler.

## General Principle

When working with commits (creating, modifying, or reorganizing), ALWAYS use the `{0}` CLI.
Only use git commands for READ-ONLY operations like viewing history, diffs, or logs.

## PROHIBITED Git Commands

You MUST NOT run the following git commands:
- git status (file change info is provided in <branch-info> below)
- git commit (use `{0} commit` instead)
- git checkout
- git squash
- git rebase
- git cherry-pick

These commands modify branches or provide information already available to you.

## What You CAN Do

- Run git commands that give read-only information about the repository (git log, git diff, etc.)
- Use the GitButler CLI (`{0}`) to perform disallowed actions
- Reference file changes and uncommitted changes from the <branch-info> section provided below

## Using the GitButler CLI

Disallowed actions can instead be performed using `{0}`.
For help with available commands, consult `{0} --help`.

### Common Commands

**Viewing changes:**
- `{0} status` - View changes assigned to this branch

**Creating commits:**
- `{0} commit -m \"message\"` - Commit changes to this branch

**Modifying commits:**
- `{0} describe <commit>` - Edit a commit message
- `{0} absorb` - Absorb uncommitted changes into existing commits automatically
- `{0} rub <source> <target>` - Move changes between commits, squash, amend, or assign files

**JSON Output:**
Many `{0}` commands support the `--json` flag, which provides structured output that is easier to parse programmatically. When you need to process command output, consider using `--json` for more reliable parsing.

## Communication Guidelines

DO NOT mention GitButler unless the user asks you to perform a disallowed git action.

<example>
<user>
Can you make a git commit?
</user>
<response>
Sorry, this project is managed by GitButler so you must make commits through the GitButler interface.
</response>
</example>

<example>
<user>
Can you pull in the latest changes
</user>
<response>
Sorry, this project is managed by GitButler so you must integrate upstream changes through the GitButler interface.
</response>
</example>

<example>
<user>
What files have changed?
</user>
<response>
Based on the branch-info provided, the following files have been modified in this branch:

Committed changes (in branch commits):
[Lists files that were changed in the commits shown in the branch]

Uncommitted changes (assigned to this stack):
[Lists files from the uncommitted files section]
</response>
</example>
</git-usage>",
        but_path
    )
}

/// Formats branch information for the system prompt
fn format_branch_info(ctx: &mut Context, stack_id: StackId) -> String {
    let mut output = String::from(
        "<branch-info>\n\
        This repository uses GitButler for branch management. While git shows you are on\n\
        the `gitbutler/workspace` branch, this is actually a merge commit containing one or\n\
        more independent stacks of branches being worked on simultaneously.\n\n\
        This session is specific to a particular branch within that workspace. When asked about\n\
        the current branch or what changes have been made, you should focus on the current working\n\
        branch listed below, not the workspace branch itself.\n\n\
        Changes and diffs should be understood relative to the target branch (upstream), as that\n\
        represents the integration point for this work.\n\n\
        IMPORTANT: This section includes both COMMITTED changes (in branch commits below) and\n\
        UNCOMMITTED changes (in the assigned files section). When asked about file changes,\n\
        consider both committed and uncommitted changes.\n\n",
    );

    append_target_branch_info(&mut output, ctx);
    append_stack_branches_info(&mut output, stack_id, ctx);
    append_assigned_files_info(&mut output, stack_id, ctx);

    output.push_str("</branch-info>");
    output
}

/// Appends target branch (upstream) information to the output
fn append_target_branch_info(output: &mut String, ctx: &Context) {
    let state = VirtualBranchesHandle::new(ctx.project_data_dir());
    match state.get_default_target() {
        Ok(target) => {
            output.push_str(&format!(
                "Target branch (upstream): {}/{}\n\n",
                target.branch.remote(),
                target.branch.branch()
            ));
        }
        Err(e) => {
            tracing::warn!("Failed to fetch target branch information: {}", e);
        }
    }
}

/// Appends information about branches in the stack
fn append_stack_branches_info(output: &mut String, stack_id: StackId, ctx: &mut Context) {
    match but_workspace::legacy::stack_branches(stack_id, ctx) {
        Ok(branches) if !branches.is_empty() => {
            if let Some(first_branch) = branches.first() {
                let first_branch_name = first_branch.name.to_str_lossy();
                output.push_str(&format!(
                    "When running git commands that reference HEAD (e.g., `git diff origin/master...HEAD`),\n\
                    replace HEAD with the current working branch name: `{}`\n\n",
                    first_branch_name
                ));
            }

            output.push_str("The following branches are part of the current stack:\n");

            // Write first branch with marker
            if let Some(first_branch) = branches.first() {
                output.push_str(&format!(
                    "- {} (current working branch)\n",
                    first_branch.name.to_str_lossy()
                ));
            }

            // Write remaining branches
            for branch in branches.iter().skip(1) {
                output.push_str(&format!("- {}\n", branch.name.to_str_lossy()));
            }
        }
        Ok(_) => {
            output.push_str("There are no branches in the current stack.\n");
        }
        Err(e) => {
            tracing::warn!("Failed to fetch branch information: {}", e);
            output.push_str("Unable to fetch branch information.\n");
        }
    }
}

/// Appends information about files assigned to this stack
fn append_assigned_files_info(output: &mut String, stack_id: StackId, ctx: &mut Context) {
    let assignments = match but_hunk_assignment::assignments_with_fallback(
        ctx,
        false,
        None::<Vec<but_core::TreeChange>>,
        None,
    ) {
        Ok((assignments, _error)) => assignments,
        Err(e) => {
            tracing::warn!("Failed to fetch hunk assignments: {}", e);
            return;
        }
    };

    let file_assignments = group_assignments_by_file(&assignments, stack_id);
    if file_assignments.is_empty() {
        return;
    }

    let mut file_paths: Vec<_> = file_assignments.keys().copied().collect();
    file_paths.sort();

    output.push_str("\nUncommitted files assigned to this stack:\n");
    for file_path in file_paths {
        format_file_with_line_ranges(output, file_path, &file_assignments[file_path]);
    }
}

type FileAssignments<'a> = HashMap<&'a str, Vec<&'a but_hunk_assignment::HunkAssignment>>;

/// Groups hunk assignments by file path for the given stack
fn group_assignments_by_file(
    assignments: &[but_hunk_assignment::HunkAssignment],
    stack_id: StackId,
) -> FileAssignments<'_> {
    assignments
        .iter()
        .filter(|a| a.stack_id == Some(stack_id))
        .fold(HashMap::new(), |mut acc, assignment| {
            acc.entry(assignment.path.as_str())
                .or_default()
                .push(assignment);
            acc
        })
}

/// Formats a file path with its associated line ranges
fn format_file_with_line_ranges(
    output: &mut String,
    file_path: &str,
    hunks: &[&but_hunk_assignment::HunkAssignment],
) {
    let line_ranges: Vec<String> = hunks
        .iter()
        .filter_map(|hunk| hunk.hunk_header.as_ref())
        .map(|header| {
            format!(
                "{}-{}",
                header.new_start,
                header.new_start + header.new_lines
            )
        })
        .collect();

    if line_ranges.is_empty() {
        output.push_str(&format!("- {}\n", file_path));
    } else {
        output.push_str(&format!(
            "- {} (lines: {})\n",
            file_path,
            line_ranges.join(", ")
        ));
    }
}

fn format_message_with_summary(
    summary: &str,
    message: &str,
    thinking_level: ThinkingLevel,
) -> String {
    let message = format!(
        "<previous-conversation>
This conversation is a continuation of a previous one.

Here is a summary of the previous conversation for you to keep in mind.
<summary>
{summary}
</summary>
</previous-conversation>

Here is the next message by the user:

{message}"
    );

    format_message(&message, thinking_level)
}

fn format_message(message: &str, thinking_level: ThinkingLevel) -> String {
    match thinking_level {
        ThinkingLevel::Normal => message.to_owned(),
        ThinkingLevel::Think => {
            format!("{message}\n\nPlease think before taking any actions")
        }
        ThinkingLevel::MegaThink => {
            format!("{message}\n\nPlease megathink before taking any actions")
        }
        ThinkingLevel::UltraThink => {
            format!("{message}\n\nPlease ultrathink before taking any actions")
        }
    }
}

/// If a session exists, it just returns it, otherwise it creates a new session
/// and makes a corresponding rule
fn upsert_session(
    ctx: &mut Context,
    session_id: uuid::Uuid,
    stack_id: StackId,
) -> Result<crate::ClaudeSession> {
    let session = if let Some(session) = db::get_session_by_id(ctx, session_id)? {
        db::set_session_in_gui(ctx, session_id, true)?;
        session
    } else {
        let session = db::save_new_session_with_gui_flag(ctx, session_id, true)?;
        create_claude_assignment_rule(ctx, session_id, stack_id)?;
        session
    };
    Ok(session)
}

/// Spawns the thread that manages reading the CC stdout and saves the events to
/// the db and streams them to the client.
fn spawn_response_streaming(
    sync_ctx: ThreadSafeContext,
    broadcaster: Arc<Mutex<Broadcaster>>,
    read_stdout: PipeReader,
    session_id: uuid::Uuid,
    stack_id: StackId,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Spawn a blocking task to read lines from the pipe
        std::thread::spawn(move || {
            let reader = BufReader::new(read_stdout);
            for line in reader.lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        let mut first = true;
        while let Some(line) = rx.recv().await {
            let parsed_event: serde_json::Value = serde_json::from_str(&line).unwrap();

            {
                let mut ctx = sync_ctx.clone().into_thread_local();
                if first {
                    let current_session_id = parsed_event["session_id"]
                        .as_str()
                        .unwrap()
                        .parse()
                        .unwrap();
                    let session = db::get_session_by_id(&mut ctx, session_id).unwrap();
                    if session.is_some() {
                        db::add_session_id(&mut ctx, session_id, current_session_id).unwrap();
                    }
                    first = false;
                }
            }

            let message_content = MessagePayload::Claude(ClaudeOutput {
                data: parsed_event.clone(),
            });
            send_claude_message(
                sync_ctx.clone(),
                broadcaster.clone(),
                session_id,
                stack_id,
                message_content,
            )
            .await
            .unwrap();
        }
    })
}

impl Default for Claudes {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of checking Claude Code availability
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ClaudeCheckResult {
    /// Claude Code is available and returned a version
    Available { version: String },
    /// Claude Code is not available or failed to execute
    NotAvailable,
}

/// Validates and sanitizes attachment data to prevent prompt injection
fn validate_attachment(attachment: &PromptAttachment) -> Result<()> {
    match attachment {
        PromptAttachment::File(file) => {
            validate_path(&file.path)?;
            if let Some(commit_id) = &file.commit_id {
                validate_commit_id(commit_id)?;
            }
        }
        PromptAttachment::Lines(lines) => {
            validate_path(&lines.path)?;
            if let Some(commit_id) = &lines.commit_id {
                validate_commit_id(commit_id)?;
            }
        }
        PromptAttachment::Commit(commit) => {
            validate_commit_id(&commit.commit_id)?;
        }
    }
    Ok(())
}

/// Validates a file path to ensure it's safe and parseable
fn validate_path(path: &str) -> Result<()> {
    // Check for empty path
    if path.trim().is_empty() {
        bail!("Path cannot be empty");
    }

    // Check for suspicious characters that could indicate injection attempts
    // Allow: alphanumeric, /, \, ., -, _, space, and common path separators
    let suspicious_chars = ['<', '>', '"', '\'', '\n', '\r', '\0'];
    if path.chars().any(|c| suspicious_chars.contains(&c)) {
        bail!("Path contains invalid characters: {}", path);
    }

    // Try to parse as a Path to ensure it's valid
    let path_buf = std::path::Path::new(path);

    // Check for null bytes which can cause issues
    if path.contains('\0') {
        bail!("Path contains null bytes");
    }

    // Ensure the path can be converted back to a string
    if path_buf.to_str().is_none() {
        bail!("Path contains invalid UTF-8");
    }

    Ok(())
}

/// Validates a commit ID to ensure it's a valid git commit hash
fn validate_commit_id(commit_id: &str) -> Result<()> {
    // Check for empty commit_id
    if commit_id.trim().is_empty() {
        bail!("Commit ID cannot be empty");
    }

    // Git commit IDs are 7-40 character hex strings (short or full SHA)
    if commit_id.len() < 7 || commit_id.len() > 40 {
        bail!("Commit ID has invalid length: {}", commit_id.len());
    }

    // Check that it only contains valid hex characters
    if !commit_id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "Commit ID contains non-hexadecimal characters: {}",
            commit_id
        );
    }

    Ok(())
}

/// Process file attachments by writing them to temporary files in the project directory
/// and enhancing the message to reference these files
async fn format_message_with_attachments(
    original_message: &str,
    attachments: &[PromptAttachment],
) -> Result<String> {
    if attachments.is_empty() {
        return Ok(original_message.to_string());
    }

    // Validate all attachments before processing
    for attachment in attachments {
        validate_attachment(attachment)?;
    }

    for attachment in attachments {
        validate_attachment(attachment)?;
    }

    let attachments_json = serde_json::to_string_pretty(&attachments)?;

    let message = format!(
        "{}

<context-attachments>
The following JSON of files, line ranges, and commits have been added as
context. Please consider them if a question or reference to files, lines,
or commits, is unspecified.
<attachments>
{}
</attachments>
</context-attachments>
",
        original_message, attachments_json
    );

    Ok(message)
}

/// Check if Claude Code is available by running the version command.
/// Returns ClaudeCheckResult indicating availability and version if available.
pub async fn check_claude_available(claude_executable: &str) -> ClaudeCheckResult {
    let mut command = Command::new(claude_executable);
    command.arg("--version");

    // Don't create a terminal window on windows.
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    match command.output().await {
        Ok(output) if output.status.success() => match String::from_utf8(output.stdout) {
            Ok(version) => ClaudeCheckResult::Available {
                version: version.trim().to_string(),
            },
            Err(_) => ClaudeCheckResult::NotAvailable,
        },
        _ => ClaudeCheckResult::NotAvailable,
    }
}
