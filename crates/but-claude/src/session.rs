//! Claude bridge.
//!
//! This module provides the integration between GitButler's frontend and Claude Code
//! using a claude agent rust SDK. It manages Claude sessions, handles tool permissions,
//! and coordinates hooks for file tracking and commit creation.
//!
//! Key components:
//! - `Claudes`: Manages active Claude sessions, keyed by stack ID
//! - `spawn_claude_inner`: Main entry point that connects to Claude SDK and streams responses
//! - Permission handling via `can_use_tool` callback for tool approvals and AskUserQuestion
//! - Hook callbacks (PreToolUse, PostToolUse, Stop) for file locking, hunk assignment, and commits

use std::{collections::HashMap, sync::Arc};

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
    process::Command,
    sync::{
        Mutex,
        mpsc::{UnboundedSender, unbounded_channel},
    },
};

use crate::{
    Broadcaster, ClaudeMessage, ClaudeOutput, ClaudeUserParams, MessagePayload, PermissionMode, PromptAttachment,
    SystemMessage, ThinkingLevel, Transcript, UserInput,
    broadcaster::FrontendEvent,
    claude_mcp::ClaudeProjectConfig,
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
        self: &Arc<Self>,
        ctx: ThreadSafeContext,
        broadcaster: Arc<Mutex<Broadcaster>>,
        stack_id: StackId,
        user_params: ClaudeUserParams,
    ) -> Result<()> {
        if self.requests.lock().await.contains_key(&stack_id) {
            bail!(
                "Claude is currently thinking, please wait for it to complete before sending another message.\n\nIf claude is stuck thinking, try restarting the application."
            );
        }

        // Verify the project has been registered with Claude Code.
        {
            let workdir = ctx.clone().into_thread_local().workdir_or_fail()?;
            if !crate::claude_mcp::is_project_registered(&workdir).await {
                bail!(
                    "This project has not been set up with Claude Code yet. \
                     Please run `claude` in the project directory first."
                );
            }
        }

        // Spawn the Claude session as a background task to avoid blocking the caller.
        // The session streams results via the broadcaster, so the frontend gets updates in real-time.
        let claudes = Arc::clone(self);
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            claudes
                .spawn_claude(ctx.clone(), broadcaster.clone(), stack_id, user_params)
                .await;

            if let Err(e) = claudes.maybe_compact_context(ctx, broadcaster, stack_id).await {
                tracing::error!(
                    stack_id = ?stack_id,
                    error = %e,
                    duration_ms = start.elapsed().as_millis(),
                    "Claude session cleanup (compaction check) failed"
                );
            }
        });

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
    ) {
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
                let broadcaster = broadcaster.lock().await;
                let mut ctx = sync_ctx.into_thread_local();
                let _ = send_claude_message(
                    &mut ctx,
                    &broadcaster,
                    rule.session_id,
                    stack_id,
                    MessagePayload::System(crate::SystemMessage::UnhandledException {
                        message: format!("{res}"),
                    }),
                );
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

        // Clean up any stale pending requests from previous sessions
        let expired_count = crate::pending_requests::pending_requests()
            .cleanup_expired(crate::pending_requests::DEFAULT_REQUEST_TIMEOUT);
        if expired_count > 0 {
            tracing::debug!("Cleaned up {} expired pending requests", expired_count);
        }

        let (send_kill, mut recv_kill) = unbounded_channel();
        self.requests
            .lock()
            .await
            .insert(stack_id, Arc::new(Claude { kill: send_kill }));

        let SessionSetup {
            summary_to_resume,
            original_session_id,
            session,
            project_workdir,
        } = setup_session(&sync_ctx, stack_id)?;

        let transcript_current_id = Transcript::current_valid_session_id(&project_workdir, &session).await?;

        // IMPORTANT: Two session IDs are used throughout this function:
        //
        // 1. `original_session_id` - Stable identifier that never changes
        //    - Stored in database as ClaudeSession.id
        //    - Used for: Database messages, frontend communication, permission storage
        //    - Persists across: Compaction, restarts, resumption
        //    - One per stack (conversation)
        //
        // 2. `current_session_id` - Claude SDK's session identifier
        //    - Changes after compaction to avoid context overflow
        //    - Used for: SDK initialization, pending requests, hooks, transcript files
        //    - Stored in database as ClaudeSession.current_id
        //    - SDK hooks receive this ID and map back to original_session_id
        //
        // Rule of thumb: Use original_session_id for messages/UI, current_session_id for SDK/hooks

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
        send_message_with_lock(
            &sync_ctx,
            &broadcaster,
            original_session_id,
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

        // Determine the current session ID (SDK session ID):
        // - If resuming after compaction (summary_to_resume.is_some()), use a new random ID
        // - If resuming an existing session (transcript_current_id), use that ID
        // - If starting new, use the stable session.id
        let current_session_id = if summary_to_resume.is_some() {
            // After compaction, start with a new session ID
            uuid::Uuid::new_v4()
        } else if let Some(current_id) = transcript_current_id {
            // If resuming, use the existing current_id
            current_id
        } else {
            // If starting new, ensure there isn't an existing invalid transcript
            let path = Transcript::get_transcript_path(&project_workdir, session.id)?;
            if fs::try_exists(&path).await? {
                fs::remove_file(&path).await?;
            }
            // Use the stable session.id
            session.id
        };

        // Persist the current session ID to the database so hooks can map current_session_id
        // back to our stable original_session_id.
        {
            let mut ctx = sync_ctx.clone().into_thread_local();
            if let Err(e) = db::add_session_id(&mut ctx, original_session_id, current_session_id) {
                tracing::error!(
                    original_session_id = %original_session_id,
                    current_session_id = %current_session_id,
                    error = %e,
                    "Failed to persist current_session_id to database - hooks may fail to find session"
                );
            }
        }

        // Build MCP server configuration
        let cc_settings = ClaudeSettings::open(&project_workdir).await;
        let project_config = ClaudeProjectConfig::open(&cc_settings, &project_workdir).await;
        let disabled_servers: Vec<&str> = user_params.disabled_mcp_servers.iter().map(String::as_str).collect();
        let mcp_servers = project_config.mcp_servers_for_sdk(&disabled_servers);

        // Build system prompt with branch info
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
        // Only set model if useConfiguredModel is false
        let model = if sync_ctx.settings.claude.use_configured_model {
            None
        } else {
            Some(user_params.model.to_cli_string().to_string())
        };

        // Determine resume behavior:
        // - summary_to_resume.is_some(): Don't resume, start fresh with summary context (use --session-id)
        // - transcript_current_id.is_some() && summary_to_resume.is_none(): Resume existing session (use --resume)
        // - Otherwise: Start new session (use --session-id)
        let (resume, extra_args) = if summary_to_resume.is_none() && transcript_current_id.is_some() {
            // Resume existing session
            (Some(current_session_id.to_string()), HashMap::new())
        } else {
            // Start new session (or after compaction) - pass session-id via extra_args
            let mut args = HashMap::new();
            args.insert("session-id".to_string(), Some(current_session_id.to_string()));
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
            broadcaster.clone(),
            stack_id,
            dangerously_skip_permissions,
            current_session_id,
        );
        // PreToolUse hook for file locking and keeping stream open for can_use_tool
        let pretool_hook = create_pretool_use_hook(sync_ctx.clone(), stack_id);
        // Add PostToolUse hook to assign hunks to the session's stack (critical for commit creation)
        let posttool_hook = create_post_tool_use_hook(sync_ctx.clone());
        // Add Stop hook to handle commit creation when Claude finishes
        let stop_hook = create_stop_hook(sync_ctx.clone());
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

        // Use the configured executable path from settings
        let cli_path = if sync_ctx.settings.claude.executable.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(&sync_ctx.settings.claude.executable))
        };

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
            cli_path,
            ..Default::default()
        };

        // Create client and connect
        let mut client = ClaudeClient::new(options);
        if let Err(e) = client.connect().await {
            tracing::error!(
                stack_id = ?stack_id,
                original_session_id = %original_session_id,
                error = %e,
                "Failed to connect to Claude SDK client"
            );
            self.requests.lock().await.remove(&stack_id);
            // Cancel pending requests using current_session_id (SDK ID)
            crate::pending_requests::pending_requests().cancel_session(current_session_id);
            if let Err(send_err) = send_message_with_lock(
                &sync_ctx,
                &broadcaster,
                original_session_id,
                stack_id,
                MessagePayload::System(SystemMessage::UnhandledException {
                    message: format!("Failed to connect to Claude SDK: {e}"),
                }),
            )
            .await
            {
                tracing::error!(error = %send_err, "Failed to send connection error message to frontend");
            }
            return Err(e.into());
        }
        tracing::debug!(stack_id = ?stack_id, original_session_id = %original_session_id, "Claude SDK client connected");

        // Note: Session ID was already persisted earlier (before MCP config was built)
        // to ensure the MCP server can find the session when it starts.

        // Prepare and send the message
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
            // Cancel pending requests using current_session_id (SDK ID)
            crate::pending_requests::pending_requests().cancel_session(current_session_id);
            client.disconnect().await?;
            send_message_with_lock(
                &sync_ctx,
                &broadcaster,
                original_session_id,
                stack_id,
                MessagePayload::System(SystemMessage::UnhandledException {
                    message: format!("Failed to send query to Claude: {e}"),
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
                            tracing::trace!(
                                stack_id = ?stack_id,
                                message_type = ?std::mem::discriminant(&sdk_message),
                                "Received SDK message"
                            );
                            match sdk_message {
                                SdkMessage::Assistant(assistant_msg) => {
                                    tracing::info!(
                                        stack_id = ?stack_id,
                                        original_session_id = %original_session_id,
                                        "Received assistant message from SDK"
                                    );
                                    // Convert SDK message to ClaudeOutput format
                                    let mut data = serde_json::to_value(&assistant_msg)?;
                                    if let Some(obj) = data.as_object_mut() {
                                        obj.insert("type".to_string(), serde_json::json!("assistant"));
                                    }
                                    send_message_with_lock(
                                        &sync_ctx,
                                        &broadcaster,
                                        original_session_id,
                                        stack_id,
                                        MessagePayload::Claude(ClaudeOutput { data }),
                                    ).await?;
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

                                    // Note: AskUserQuestion answers are injected via the can_use_tool callback's
                                    // updated_input field, not by modifying the tool result here.

                                    send_message_with_lock(
                                        &sync_ctx,
                                        &broadcaster,
                                        original_session_id,
                                        stack_id,
                                        MessagePayload::Claude(ClaudeOutput { data }),
                                    ).await?;
                                }
                                SdkMessage::Result(result_msg) => {
                                    send_message_with_lock(
                                        &sync_ctx,
                                        &broadcaster,
                                        original_session_id,
                                        stack_id,
                                        MessagePayload::System(SystemMessage::ClaudeExit {
                                            code: if result_msg.is_error { 1 } else { 0 },
                                            message: result_msg.result.unwrap_or_default(),
                                        }),
                                    ).await?;
                                    break;
                                }
                                // System and StreamEvent messages are informational
                                _ => {
                                    tracing::trace!(
                                        stack_id = ?stack_id,
                                        "Received informational SDK message (System/StreamEvent)"
                                    );
                                }
                            }
                        }
                        Some(Err(e)) => {
                            send_message_with_lock(
                                &sync_ctx,
                                &broadcaster,
                                original_session_id,
                                stack_id,
                                MessagePayload::System(SystemMessage::UnhandledException {
                                    message: format!("SDK error: {e}"),
                                }),
                            ).await?;
                            break;
                        }
                        None => {
                            // Stream ended without a Result message - unexpected termination
                            send_message_with_lock(
                                &sync_ctx,
                                &broadcaster,
                                original_session_id,
                                stack_id,
                                MessagePayload::System(SystemMessage::ClaudeExit {
                                    code: 1,
                                    message: "Claude session ended unexpectedly".to_string(),
                                }),
                            ).await?;
                            break;
                        }
                    }
                }
                _ = recv_kill.recv() => {
                    tracing::warn!(
                        stack_id = ?stack_id,
                        original_session_id = %original_session_id,
                        "Kill signal received - cancelling Claude session"
                    );

                    if let Err(e) = client.interrupt().await {
                        tracing::warn!(error = %e, "Failed to send interrupt signal to Claude CLI");
                    }

                    if let Err(e) = send_message_with_lock(
                        &sync_ctx,
                        &broadcaster,
                        original_session_id,
                        stack_id,
                        MessagePayload::System(SystemMessage::UserAbort),
                    ).await {
                        tracing::warn!(error = %e, "Failed to send UserAbort message during cancellation");
                    }
                    break;
                }
            }
        }

        drop(stream);
        self.requests.lock().await.remove(&stack_id);
        crate::pending_requests::pending_requests().cancel_session(current_session_id);

        if let Err(e) = client.disconnect().await {
            tracing::warn!(
                stack_id = ?stack_id,
                original_session_id = %original_session_id,
                error = %e,
                "Failed to cleanly disconnect Claude SDK client"
            );
        }

        if let Err(e) = broadcast_gitbutler_messages(
            &sync_ctx,
            &broadcaster,
            original_session_id,
            stack_id,
            session_start_time,
        )
        .await
        {
            tracing::warn!(
                original_session_id = %original_session_id,
                error = %e,
                "Failed to broadcast GitButler messages created during session"
            );
        }
        send_completion_notification(&sync_ctx);

        Ok(())
    }
}

/// Result of setting up a Claude session.
struct SessionSetup {
    /// Summary from context compaction, if resuming after compaction
    summary_to_resume: Option<String>,
    /// Stable session ID that never changes (ClaudeSession.id)
    original_session_id: uuid::Uuid,
    /// The session object from the database
    session: crate::ClaudeSession,
    /// The project working directory
    project_workdir: std::path::PathBuf,
}

/// Helper to send a Claude message by acquiring broadcaster lock and creating thread-local context.
async fn send_message_with_lock(
    sync_ctx: &ThreadSafeContext,
    broadcaster: &Arc<Mutex<Broadcaster>>,
    session_id: uuid::Uuid,
    stack_id: StackId,
    payload: MessagePayload,
) -> Result<()> {
    let broadcaster = broadcaster.lock().await;
    let mut ctx = sync_ctx.clone().into_thread_local();
    send_claude_message(&mut ctx, &broadcaster, session_id, stack_id, payload)
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

    let original_session_id = rule.map(|r| r.session_id).unwrap_or(uuid::Uuid::new_v4());
    let session = upsert_session(&mut ctx, original_session_id, stack_id, guard.write_permission())?;
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
        original_session_id,
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
) -> Result<()> {
    let project_id = sync_ctx.legacy_project.id;
    let all_messages = {
        let ctx = sync_ctx.clone().into_thread_local();
        db::list_messages_by_session(&ctx, session_id)?
    };

    let new_messages: Vec<_> = all_messages
        .into_iter()
        .filter(|msg| matches!(msg.payload, MessagePayload::GitButler(_)))
        .filter(|msg| msg.created_at > session_start_time)
        .collect();

    for message in new_messages {
        broadcaster.lock().await.send(FrontendEvent {
            name: format!("project://{project_id}/claude/{stack_id}/message_received"),
            payload: serde_json::json!(message),
        });
    }

    Ok(())
}

/// Sends completion notification if configured.
fn send_completion_notification(sync_ctx: &ThreadSafeContext) {
    if let Err(e) = crate::notifications::notify_completion(&sync_ctx.settings) {
        tracing::warn!("Failed to send completion notification: {}", e);
    }
}

fn system_prompt() -> String {
    let but_path = get_cli_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "but".to_string());

    format!(
        "<git-usage>
CRITICAL: You are working on a project that is managed by GitButler.

## General Principle

When working with commits (creating, modifying, or reorganizing), ALWAYS use the `{but_path}` CLI.
Only use git commands for READ-ONLY operations like viewing history, diffs, or logs.

## PROHIBITED Git Commands

You MUST NOT run the following git commands:
- git status (file change info is provided in <branch-info> below)
- git commit (use `{but_path} commit` instead)
- git checkout
- git squash
- git rebase
- git cherry-pick

These commands modify branches or provide information already available to you.

## What You CAN Do

- Run git commands that give read-only information about the repository (git log, git diff, etc.)
- Use the GitButler CLI (`{but_path}`) to perform disallowed actions
- Reference file changes and uncommitted changes from the <branch-info> section provided below

## Using the GitButler CLI

Disallowed actions can instead be performed using `{but_path}`.
For help with available commands, consult `{but_path} --help`.

### Common Commands

**Viewing changes:**
- `{but_path} status` - View changes assigned to this branch

**Creating commits:**
- `{but_path} commit -m \"message\"` - Commit changes to this branch

**Modifying commits:**
- `{but_path} describe <commit>` - Edit a commit message
- `{but_path} absorb` - Absorb uncommitted changes into existing commits automatically
- `{but_path} rub <source> <target>` - Move changes between commits, squash, amend, or assign files

**JSON Output:**
Many `{but_path}` commands support the `--json` flag, which provides structured output that is easier to parse programmatically. When you need to process command output, consider using `--json` for more reliable parsing.

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
</git-usage>"
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
                    replace HEAD with the current working branch name: `{first_branch_name}`\n\n"
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
        output.push_str(&format!("- {file_path}\n"));
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
    // Reject: <, >, ", ', newlines, carriage returns, and null bytes
    let suspicious_chars = ['<', '>', '"', '\'', '\n', '\r', '\0'];
    if path.chars().any(|c| suspicious_chars.contains(&c)) {
        bail!("Path contains invalid characters: {path}");
    }

    // Try to parse as a Path to ensure it's valid and can be converted back to a string
    let path_buf = std::path::Path::new(path);
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
        bail!("Commit ID contains non-hexadecimal characters: {commit_id}");
    }

    Ok(())
}

/// Process file attachments by writing them to temporary files in the project directory
/// and enhancing the message to reference these files
async fn format_message_with_attachments(original_message: &str, attachments: &[PromptAttachment]) -> Result<String> {
    if attachments.is_empty() {
        return Ok(original_message.to_string());
    }

    for attachment in attachments {
        validate_attachment(attachment)?;
    }

    let attachments_json = serde_json::to_string_pretty(&attachments)?;

    let message = format!(
        "{original_message}

<context-attachments>
The following JSON of files, line ranges, and commits have been added as
context. Please consider them if a question or reference to files, lines,
or commits, is unspecified.
<attachments>
{attachments_json}
</attachments>
</context-attachments>
"
    );

    Ok(message)
}

/// Creates a can_use_tool callback that handles tool permissions and AskUserQuestion.
///
/// This callback is invoked by the SDK for every tool call and handles:
///
/// 1. **AskUserQuestion**: Waits for user answers via in-memory channel, then returns
///    `PermissionResultAllow` with `updated_input` containing the answers. The CLI uses
///    these answers directly instead of executing the built-in AskUserQuestion tool.
///
/// 2. **Other tools**: Checks permissions against runtime and session permissions.
///    - If `auto_approve_tools` is true (bypass mode), auto-approve everything.
///    - Otherwise, check stored permissions; if no match, prompt the user and wait
///      for their decision via in-memory channel.
fn create_can_use_tool_callback(
    sync_ctx: ThreadSafeContext,
    broadcaster: Arc<Mutex<Broadcaster>>,
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
            let broadcaster = broadcaster.clone();
            let stack_id = stack_id;
            let auto_approve = auto_approve_tools;
            let session_id = session_id;
            let runtime_permissions = Arc::clone(&runtime_permissions);

            async move {
                // Handle AskUserQuestion specially - poll for user answers
                if tool_name == "AskUserQuestion" {
                    return handle_ask_user_question(sync_ctx, stack_id, session_id, tool_input).await;
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
                // Wrap in spawn_blocking since database access is blocking I/O
                let check_result = {
                    let sync_ctx_clone = sync_ctx.clone();
                    let permission_request_clone = permission_request.clone();
                    let runtime_perms_snapshot = {
                        let perms = runtime_permissions.lock().unwrap();
                        perms.clone()
                    };

                    let result = tokio::task::spawn_blocking(move || {
                        let ctx = sync_ctx_clone.into_thread_local();

                        // Load session permissions
                        let session_perms = match crate::db::get_session_by_id(&ctx, session_id) {
                            Ok(Some(session)) => crate::permissions::Permissions::from_slices(
                                session.approved_permissions(),
                                session.denied_permissions(),
                            ),
                            _ => crate::permissions::Permissions::default(),
                        };

                        // Merge runtime and session permissions
                        let combined_perms =
                            crate::permissions::Permissions::merge([&runtime_perms_snapshot, &session_perms]);

                        combined_perms.check(&permission_request_clone).unwrap_or_default()
                    })
                    .await;

                    match result {
                        Ok(check) => {
                            tracing::debug!(
                                tool = %tool_name,
                                result = ?check,
                                "Permission check completed"
                            );
                            check
                        }
                        Err(e) => {
                            tracing::error!(
                                tool = %tool_name,
                                error = %e,
                                "Permission check spawn_blocking failed - possible thread pool exhaustion, defaulting to Ask"
                            );
                            crate::permissions::PermissionCheck::Ask
                        }
                    }
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

                // Broadcast to frontend so it can display the permission request
                let project_id = sync_ctx.legacy_project.id;
                broadcaster.lock().await.send(FrontendEvent {
                    name: format!("project://{project_id}/claude-permission-requests"),
                    payload: serde_json::json!({
                        "kind": "claude-permission-requests"
                    }),
                });

                // Send notification
                if let Err(e) = crate::notifications::notify_permission_request(&sync_ctx.settings, &tool_name) {
                    tracing::warn!("Failed to send notification: {}", e);
                }

                // Wait for user decision with timeout
                let timeout = crate::pending_requests::DEFAULT_REQUEST_TIMEOUT;
                match tokio::time::timeout(timeout, receiver).await {
                    Ok(Ok((decision, use_wildcard))) => {
                        let approved = decision.is_allowed();

                        // Handle the decision (update runtime permissions, persist to settings)
                        // Wrap in spawn_blocking since decision.handle involves blocking I/O
                        {
                            let sync_ctx_clone = sync_ctx.clone();
                            // Apply the use_wildcard preference from the user's response
                            let mut permission_request_clone = permission_request.clone();
                            permission_request_clone.use_wildcard = use_wildcard;
                            let current_perms = {
                                let perms = runtime_permissions.lock().unwrap();
                                perms.clone()
                            };

                            let handle_result = tokio::task::spawn_blocking(move || {
                                let mut ctx = sync_ctx_clone.into_thread_local();
                                let mut perms = current_perms;
                                let result =
                                    decision.handle(&permission_request_clone, &mut perms, &mut ctx, session_id);
                                (result, perms)
                            })
                            .await;

                            match handle_result {
                                Ok((Ok(()), updated_perms)) => {
                                    // Update runtime permissions with the modified copy
                                    let mut perms = runtime_permissions.lock().unwrap();
                                    *perms = updated_perms;
                                    tracing::debug!(
                                        tool = %tool_name,
                                        "Permission decision handled and runtime permissions updated"
                                    );
                                }
                                Ok((Err(e), _)) => {
                                    tracing::warn!(
                                        tool = %tool_name,
                                        error = %e,
                                        "Failed to handle permission decision - permission may not be persisted"
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(
                                        tool = %tool_name,
                                        error = %e,
                                        "Permission decision spawn_blocking failed - possible thread pool exhaustion"
                                    );
                                }
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
    session_id: uuid::Uuid,
    tool_input: serde_json::Value,
) -> claude_agent_sdk_rs::PermissionResult {
    use claude_agent_sdk_rs::{PermissionResult, PermissionResultAllow, PermissionResultDeny};

    // Parse the questions from the input
    let questions: Vec<crate::AskUserQuestion> = match tool_input.get("questions") {
        Some(q) => match serde_json::from_value(q.clone()) {
            Ok(questions) => questions,
            Err(e) => {
                tracing::error!("Failed to parse AskUserQuestion questions: {}", e);
                return PermissionResult::Deny(PermissionResultDeny {
                    message: format!("Failed to parse AskUserQuestion: {e}"),
                    interrupt: false,
                });
            }
        },
        None => {
            tracing::error!("AskUserQuestion input missing 'questions' field");
            return PermissionResult::Deny(PermissionResultDeny {
                message: "AskUserQuestion input missing 'questions' field".to_string(),
                interrupt: false,
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

    // Store in-memory and get receiver for response
    // Use the actual session_id so cancel_session() can properly cancel pending AskUserQuestion requests
    let receiver = crate::pending_requests::pending_requests().insert_question(request, session_id);

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
            // Sender dropped (session cancelled)
            tracing::warn!("AskUserQuestion request cancelled");
            PermissionResult::Deny(PermissionResultDeny {
                message: "AskUserQuestion request cancelled".to_string(),
                interrupt: false,
            })
        }
        Err(_) => {
            // Timeout - clean up
            crate::pending_requests::pending_requests().remove_question(&request_id);
            tracing::warn!("AskUserQuestion timeout after 24 hours");
            PermissionResult::Deny(PermissionResultDeny {
                message: "AskUserQuestion request timed out".to_string(),
                interrupt: false,
            })
        }
    }
}

/// Creates a PreToolUse hook that performs file locking and keeps the stream open.
///
/// This hook serves two purposes:
/// 1. Returns `continue_: true` to keep the stream open, which is required for the
///    `can_use_tool` callback to work correctly with the SDK's streaming mode.
/// 2. Performs file locking to track which files are being edited during the session,
///    preventing conflicts when Claude modifies files.
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
            let sync_ctx = sync_ctx.clone();
            async move {
                if let HookInput::PreToolUse(pre_tool_input) = input {
                    let file_path = pre_tool_input.tool_input.get("file_path").and_then(|v| v.as_str());
                    if let Some(file_path) = file_path {
                        let session_id_str = pre_tool_input.session_id;
                        let session_id = match uuid::Uuid::parse_str(&session_id_str) {
                            Ok(id) => id,
                            Err(e) => {
                                tracing::warn!(
                                    session_id = %session_id_str,
                                    error = %e,
                                    "PreToolUse hook received invalid session ID - skipping file lock"
                                );
                                return HookJsonOutput::Sync(SyncHookJsonOutput {
                                    continue_: Some(true),
                                    ..Default::default()
                                });
                            }
                        };
                        let file_path_owned = file_path.to_string();
                        let result = tokio::task::spawn_blocking(move || {
                            crate::hooks::lock_file_for_tool_call(sync_ctx, session_id, &file_path_owned)
                        })
                        .await;

                        match result {
                            Ok(Ok(_)) => {}
                            Ok(Err(e)) => {
                                tracing::warn!(
                                    error = %e,
                                    "PreToolUse file locking failed - continuing without lock"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "PreToolUse file locking task failed"
                                );
                            }
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
fn create_post_tool_use_hook(sync_ctx: ThreadSafeContext) -> claude_agent_sdk_rs::HookCallback {
    use claude_agent_sdk_rs::{HookContext, HookInput, HookJsonOutput, SyncHookJsonOutput};
    use futures::FutureExt;
    use std::sync::Arc;

    Arc::new(
        move |input: HookInput, _tool_use_id: Option<String>, _context: HookContext| {
            let sync_ctx = sync_ctx.clone();
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

                    let session_id_str = post_tool_input.session_id;
                    let session_id = match uuid::Uuid::parse_str(&session_id_str) {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::warn!(
                                session_id = %session_id_str,
                                error = %e,
                                "PostToolUse hook received invalid session ID - skipping hunk assignment"
                            );
                            return HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(true),
                                ..Default::default()
                            });
                        }
                    };
                    let file_path_owned = file_path.to_string();
                    let file_path_for_log = file_path_owned.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        crate::hooks::assign_hunks_post_tool_call(
                            sync_ctx.into_thread_local(),
                            session_id,
                            &file_path_owned,
                            &structured_patch,
                            true,
                        )
                    })
                    .await;

                    match result {
                        Ok(Ok(output)) => {
                            tracing::info!(
                                file_path = %file_path_for_log,
                                should_continue = output.should_continue(),
                                "PostToolUse hook succeeded"
                            );
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(output.should_continue()),
                                ..Default::default()
                            })
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(
                                file_path = %file_path_for_log,
                                error = %e,
                                "PostToolUse hook handler failed - hunk assignment may not be updated"
                            );
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(true),
                                ..Default::default()
                            })
                        }
                        Err(e) => {
                            tracing::error!(
                                file_path = %file_path_for_log,
                                error = %e,
                                "PostToolUse spawn_blocking failed - possible thread pool exhaustion, hunk assignment skipped"
                            );
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
fn create_stop_hook(sync_ctx: ThreadSafeContext) -> claude_agent_sdk_rs::HookCallback {
    use claude_agent_sdk_rs::{HookContext, HookInput, HookJsonOutput, SyncHookJsonOutput};
    use futures::FutureExt;
    use std::sync::Arc;

    Arc::new(
        move |input: HookInput, _tool_use_id: Option<String>, _context: HookContext| {
            let sync_ctx = sync_ctx.clone();
            async move {
                tracing::info!("Stop hook called, input type: {:?}", std::mem::discriminant(&input));
                if let HookInput::Stop(stop_input) = input {
                    tracing::info!(
                        "Stop hook: session_id={}, transcript_path={}",
                        stop_input.session_id,
                        stop_input.transcript_path
                    );
                    let session_id_str = stop_input.session_id;
                    let session_id = match uuid::Uuid::parse_str(&session_id_str) {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::warn!(
                                session_id = %session_id_str,
                                error = %e,
                                "Stop hook received invalid session ID - skipping stop handling"
                            );
                            return HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(true),
                                ..Default::default()
                            });
                        }
                    };
                    let transcript_path = stop_input.transcript_path.clone();
                    let session_id_for_log = session_id_str;
                    let transcript_path_for_log = transcript_path.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        crate::hooks::handle_session_stop(
                            sync_ctx.into_thread_local(),
                            session_id,
                            &transcript_path,
                            true,
                        )
                    })
                    .await;

                    match result {
                        Ok(Ok(output)) => {
                            tracing::info!(
                                session_id = %session_id_for_log,
                                should_continue = output.should_continue(),
                                "Stop hook succeeded"
                            );
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(output.should_continue()),
                                ..Default::default()
                            })
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(
                                session_id = %session_id_for_log,
                                transcript_path = %transcript_path_for_log,
                                error = %e,
                                "Stop hook handler failed - commit creation may have failed"
                            );
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                continue_: Some(true),
                                ..Default::default()
                            })
                        }
                        Err(e) => {
                            tracing::error!(
                                session_id = %session_id_for_log,
                                transcript_path = %transcript_path_for_log,
                                error = %e,
                                "Stop hook spawn_blocking failed - possible thread pool exhaustion, commit creation skipped"
                            );
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
