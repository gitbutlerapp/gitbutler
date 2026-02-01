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
use but_core::{
    ref_metadata::StackId,
    sync::{RepoExclusive, RepoShared},
};
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
    Broadcaster, ClaudeMessage, ClaudeOutput, ClaudeUserParams, MessagePayload, PermissionMode, PromptAttachment,
    SystemMessage, ThinkingLevel, Transcript, UserInput,
    broadcaster::FrontendEvent,
    claude_config::fmt_claude_settings,
    claude_mcp::{BUT_SECURITY_MCP, ClaudeMcpConfig, convert_mcp_config_to_sdk},
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

    pub fn get_messages(&self, ctx: &Context, stack_id: StackId) -> Result<Vec<ClaudeMessage>> {
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
                let ctx = sync_ctx.clone().into_thread_local();
                list_claude_assignment_rules(&ctx)
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
        use claude_agent_sdk_rs::{ClaudeAgentOptions, ClaudeClient, Message as SdkMessage, SystemPrompt};
        use futures::stream::StreamExt;

        // Capture the start time to filter messages created during this session
        let session_start_time = chrono::Utc::now().naive_utc();

        let (send_kill, mut recv_kill) = unbounded_channel();
        self.requests
            .lock()
            .await
            .insert(stack_id, Arc::new(Claude { kill: send_kill }));

        let SessionSetup {
            summary_to_resume,
            session_id,
            session,
            project_workdir,
        } = setup_session(&sync_ctx, stack_id)?;

        let transcript_current_id = Transcript::current_valid_session_id(&project_workdir, &session).await?;

        tracing::info!(
            "spawn_claude_sdk: session.id={}, session.session_ids={:?}, transcript_current_id={:?}, summary_to_resume={:?}",
            session.id,
            session.session_ids,
            transcript_current_id,
            summary_to_resume
                .as_ref()
                .map(|s| s.chars().take(100).collect::<String>())
        );

        // Store the original user message for UI display
        send_claude_message(
            sync_ctx.clone(),
            broadcaster.clone(),
            session_id,
            stack_id,
            MessagePayload::User(UserInput {
                message: user_params.message.clone(),
                attachments: user_params.attachments.clone(),
            }),
        )
        .await?;

        // Configure SDK options
        let dangerously_skip_permissions = sync_ctx.settings.claude.dangerously_allow_all_permissions;
        let permission_mode = if dangerously_skip_permissions {
            claude_agent_sdk_rs::PermissionMode::BypassPermissions
        } else {
            match user_params.permission_mode {
                PermissionMode::Default => claude_agent_sdk_rs::PermissionMode::Default,
                PermissionMode::Plan => claude_agent_sdk_rs::PermissionMode::Plan,
                PermissionMode::AcceptEdits => claude_agent_sdk_rs::PermissionMode::AcceptEdits,
            }
        };

        // Determine the session ID to use, matching the binary implementation logic:
        // - If resuming after compaction (summary_to_resume.is_some()), use a new random ID
        // - If resuming an existing session (transcript_current_id), use that ID
        // - If starting new, use the stable session.id
        let claude_session_id = if summary_to_resume.is_some() {
            // After compaction, start with a new session ID
            uuid::Uuid::new_v4()
        } else if let Some(current_id) = transcript_current_id {
            // If resuming, use the existing current_id
            current_id
        } else {
            // If starting new, ensure there isn't an existing invalid transcript
            // (matching binary implementation)
            let path = Transcript::get_transcript_path(&project_workdir, session.id)?;
            if fs::try_exists(&path).await? {
                fs::remove_file(&path).await?;
            }
            // Use the stable session.id
            session.id
        };

        // IMPORTANT: Persist the Claude session ID to the database BEFORE building MCP config.
        // The MCP server (but-security) needs to look up the session by current_id when it starts.
        // If we wait until after client.connect(), the MCP server will fail because the session
        // won't exist yet in the database.
        {
            let mut ctx = sync_ctx.clone().into_thread_local();
            if let Err(e) = db::add_session_id(&mut ctx, session_id, claude_session_id) {
                tracing::warn!(
                    "Failed to persist claude_session_id {} to session {}: {}",
                    claude_session_id,
                    session_id,
                    e
                );
            }
        }

        // Build MCP server configuration using the same logic as the binary implementation
        let cc_settings = ClaudeSettings::open(&project_workdir).await;
        let mcp_config = ClaudeMcpConfig::open(&cc_settings, &project_workdir).await;

        // NOTE: In SDK path, we do NOT include the but-security MCP server because
        // we handle permissions via the can_use_tool callback with --permission-prompt-tool stdio.
        // Including but-security would cause the CLI to try to use the MCP server for permissions
        // instead of sending control requests to the SDK.

        // Filter out disabled servers (but-security is already excluded in SDK path)
        let disabled_mcp_servers = user_params
            .disabled_mcp_servers
            .iter()
            .map(String::as_str)
            .collect::<Vec<&str>>();
        let mcp_config = mcp_config.mcp_servers().exclude(&disabled_mcp_servers);

        // Convert McpConfig to SDK's McpServers format
        let mcp_servers = convert_mcp_config_to_sdk(&mcp_config);

        // Build system prompt with branch info (same as binary implementation)
        let system_prompt_append = {
            let mut ctx = sync_ctx.clone().into_thread_local();
            let guard = ctx.exclusive_worktree_access();
            let branch_info = format_branch_info(&mut ctx, stack_id, guard.read_permission());
            format!("{}\n\n{}", system_prompt(), branch_info)
        };
        let sdk_system_prompt = SystemPrompt::Preset(claude_agent_sdk_rs::SystemPromptPreset::with_append(
            "claude_code",
            system_prompt_append,
        ));

        // Build options
        // Only set model if useConfiguredModel is false (same as binary implementation)
        let model = if sync_ctx.settings.claude.use_configured_model {
            None
        } else {
            Some(user_params.model.to_cli_string().to_string())
        };

        // Determine resume behavior (matching binary implementation):
        // - summary_to_resume.is_some(): Don't resume, start fresh with summary context (use --session-id)
        // - transcript_current_id.is_some() && summary_to_resume.is_none(): Resume existing session (use --resume)
        // - Otherwise: Start new session (use --session-id)
        let (resume, extra_args) = if summary_to_resume.is_none() && transcript_current_id.is_some() {
            // Resume existing session
            (Some(claude_session_id.to_string()), HashMap::new())
        } else {
            // Start new session (or after compaction) - pass session-id via extra_args
            let mut args = HashMap::new();
            args.insert("session-id".to_string(), Some(claude_session_id.to_string()));
            (None, args)
        };

        // Use can_use_tool callback to handle AskUserQuestion. This is the proper SDK mechanism
        // for handling user questions - when can_use_tool returns PermissionResultAllow with
        // updated_input containing the answers, the CLI uses those answers directly instead of
        // executing the built-in AskUserQuestion tool (which would prompt for input and fail).
        // Create the can_use_tool callback which handles AskUserQuestion specially
        // and auto-approves all other tools when dangerously_skip_permissions is enabled.
        let can_use_tool_callback = create_can_use_tool_callback(
            sync_ctx.clone(),
            stack_id,
            dangerously_skip_permissions,
            claude_session_id,
        );
        // Keep a PreToolUse hook as well for the dummy hook requirement (keeps stream open)
        let pretool_hook = create_pretool_use_hook(sync_ctx.clone(), stack_id);
        // Add PostToolUse hook to assign hunks to the session's stack (critical for commit creation)
        let posttool_hook = create_post_tool_use_hook();
        // Add Stop hook to handle commit creation when Claude finishes
        let stop_hook = create_stop_hook();
        let mut hooks = std::collections::HashMap::new();
        hooks.insert(
            claude_agent_sdk_rs::HookEvent::PreToolUse,
            vec![
                claude_agent_sdk_rs::HookMatcher::builder()
                    .hooks(vec![pretool_hook])
                    .build(),
            ],
        );
        hooks.insert(
            claude_agent_sdk_rs::HookEvent::PostToolUse,
            vec![
                claude_agent_sdk_rs::HookMatcher::builder()
                    .matcher("Edit|MultiEdit|Write".to_string())
                    .hooks(vec![posttool_hook])
                    .build(),
            ],
        );
        hooks.insert(
            claude_agent_sdk_rs::HookEvent::Stop,
            vec![
                claude_agent_sdk_rs::HookMatcher::builder()
                    .hooks(vec![stop_hook])
                    .build(),
            ],
        );

        let options = ClaudeAgentOptions {
            model,
            permission_mode: Some(permission_mode),
            mcp_servers,
            cwd: Some(project_workdir.clone()),
            system_prompt: Some(sdk_system_prompt),
            resume,
            extra_args,
            add_dirs: user_params.add_dirs.iter().map(Into::into).collect(),
            hooks: Some(hooks),
            can_use_tool: Some(can_use_tool_callback),
            // This is critical: tells CLI to send permission requests via stdio control protocol
            // instead of using built-in UI. Required for can_use_tool callback to work.
            permission_prompt_tool_name: Some("stdio".to_string()),
            ..Default::default()
        };

        // Create client and connect
        let mut client = ClaudeClient::new(options);
        if let Err(e) = client.connect().await {
            self.requests.lock().await.remove(&stack_id);
            crate::pending_requests::pending_requests().cancel_session(session_id);
            send_claude_message(
                sync_ctx.clone(),
                broadcaster.clone(),
                session_id,
                stack_id,
                MessagePayload::System(SystemMessage::UnhandledException {
                    message: format!("Failed to connect to Claude SDK: {}", e),
                }),
            )
            .await?;
            return Err(e.into());
        }

        // Note: Session ID was already persisted earlier (before MCP config was built)
        // to ensure the MCP server can find the session when it starts.

        // Prepare and send the message (matching binary implementation)
        let message = if let Some(attachments) = &user_params.attachments {
            format_message_with_attachments(&user_params.message, attachments).await?
        } else {
            user_params.message.clone()
        };

        // Format message with summary context if resuming from compaction
        let formatted_message = if let Some(summary) = &summary_to_resume {
            format_message_with_summary(summary, &message, user_params.thinking_level)
        } else {
            format_message(&message, user_params.thinking_level)
        };

        if let Err(e) = client.query(&formatted_message).await {
            self.requests.lock().await.remove(&stack_id);
            crate::pending_requests::pending_requests().cancel_session(session_id);
            client.disconnect().await?;
            send_claude_message(
                sync_ctx.clone(),
                broadcaster.clone(),
                session_id,
                stack_id,
                MessagePayload::System(SystemMessage::UnhandledException {
                    message: format!("Failed to send query to Claude: {}", e),
                }),
            )
            .await?;
            return Err(e.into());
        }

        // Stream responses
        let mut stream = client.receive_response();

        loop {
            tokio::select! {
                message_result = stream.next() => {
                    match message_result {
                        Some(Ok(sdk_message)) => {
                            match sdk_message {
                                SdkMessage::Assistant(assistant_msg) => {
                                    // Convert SDK message to ClaudeOutput format
                                    let mut data = serde_json::to_value(&assistant_msg)?;
                                    if let Some(obj) = data.as_object_mut() {
                                        obj.insert("type".to_string(), serde_json::json!("assistant"));
                                    }
                                    send_claude_message(
                                        sync_ctx.clone(),
                                        broadcaster.clone(),
                                        session_id,
                                        stack_id,
                                        MessagePayload::Claude(ClaudeOutput { data }),
                                    )
                                    .await?;
                                }
                                SdkMessage::User(user_msg) => {
                                    // The CLI outputs: {"type": "user", "message": {"content": [...]}}
                                    // The SDK's UserMessage struct has content directly, but due to
                                    // serde flatten, the "message" wrapper ends up in user_msg.extra.
                                    // We need to reconstruct the format expected by the frontend.
                                    let data = if let Some(message) = user_msg.extra.get("message") {
                                        // The "message" wrapper is in extra - use it directly
                                        serde_json::json!({
                                            "type": "user",
                                            "message": message
                                        })
                                    } else if user_msg.content.is_some() {
                                        // Content is at top level (alternative format)
                                        serde_json::json!({
                                            "type": "user",
                                            "message": {
                                                "content": user_msg.content
                                            }
                                        })
                                    } else {
                                        // Fallback: serialize the whole message
                                        let mut data = serde_json::to_value(&user_msg)?;
                                        if let Some(obj) = data.as_object_mut() {
                                            obj.insert("type".to_string(), serde_json::json!("user"));
                                        }
                                        data
                                    };

                                    // Note: AskUserQuestion answers are now injected via PostToolUse hook's
                                    // additional_context, not by modifying the tool result here.

                                    send_claude_message(
                                        sync_ctx.clone(),
                                        broadcaster.clone(),
                                        session_id,
                                        stack_id,
                                        MessagePayload::Claude(ClaudeOutput { data }),
                                    )
                                    .await?;
                                }
                                SdkMessage::Result(result_msg) => {
                                    send_claude_message(
                                        sync_ctx.clone(),
                                        broadcaster.clone(),
                                        session_id,
                                        stack_id,
                                        MessagePayload::System(SystemMessage::ClaudeExit {
                                            code: if result_msg.is_error { 1 } else { 0 },
                                            message: result_msg.result.unwrap_or_default(),
                                        }),
                                    )
                                    .await?;
                                    break;
                                }
                                // System and StreamEvent messages are informational
                                _ => {}
                            }
                        }
                        Some(Err(e)) => {
                            send_claude_message(
                                sync_ctx.clone(),
                                broadcaster.clone(),
                                session_id,
                                stack_id,
                                MessagePayload::System(SystemMessage::UnhandledException {
                                    message: format!("SDK error: {}", e),
                                }),
                            )
                            .await?;
                            break;
                        }
                        None => break,
                    }
                }
                _ = recv_kill.recv() => {
                    send_claude_message(
                        sync_ctx.clone(),
                        broadcaster.clone(),
                        session_id,
                        stack_id,
                        MessagePayload::System(SystemMessage::UserAbort),
                    )
                    .await?;
                    break;
                }
            }
        }

        // Clean up
        drop(stream);
        self.requests.lock().await.remove(&stack_id);
        crate::pending_requests::pending_requests().cancel_session(session_id);
        client.disconnect().await?;

        broadcast_gitbutler_messages(&sync_ctx, &broadcaster, session_id, stack_id, session_start_time).await;
        send_completion_notification(&sync_ctx);

        Ok(())
    }
}

/// Result of setting up a Claude session, shared between binary and SDK implementations.
struct SessionSetup {
    /// Summary from context compaction, if resuming after compaction
    summary_to_resume: Option<String>,
    /// Our internal session ID (stable across Claude session restarts)
    session_id: uuid::Uuid,
    /// The session object from the database
    session: crate::ClaudeSession,
    /// The project working directory
    project_workdir: std::path::PathBuf,
}

/// Sets up the Claude session, finding or creating the session and checking for compaction summary.
fn setup_session(sync_ctx: &ThreadSafeContext, stack_id: StackId) -> Result<SessionSetup> {
    let mut ctx = sync_ctx.clone().into_thread_local();

    // Create repo and workspace once at the entry point
    let mut guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?.clone();

    let rule = {
        list_claude_assignment_rules(&ctx)?
            .into_iter()
            .find(|rule| rule.stack_id == stack_id)
    };

    let session_id = rule.map(|r| r.session_id).unwrap_or(uuid::Uuid::new_v4());
    let session = upsert_session(&mut ctx, session_id, stack_id, guard.write_permission())?;
    let project_workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))?
        .to_owned();

    // Check for summary from context compaction
    let ctx = sync_ctx.clone().into_thread_local();
    let messages = list_messages_by_session(&ctx, session.id)?;
    let summary_to_resume = if let Some(ClaudeMessage { payload, .. }) = messages.last() {
        match payload {
            MessagePayload::System(SystemMessage::CompactFinished { summary }) => Some(summary.clone()),
            _ => None,
        }
    } else {
        None
    };

    Ok(SessionSetup {
        summary_to_resume,
        session_id,
        session,
        project_workdir,
    })
}

/// Broadcasts any GitButler messages created during this Claude session.
async fn broadcast_gitbutler_messages(
    sync_ctx: &ThreadSafeContext,
    broadcaster: &Arc<Mutex<Broadcaster>>,
    session_id: uuid::Uuid,
    stack_id: StackId,
    session_start_time: chrono::NaiveDateTime,
) {
    let project_id = sync_ctx.legacy_project.id;
    let all_messages = {
        let ctx = sync_ctx.clone().into_thread_local();
        db::list_messages_by_session(&ctx, session_id)
    };
    if let Ok(all_messages) = all_messages {
        let new_messages: Vec<_> = all_messages
            .into_iter()
            .filter(|msg| matches!(msg.payload, MessagePayload::GitButler(_)))
            .filter(|msg| msg.created_at > session_start_time)
            .collect();

        for message in new_messages {
            broadcaster.lock().await.send(FrontendEvent {
                name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
                payload: serde_json::json!(message),
            });
        }
    }
}

/// Sends completion notification if configured.
fn send_completion_notification(sync_ctx: &ThreadSafeContext) {
    if let Err(e) = crate::notifications::notify_completion(&sync_ctx.settings) {
        tracing::warn!("Failed to send completion notification: {}", e);
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
    let transcript_current_id = Transcript::current_valid_session_id(&project_path, &session).await?;
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
    tracing::info!("spawn_command mcp_servers: {:?}", mcp_config.mcp_servers.keys());
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

    if sync_ctx.settings.clone().claude.dangerously_allow_all_permissions {
        command.arg("--dangerously-skip-permissions");
    } else {
        command.args(["--permission-prompt-tool", "mcp__but-security__approval_prompt"]);
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
        let guard = ctx.exclusive_worktree_access();
        format_branch_info(&mut ctx, stack_id, guard.read_permission())
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
    let child = command.spawn()?;

    // MUST NOT LOG plain `command` as it can contain secrets, like `ANTHROPIC_AUTH_TOKEN`.
    // This happens as the command env is passed through by reading its configuration files,
    // which may also contain secrects.
    command.env_clear();
    tracing::debug!(
        ?command,
        env_keys = ?cc_settings.env().keys().collect::<Vec<_>>(),
        "claude code command spawned successfully"
    );
    Ok(child)
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
fn format_branch_info(ctx: &mut Context, stack_id: StackId, perm: &RepoShared) -> String {
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
    append_assigned_files_info(&mut output, stack_id, ctx, perm).ok();

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
fn append_stack_branches_info(output: &mut String, stack_id: StackId, ctx: &Context) {
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
fn append_assigned_files_info(
    output: &mut String,
    stack_id: StackId,
    ctx: &mut Context,
    perm: &RepoShared,
) -> anyhow::Result<()> {
    let context_lines = ctx.settings.context_lines;
    let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm)?;
    let assignments = match but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        false,
        None::<Vec<but_core::TreeChange>>,
        None,
        context_lines,
    ) {
        Ok((assignments, _error)) => assignments,
        Err(e) => {
            tracing::warn!("Failed to fetch hunk assignments: {}", e);
            return Ok(());
        }
    };

    let file_assignments = group_assignments_by_file(&assignments, stack_id);
    if file_assignments.is_empty() {
        return Ok(());
    }

    let mut file_paths: Vec<_> = file_assignments.keys().copied().collect();
    file_paths.sort();

    output.push_str("\nUncommitted files assigned to this stack:\n");
    for file_path in file_paths {
        format_file_with_line_ranges(output, file_path, &file_assignments[file_path]);
    }
    Ok(())
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
            acc.entry(assignment.path.as_str()).or_default().push(assignment);
            acc
        })
}

/// Formats a file path with its associated line ranges
fn format_file_with_line_ranges(output: &mut String, file_path: &str, hunks: &[&but_hunk_assignment::HunkAssignment]) {
    let line_ranges: Vec<String> = hunks
        .iter()
        .filter_map(|hunk| hunk.hunk_header.as_ref())
        .map(|header| format!("{}-{}", header.new_start, header.new_start + header.new_lines))
        .collect();

    if line_ranges.is_empty() {
        output.push_str(&format!("- {}\n", file_path));
    } else {
        output.push_str(&format!("- {} (lines: {})\n", file_path, line_ranges.join(", ")));
    }
}

fn format_message_with_summary(summary: &str, message: &str, thinking_level: ThinkingLevel) -> String {
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
    perm: &mut RepoExclusive,
) -> Result<crate::ClaudeSession> {
    let session = if let Some(session) = db::get_session_by_id(ctx, session_id)? {
        db::set_session_in_gui(ctx, session_id, true)?;
        session
    } else {
        let session = db::save_new_session_with_gui_flag(ctx, session_id, true)?;
        create_claude_assignment_rule(ctx, session_id, stack_id, perm)?;
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
                    let current_session_id = parsed_event["session_id"].as_str().unwrap().parse().unwrap();
                    let session = db::get_session_by_id(&ctx, session_id).unwrap();
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
    /// Claude Code _could_ be found, but failed to execute
    ExecutionFailed { stdout: String, stderr: String },
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
        bail!("Commit ID contains non-hexadecimal characters: {}", commit_id);
    }

    Ok(())
}

/// Process file attachments by writing them to temporary files in the project directory
/// and enhancing the message to reference these files
async fn format_message_with_attachments(original_message: &str, attachments: &[PromptAttachment]) -> Result<String> {
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

/// Creates a can_use_tool callback that handles AskUserQuestion specially.
///
/// This is the proper SDK mechanism for handling user questions. When the callback
/// returns PermissionResultAllow with updated_input containing the answers, the CLI
/// uses those answers directly instead of executing the built-in AskUserQuestion tool.
///
/// For all other tools:
/// - If `auto_approve_tools` is true (user has bypass enabled), auto-approve everything
/// - Otherwise, auto-approve as well (we use Default mode to get can_use_tool callbacks)
///
/// Note: We always use Default permission mode to ensure can_use_tool is invoked,
/// but we auto-approve non-AskUserQuestion tools to maintain the expected behavior.
fn create_can_use_tool_callback(
    sync_ctx: ThreadSafeContext,
    stack_id: gitbutler_stack::StackId,
    auto_approve_tools: bool,
    session_id: uuid::Uuid,
) -> claude_agent_sdk_rs::CanUseToolCallback {
    use claude_agent_sdk_rs::{PermissionResult, PermissionResultAllow, PermissionResultDeny, ToolPermissionContext};
    use futures::FutureExt;
    use std::sync::Arc;

    // Runtime permissions for this session
    let runtime_permissions = Arc::new(std::sync::Mutex::new(crate::permissions::Permissions::default()));

    Arc::new(
        move |tool_name: String, tool_input: serde_json::Value, context: ToolPermissionContext| {
            let sync_ctx = sync_ctx.clone();
            let stack_id = stack_id;
            let auto_approve = auto_approve_tools;
            let session_id = session_id;
            let runtime_permissions = Arc::clone(&runtime_permissions);

            async move {
                // Handle AskUserQuestion specially - poll for user answers
                if tool_name == "AskUserQuestion" {
                    return handle_ask_user_question(sync_ctx, stack_id, tool_input).await;
                }

                // For all other tools, check permissions
                // If auto_approve is true (bypass mode), allow everything
                if auto_approve {
                    return PermissionResult::Allow(PermissionResultAllow {
                        updated_input: Some(tool_input),
                        ..Default::default()
                    });
                }

                // Use tool_use_id from context as the request ID so it matches the tool call ID
                // in the transcript. This allows the UI to correlate permission requests with tool calls.
                let request_id = context
                    .tool_use_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let now = chrono::Utc::now().naive_utc();
                let permission_request = crate::ClaudePermissionRequest {
                    id: request_id.clone(),
                    created_at: now,
                    updated_at: now,
                    tool_name: tool_name.clone(),
                    input: tool_input.clone(),
                    decision: None,
                    use_wildcard: false,
                };

                // Check existing permissions (runtime + session)
                let check_result = {
                    let ctx = sync_ctx.clone().into_thread_local();

                    // Load session permissions
                    let session_perms = match crate::db::get_session_by_id(&ctx, session_id) {
                        Ok(Some(session)) => crate::permissions::Permissions::from_slices(
                            session.approved_permissions(),
                            session.denied_permissions(),
                        ),
                        _ => crate::permissions::Permissions::default(),
                    };

                    // Merge runtime and session permissions
                    let runtime_perms = runtime_permissions.lock().unwrap();
                    let combined_perms = crate::permissions::Permissions::merge([&*runtime_perms, &session_perms]);
                    drop(runtime_perms);

                    combined_perms.check(&permission_request).unwrap_or_default()
                };

                match check_result {
                    crate::permissions::PermissionCheck::Approved => {
                        return PermissionResult::Allow(PermissionResultAllow {
                            updated_input: Some(tool_input),
                            ..Default::default()
                        });
                    }
                    crate::permissions::PermissionCheck::Denied => {
                        return PermissionResult::Deny(PermissionResultDeny {
                            message: "Denied by permission settings".to_string(),
                            interrupt: false,
                        });
                    }
                    crate::permissions::PermissionCheck::Ask => {
                        // Need to ask the user - continue below
                    }
                }

                // Store in-memory and get receiver for response
                let receiver = crate::pending_requests::pending_requests()
                    .insert_permission(permission_request.clone(), session_id);

                // Send notification
                if let Err(e) = crate::notifications::notify_permission_request(&sync_ctx.settings, &tool_name) {
                    tracing::warn!("Failed to send notification: {}", e);
                }

                // Wait for user decision with timeout
                let timeout = crate::pending_requests::DEFAULT_REQUEST_TIMEOUT;
                match tokio::time::timeout(timeout, receiver).await {
                    Ok(Ok(decision)) => {
                        let approved = decision.is_allowed();

                        // Handle the decision (update runtime permissions, persist to settings)
                        {
                            let mut ctx = sync_ctx.clone().into_thread_local();
                            let mut perms = runtime_permissions.lock().unwrap();
                            if let Err(e) = decision.handle(
                                &permission_request,
                                &mut perms,
                                &mut ctx,
                                session_id,
                            ) {
                                tracing::warn!("Failed to handle permission decision: {}", e);
                            }
                        }

                        if approved {
                            PermissionResult::Allow(PermissionResultAllow {
                                updated_input: Some(tool_input.clone()),
                                ..Default::default()
                            })
                        } else {
                            PermissionResult::Deny(PermissionResultDeny {
                                message: "Denied by user".to_string(),
                                interrupt: false,
                            })
                        }
                    }
                    Ok(Err(_)) => {
                        // Sender dropped (session cancelled)
                        PermissionResult::Deny(PermissionResultDeny {
                            message: "Permission request cancelled".to_string(),
                            interrupt: false,
                        })
                    }
                    Err(_) => {
                        // Timeout - clean up
                        crate::pending_requests::pending_requests().remove_permission(&request_id);
                        PermissionResult::Deny(PermissionResultDeny {
                            message: "Permission request timed out".to_string(),
                            interrupt: false,
                        })
                    }
                }
            }
            .boxed()
        },
    )
}

/// Handle AskUserQuestion tool - waits for user answers via in-memory channel
async fn handle_ask_user_question(
    sync_ctx: ThreadSafeContext,
    stack_id: gitbutler_stack::StackId,
    tool_input: serde_json::Value,
) -> claude_agent_sdk_rs::PermissionResult {
    use claude_agent_sdk_rs::{PermissionResult, PermissionResultAllow};
    let _ = sync_ctx; // Silence unused warning - may be needed for future enhancements

    // Parse the questions from the input
    let questions: Vec<crate::AskUserQuestion> = match tool_input.get("questions") {
        Some(q) => match serde_json::from_value(q.clone()) {
            Ok(questions) => questions,
            Err(e) => {
                tracing::error!("Failed to parse AskUserQuestion questions: {}", e);
                return PermissionResult::Allow(PermissionResultAllow {
                    updated_input: Some(tool_input),
                    ..Default::default()
                });
            }
        },
        None => {
            tracing::error!("AskUserQuestion input missing 'questions' field");
            return PermissionResult::Allow(PermissionResultAllow {
                updated_input: Some(tool_input),
                ..Default::default()
            });
        }
    };

    // Generate a unique ID for this request
    let request_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().naive_utc();

    let request = crate::ClaudeAskUserQuestionRequest {
        id: request_id.clone(),
        created_at: now,
        updated_at: now,
        questions,
        answers: None,
        stack_id: Some(stack_id),
    };

    // Generate a session_id for tracking
    // Note: We use a new UUID for each request. Session cancellation will need to be handled
    // by looking up requests by stack_id instead.
    let session_id_for_tracking = uuid::Uuid::new_v4();

    // Store in-memory and get receiver for response
    let receiver = crate::pending_requests::pending_requests()
        .insert_question(request, session_id_for_tracking);

    // Send notification
    if let Err(e) = crate::notifications::notify_permission_request(&sync_ctx.settings, "AskUserQuestion") {
        tracing::warn!("Failed to send AskUserQuestion notification: {}", e);
    }

    // Wait for user answers with timeout
    let timeout = crate::pending_requests::DEFAULT_REQUEST_TIMEOUT;
    match tokio::time::timeout(timeout, receiver).await {
        Ok(Ok(answers)) => {
            // Build updated input with answers
            let mut updated_input = tool_input.clone();
            if let Some(obj) = updated_input.as_object_mut() {
                obj.insert("answers".to_string(), serde_json::json!(answers));
            }

            PermissionResult::Allow(PermissionResultAllow {
                updated_input: Some(updated_input),
                ..Default::default()
            })
        }
        Ok(Err(_)) => {
            // Sender dropped (session cancelled) - allow with original input
            tracing::warn!("AskUserQuestion request cancelled");
            PermissionResult::Allow(PermissionResultAllow {
                updated_input: Some(tool_input),
                ..Default::default()
            })
        }
        Err(_) => {
            // Timeout - clean up and allow with original input
            crate::pending_requests::pending_requests().remove_question(&request_id);
            tracing::warn!("AskUserQuestion timeout after 24 hours");
            PermissionResult::Allow(PermissionResultAllow {
                updated_input: Some(tool_input),
                ..Default::default()
            })
        }
    }
}

/// Creates a PreToolUse hook that performs file locking and keeps the stream open.
///
/// This hook is required for two reasons:
/// 1. The Python SDK docs state: "can_use_tool requires streaming mode
///    and a PreToolUse hook that returns {"continue_": True} to keep the stream open.
///    Without this hook, the stream closes before the permission callback can be invoked."
/// 2. It performs file locking to track which files are being edited during the session,
///    matching the behavior of the binary path.
#[allow(unused_variables)]
fn create_pretool_use_hook(
    sync_ctx: ThreadSafeContext,
    stack_id: gitbutler_stack::StackId,
) -> claude_agent_sdk_rs::HookCallback {
    use claude_agent_sdk_rs::{HookContext, HookInput, HookJsonOutput, SyncHookJsonOutput};
    use futures::FutureExt;
    use std::sync::Arc;

    Arc::new(
        move |input: HookInput, _tool_use_id: Option<String>, _context: HookContext| {
            async move {
                if let HookInput::PreToolUse(pre_tool_input) = input {
                    // Extract file_path from tool_input for file locking
                    let file_path = pre_tool_input.tool_input.get("file_path").and_then(|v| v.as_str());

                    if let Some(file_path) = file_path {
                        tracing::info!(
                            "PreToolUse hook: tool_name={}, session_id={}, file_path={}",
                            pre_tool_input.tool_name,
                            pre_tool_input.session_id,
                            file_path
                        );

                        // Perform file locking using the SDK variant
                        if let Err(e) =
                            crate::hooks::handle_pre_tool_call_for_sdk(&pre_tool_input.session_id, file_path)
                        {
                            tracing::warn!("PreToolUse file locking failed: {}", e);
                            // Continue even if locking fails - don't block the tool execution
                        }
                    }
                }

                // Always return continue=true to keep the stream open
                HookJsonOutput::Sync(SyncHookJsonOutput {
                    continue_: Some(true),
                    ..Default::default()
                })
            }
            .boxed()
        },
    )
}

/// Creates a PostToolUse hook that assigns hunks to the session's stack.
/// This is critical - without it, changes made by Claude won't be assigned to the stack
/// and won't be committed when the session ends.
fn create_post_tool_use_hook() -> claude_agent_sdk_rs::HookCallback {
    use claude_agent_sdk_rs::{HookContext, HookInput, HookJsonOutput, SyncHookJsonOutput};
    use futures::FutureExt;
    use std::sync::Arc;

    Arc::new(
        move |input: HookInput, _tool_use_id: Option<String>, _context: HookContext| {
            async move {
                tracing::info!(
                    "PostToolUse hook called, input type: {:?}",
                    std::mem::discriminant(&input)
                );
                if let HookInput::PostToolUse(post_tool_input) = input {
                    tracing::info!(
                        "PostToolUse hook: tool_name={}, session_id={}, tool_response keys: {:?}",
                        post_tool_input.tool_name,
                        post_tool_input.session_id,
                        post_tool_input
                            .tool_response
                            .as_object()
                            .map(|o| o.keys().collect::<Vec<_>>())
                    );

                    // Extract file_path and structured_patch from tool_response
                    let file_path = post_tool_input
                        .tool_response
                        .get("filePath")
                        .or_else(|| post_tool_input.tool_response.get("file_path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let structured_patch: Vec<crate::hooks::StructuredPatch> = post_tool_input
                        .tool_response
                        .get("structuredPatch")
                        .or_else(|| post_tool_input.tool_response.get("structured_patch"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();

                    tracing::info!(
                        "PostToolUse hook: file_path='{}', structured_patch count={}",
                        file_path,
                        structured_patch.len()
                    );

                    if file_path.is_empty() {
                        tracing::warn!(
                            "PostToolUse hook: file_path is empty, skipping. Full tool_response: {:?}",
                            post_tool_input.tool_response
                        );
                        return HookJsonOutput::Sync(SyncHookJsonOutput {
                            continue_: Some(true),
                            ..Default::default()
                        });
                    }

                    // Use the _for_sdk variant which skips the GUI check (SDK hooks always run in GUI context)
                    match crate::hooks::handle_post_tool_call_from_input_for_sdk(
                        &post_tool_input.session_id,
                        file_path,
                        &structured_patch,
                    ) {
                        Ok(output) => {
                            tracing::info!(
                                "PostToolUse hook: handler succeeded, should_continue={}",
                                output.should_continue()
                            );
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(output.should_continue()),
                                ..Default::default()
                            })
                        }
                        Err(e) => {
                            tracing::warn!("PostToolUse hook failed: {}", e);
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(true),
                                ..Default::default()
                            })
                        }
                    }
                } else {
                    tracing::debug!("PostToolUse hook: received non-PostToolUse input, ignoring");
                    HookJsonOutput::Sync(SyncHookJsonOutput {
                        continue_: Some(true),
                        ..Default::default()
                    })
                }
            }
            .boxed()
        },
    )
}

/// Creates a Stop hook that handles commit creation when Claude finishes.
/// This is the SDK equivalent of the binary's Stop hook configured via --settings.
fn create_stop_hook() -> claude_agent_sdk_rs::HookCallback {
    use claude_agent_sdk_rs::{HookContext, HookInput, HookJsonOutput, SyncHookJsonOutput};
    use futures::FutureExt;
    use std::sync::Arc;

    Arc::new(
        move |input: HookInput, _tool_use_id: Option<String>, _context: HookContext| {
            async move {
                tracing::info!("Stop hook called, input type: {:?}", std::mem::discriminant(&input));
                if let HookInput::Stop(stop_input) = input {
                    tracing::info!(
                        "Stop hook: session_id={}, transcript_path={}",
                        stop_input.session_id,
                        stop_input.transcript_path
                    );
                    // Use the _for_sdk variant which skips the GUI check (SDK hooks always run in GUI context)
                    match crate::hooks::handle_stop_from_input_for_sdk(
                        &stop_input.session_id,
                        &stop_input.transcript_path,
                    ) {
                        Ok(output) => {
                            tracing::info!(
                                "Stop hook: handler succeeded, should_continue={}",
                                output.should_continue()
                            );
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(output.should_continue()),
                                ..Default::default()
                            })
                        }
                        Err(e) => {
                            tracing::warn!("Stop hook failed: {}", e);
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(true),
                                ..Default::default()
                            })
                        }
                    }
                } else {
                    HookJsonOutput::Sync(SyncHookJsonOutput {
                        continue_: Some(true),
                        ..Default::default()
                    })
                }
            }
            .boxed()
        },
    )
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
        Ok(output) if output.status.success() => {
            let version = str::from_utf8(&output.stdout).unwrap_or("").trim().into();
            ClaudeCheckResult::Available { version }
        }
        Ok(output) if !output.status.success() => {
            let stdout = String::from_utf8(output.stdout).unwrap_or_default();
            let stderr = String::from_utf8(output.stderr).unwrap_or_default();
            ClaudeCheckResult::ExecutionFailed { stdout, stderr }
        }
        _ => ClaudeCheckResult::NotAvailable,
    }
}
