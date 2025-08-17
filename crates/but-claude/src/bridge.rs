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

use crate::{ClaudeMessage, ClaudeMessageContent, UserInput, claude_transcript::Transcript, db};
use anyhow::{Result, bail};
use but_broadcaster::{Broadcaster, FrontendEvent};
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use serde_json::json;
use std::{
    collections::HashSet,
    io::{BufRead, BufReader},
    sync::Arc,
};
use tokio::{process::Command, sync::Mutex};

/// Holds the CC instances. Currently keyed by stackId, since our current model
/// assumes one CC per stack at any given time.
pub struct Claudes {
    /// A set that contains all the currently running requests
    requests: Mutex<HashSet<StackId>>,
}

pub struct Claude {}

impl Claudes {
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(HashSet::new()),
        }
    }

    pub async fn send_message(
        &self,
        ctx: Mutex<CommandContext>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
        message: &str,
    ) -> Result<()> {
        if self.requests.lock().await.contains(&stack_id) {
            bail!("Claude is thinking, back off!!!")
        } else {
            self.spawn_claude(ctx, broadcaster, stack_id, message.to_owned())
                .await?
        };

        Ok(())
    }

    pub fn get_messages(
        &self,
        ctx: &mut CommandContext,
        stack_id: StackId,
    ) -> Result<Vec<ClaudeMessage>> {
        let messages = db::list_messages_by_session(ctx, stack_id.into())?;
        Ok(messages)
    }

    async fn spawn_claude(
        &self,
        ctx: Mutex<CommandContext>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
        message: String,
    ) -> Result<()> {
        self.requests.lock().await.insert(stack_id);

        // Clone so the reference to ctx can be immediatly dropped
        let project = ctx.lock().await.project().clone();

        // We're also making the bold assumption that if we can find the
        // transcript, that a session was created. This is _not_ the best
        // way to do this.
        //
        // https://github.com/anthropics/claude-code/issues/5161 could
        // simplify this
        let transcript_path = Transcript::get_transcript_path(&project.path, stack_id.into())?;

        let create_new = !transcript_path.try_exists()?;

        let (read_stdout, writer) = std::io::pipe()?;
        let broadcaster = broadcaster.clone();

        // Currently the stack_id is used as the initial "stable" identifier.
        let session_id: uuid::Uuid = stack_id.into();
        let project_id = project.id;

        let session = {
            let mut ctx = ctx.lock().await;
            let session = if let Some(session) = db::get_session_by_id(&mut ctx, session_id)? {
                session
            } else {
                db::save_new_session(&mut ctx, session_id)?
            };

            // Before we save the first line, we want to append the user's side
            let message = db::save_new_message(
                &mut ctx,
                stack_id.into(),
                ClaudeMessageContent::UserInput(UserInput {
                    message: message.clone(),
                }),
            )?;

            broadcaster.lock().await.send(FrontendEvent {
                name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
                payload: json!(message),
            });

            session
        };

        let response_streamer = tokio::spawn(async move {
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
                        db::set_session_current_id(&mut ctx, session_id, current_session_id)
                            .unwrap();
                    }
                    first = false;
                }

                let message_content = ClaudeMessageContent::ClaudeOutput(parsed_event.clone());
                let message =
                    db::save_new_message(&mut ctx, stack_id.into(), message_content.clone())
                        .unwrap();

                broadcaster.lock().await.send(FrontendEvent {
                    name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
                    payload: json!(message),
                })
            }
        });

        let project_path = project.path.clone();

        let mut command = Command::new("claude");
        command.stdout(writer);
        command.current_dir(&project_path);
        command.args([
            "-p",
            "--output-format=stream-json",
            "--verbose",
            "--dangerously-skip-permissions",
        ]);
        if create_new {
            command.arg(format!("--session-id={stack_id}"));
        } else {
            command.arg(format!("--resume={}", session.current_id));
        }
        command.arg(message);

        let mut handle = command.spawn().unwrap();
        handle.wait().await.unwrap();
        response_streamer.abort();

        self.requests.lock().await.remove(&stack_id);

        Ok(())
    }
}

impl Default for Claudes {
    fn default() -> Self {
        Self::new()
    }
}
