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
//!
//! Streamed output
//! - It would be curious how this plays into features like queuing multiple
//!   messages.
//!
//! Streamed output and managing tool call output
//! - This might give us more flexabiity in the long run, but initially seems
//!   more complex with more unknowns.

use anyhow::Result;
use but_broadcaster::{Broadcaster, FrontendEvent};
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gix::create;
use serde_json::json;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, PipeWriter, Write},
    sync::Arc,
};
use tokio::{process::Command, sync::Mutex};

use but_workspace::StackId;

use crate::claude_transcript::Transcript;

/// Holds the CC instances. Currently keyed by stackId, since our current model
/// assumes one CC per stack at any given time.
pub struct Claudes {
    processes: HashMap<StackId, Arc<Claude>>,
}

pub struct Claude {
    child: tokio::process::Child,
    manager: tokio::task::JoinHandle<()>,
    message_in: Mutex<PipeWriter>,
}

impl Claudes {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    pub fn send_message(
        &mut self,
        ctx: &CommandContext,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
        message: &str,
    ) -> Result<()> {
        let process = if let Some(process) = self.processes.get(&stack_id) {
            process.clone()
        } else {
            self.spawn_claude(ctx, broadcaster, stack_id)?
        };

        let packet = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": message
                }]
            }
        });

        let formatted_packet = format!("{}\n", serde_json::to_string(&packet)?);

        process
            .message_in
            .try_lock()?
            .write_all(formatted_packet.as_bytes())?;

        Ok(())
    }

    pub fn get_transcript(
        &mut self,
        ctx: &CommandContext,
        stack_id: StackId,
    ) -> Result<Vec<serde_json::Value>> {
        let project = ctx.project();
        let path = Transcript::get_transcript_path(&project.path, stack_id.into())?;
        if path.try_exists()? {
            Transcript::from_file_raw(&path)
        } else {
            Ok(vec![])
        }
    }

    fn spawn_claude(
        &mut self,
        ctx: &CommandContext,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
    ) -> Result<Arc<Claude>> {
        let project = ctx.project();
        // Currently we are using the stack-id _as_ the session-id. In the
        // future we will want these to be detached.

        // We're also making the bold assumption that if we can find the
        // transcript, that a session was created. This is _not_ the best
        // way to do this.
        //
        // https://github.com/anthropics/claude-code/issues/5161 could
        // simplify this
        let transcript_path = Transcript::get_transcript_path(&project.path, stack_id.into())?;

        let create_new = !transcript_path.try_exists()?;

        let (reader, write_stdin) = std::io::pipe()?;
        let (read_stdout, writer) = std::io::pipe()?;

        let broadcaster = broadcaster.clone();

        let project_id = project.id;
        let manager = tokio::spawn(async move {
            let reader = BufReader::new(read_stdout);
            for line in reader.lines() {
                let line = line.unwrap();
                let parsed_event = serde_json::from_str(&line).unwrap();
                broadcaster.lock().await.send(FrontendEvent {
                    name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
                    payload: parsed_event,
                })
            }
        });

        let mut command = Command::new("claude");
        command.stdin(reader);
        command.stdout(writer);
        command.current_dir(&project.path);
        command.args([
            "-p",
            "--output-format=stream-json",
            "--input-format=stream-json",
            "--verbose",
            "--dangerously-skip-permissions",
        ]);
        if create_new {
            command.arg(format!("--session-id={stack_id}"));
        } else {
            command.arg(format!("--resume={stack_id}"));
        }

        let claude = Arc::new(Claude {
            child: command.spawn()?,
            manager,
            message_in: Mutex::new(write_stdin),
        });

        self.processes.insert(stack_id, claude.clone());

        Ok(claude)
    }
}

impl Default for Claudes {
    fn default() -> Self {
        Self::new()
    }
}
