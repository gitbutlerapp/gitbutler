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

use crate::{
    ClaudeMessage, ClaudeMessageContent, ClaudeUserParams, PermissionMode, ThinkingLevel,
    Transcript, UserInput,
    claude_config::fmt_claude_settings,
    claude_mcp::{BUT_SECURITY_MCP, ClaudeMcpConfig},
    claude_settings::ClaudeSettings,
    db,
    rules::{create_claude_assignment_rule, list_claude_assignment_rules},
};
use anyhow::{Result, bail};
use but_broadcaster::{Broadcaster, FrontendEvent};
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, PipeReader, Read as _},
    process::ExitStatus,
    sync::Arc,
};
use tokio::{
    fs,
    process::{Child, Command},
    sync::{
        Mutex,
        mpsc::{UnboundedSender, unbounded_channel},
    },
};

/// Holds the CC instances. Currently keyed by stackId, since our current model
/// assumes one CC per stack at any given time.
pub struct Claudes {
    /// A set that contains all the currently running requests
    requests: Mutex<HashMap<StackId, Arc<Claude>>>,
}

pub struct Claude {
    kill: UnboundedSender<()>,
}

impl Claudes {
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
        }
    }

    pub async fn send_message(
        &self,
        ctx: Arc<Mutex<CommandContext>>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
        user_params: ClaudeUserParams,
    ) -> Result<()> {
        if self.requests.lock().await.contains_key(&stack_id) {
            bail!(
                "Claude is currently thinking, please wait for it to complete before sending another message.\n\nIf claude is stuck thinking, try restarting the application."
            )
        } else {
            self.spawn_claude(ctx, broadcaster, stack_id, user_params)
                .await
        };

        Ok(())
    }

    pub fn get_messages(
        &self,
        ctx: &mut CommandContext,
        stack_id: StackId,
    ) -> Result<Vec<ClaudeMessage>> {
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
        ctx: Arc<Mutex<CommandContext>>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
        user_params: ClaudeUserParams,
    ) -> () {
        let res = self
            .spawn_claude_inner(ctx.clone(), broadcaster.clone(), stack_id, user_params)
            .await;
        if let Err(res) = res {
            let mut ctx = ctx.lock().await;
            self.requests.lock().await.remove(&stack_id);

            let rule = list_claude_assignment_rules(&mut ctx)
                .ok()
                .and_then(|rules| rules.into_iter().find(|rule| rule.stack_id == stack_id));

            if let Some(rule) = rule {
                let _ = send_claude_message(
                    &mut ctx,
                    broadcaster.clone(),
                    rule.session_id,
                    stack_id,
                    ClaudeMessageContent::GitButlerMessage(
                        crate::GitButlerMessage::UnhandledException {
                            message: format!("{res}"),
                        },
                    ),
                )
                .await;
            }
        };
    }

    async fn spawn_claude_inner(
        &self,
        ctx: Arc<Mutex<CommandContext>>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
        user_params: ClaudeUserParams,
    ) -> Result<()> {
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
        let rule = {
            let mut ctx = ctx.lock().await;
            list_claude_assignment_rules(&mut ctx)?
                .into_iter()
                .find(|rule| rule.stack_id == stack_id)
        };

        let session_id = rule.map(|r| r.session_id).unwrap_or(uuid::Uuid::new_v4());

        let broadcaster = broadcaster.clone();

        let session = upsert_session(ctx.clone(), session_id, stack_id).await?;
        {
            let mut ctx = ctx.lock().await;
            send_claude_message(
                &mut ctx,
                broadcaster.clone(),
                session_id,
                stack_id,
                ClaudeMessageContent::UserInput(UserInput {
                    message: user_params.message.clone(),
                }),
            )
            .await?;
        }
        let (read_stdout, writer) = std::io::pipe()?;
        let response_streamer = spawn_response_streaming(
            ctx.clone(),
            broadcaster.clone(),
            read_stdout,
            session_id,
            stack_id,
        );

        let (read_stderr, write_stderr) = std::io::pipe()?;
        // Clone so the reference to ctx can be immediatly dropped
        let project = ctx.lock().await.project().clone();
        let mut handle = spawn_command(
            writer,
            write_stderr,
            session,
            project.path.clone(),
            ctx.clone(),
            user_params,
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
            ctx.clone(),
            broadcaster,
            stack_id,
            session_id,
            read_stderr,
            handle,
            cmd_exit,
        )
        .await?;

        // Send completion notification
        let app_settings = ctx.lock().await.app_settings().clone();
        if let Err(e) = crate::notifications::notify_completion(&app_settings) {
            tracing::warn!("Failed to send completion notification: {}", e);
        }

        Ok(())
    }
}

async fn handle_exit(
    ctx: Arc<Mutex<CommandContext>>,
    broadcaster: Arc<Mutex<Broadcaster>>,
    stack_id: but_core::Id<'S'>,
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
            let mut ctx = ctx.lock().await;
            send_claude_message(
                &mut ctx,
                broadcaster.clone(),
                session_id,
                stack_id,
                ClaudeMessageContent::GitButlerMessage(crate::GitButlerMessage::ClaudeExit {
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
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;
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
            let mut ctx = ctx.lock().await;
            send_claude_message(
                &mut ctx,
                broadcaster.clone(),
                session_id,
                stack_id,
                ClaudeMessageContent::GitButlerMessage(crate::GitButlerMessage::UserAbort),
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
async fn spawn_command(
    writer: std::io::PipeWriter,
    write_stderr: std::io::PipeWriter,
    session: crate::ClaudeSession,
    project_path: std::path::PathBuf,
    ctx: Arc<Mutex<CommandContext>>,
    user_params: ClaudeUserParams,
) -> Result<Child> {
    // Write and obtain our own claude hooks path.
    let settings = fmt_claude_settings()?;

    let app_settings = ctx.lock().await.app_settings().clone();
    let claude_executable = app_settings.claude.executable.clone();
    let cc_settings = ClaudeSettings::open(&project_path).await;
    let mcp_config = ClaudeMcpConfig::open(&cc_settings, &project_path).await;
    let disabled_mcp_servers = user_params
        .disabled_mcp_servers
        .iter()
        .filter(|f| *f != BUT_SECURITY_MCP)
        .map(String::as_str)
        .collect::<Vec<&str>>();
    let mcp_config = &mcp_config
        .mcp_servers_with_security()
        .exclude(&disabled_mcp_servers);
    tracing::info!(
        "spawn_command mcp_servers: {:?}",
        mcp_config.mcp_servers.keys()
    );
    let mcp_config = serde_json::to_string(mcp_config)?;
    let mut command = Command::new(claude_executable);

    /// Don't create a terminal window on windows.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
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
    if !app_settings.claude.use_configured_model {
        command.args(["--model", user_params.model.to_cli_string()]);
    }

    command.args(["-p", "--verbose"]);

    if app_settings.claude.dangerously_allow_all_permissions {
        command.arg("--dangerously-skip-permissions");
    } else {
        command.args([
            "--permission-prompt-tool",
            "mcp__but-security__approval_prompt",
        ]);
        // Set permission mode based on interaction mode
        match user_params.permission_mode {
            PermissionMode::Default => {}
            PermissionMode::Plan => {
                command.args(["--permission-mode", "plan"]);
            }
            PermissionMode::AcceptEdits => {
                command.args(["--permission-mode", "acceptEdits"]);
            }
        };
    }

    let current_id = Transcript::current_valid_session_id(&project_path, &session).await?;

    if let Some(current_id) = current_id {
        command.args(["--resume", &format!("{current_id}")]);
    } else {
        // Ensure that there isn't an existant invalid transcript
        let path = Transcript::get_transcript_path(&project_path, session.id)?;
        if fs::try_exists(&path).await? {
            fs::remove_file(&path).await?;
        }
        command.args(["--session-id", &format!("{}", session.id)]);
    }

    command.args(["--append-system-prompt", SYSTEM_PROMPT]);

    command.arg(format_message(
        &user_params.message,
        user_params.thinking_level,
    ));
    tracing::info!("spawn_command: {:?}", command);
    Ok(command.spawn()?)
}

const SYSTEM_PROMPT: &str = "<git-usage>
You are working on a project that is managed by GitButler.

This means that you MUST NOT run git commands that checkout branches or update heads.

For example you MUST NOT run the following git commands:
- git commit
- git checkout
- git rebase
- git cherry-pick

You MAY run git commands that give you information about the current git state.

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
Sorry, this project is managed by GitButler so you must integrate upstream upstream changes through the GitButler interface.
</response>
</example>
</git-usage>";

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
/// and makes a cooresponding rule
async fn upsert_session(
    ctx: Arc<Mutex<CommandContext>>,
    session_id: uuid::Uuid,
    stack_id: StackId,
) -> Result<crate::ClaudeSession> {
    let mut ctx = ctx.lock().await;
    let session = if let Some(session) = db::get_session_by_id(&mut ctx, session_id)? {
        db::set_session_in_gui(&mut ctx, session_id, true)?;
        session
    } else {
        let session = db::save_new_session_with_gui_flag(&mut ctx, session_id, true)?;
        create_claude_assignment_rule(&mut ctx, session_id, stack_id)?;
        session
    };
    Ok(session)
}

/// Spawns the thread that manages reading the CC stdout and saves the events to
/// the db and streams them to the client.
fn spawn_response_streaming(
    ctx: Arc<Mutex<CommandContext>>,
    broadcaster: Arc<Mutex<Broadcaster>>,
    read_stdout: PipeReader,
    session_id: uuid::Uuid,
    stack_id: StackId,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let reader = BufReader::new(read_stdout);
        let mut first = true;
        for line in reader.lines() {
            let mut ctx = ctx.lock().await;
            let line = line.unwrap();
            let parsed_event: serde_json::Value = serde_json::from_str(&line).unwrap();

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

            let message_content = ClaudeMessageContent::ClaudeOutput(parsed_event.clone());
            send_claude_message(
                &mut ctx,
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

async fn send_claude_message(
    ctx: &mut CommandContext,
    broadcaster: Arc<Mutex<Broadcaster>>,
    session_id: uuid::Uuid,
    stack_id: StackId,
    content: ClaudeMessageContent,
) -> Result<()> {
    let message = db::save_new_message(ctx, session_id, content.clone())?;
    let project_id = ctx.project().id;

    broadcaster.lock().await.send(FrontendEvent {
        name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
        payload: json!(message),
    });
    Ok(())
}

/// Result of checking Claude Code availability
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ClaudeCheckResult {
    /// Claude Code is available and returned a version
    Available { version: String },
    /// Claude Code is not available or failed to execute
    NotAvailable,
}

/// Check if Claude Code is available by running the version command.
/// Returns ClaudeCheckResult indicating availability and version if available.
pub async fn check_claude_available(claude_executable: &str) -> ClaudeCheckResult {
    let mut command = Command::new(claude_executable);
    command.arg("--version");

    /// Don't create a terminal window on windows.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
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
