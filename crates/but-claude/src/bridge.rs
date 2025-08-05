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

use anyhow::{Result, anyhow};
use but_broadcaster::{Broadcaster, FrontendEvent};
use gitbutler_project::ProjectId;
use serde_json::json;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, PipeWriter, Write},
    sync::Arc,
};
use tokio::{process::Command, sync::Mutex};

use but_workspace::StackId;

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
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        project_id: ProjectId,
        stack_id: StackId,
        message: &str,
    ) -> Result<()> {
        let process = if let Some(process) = self.processes.get(&stack_id) {
            process.clone()
        } else {
            let (reader, write_stdin) = std::io::pipe()?;
            let (read_stdout, writer) = std::io::pipe()?;

            let broadcaster = broadcaster.clone();

            let manager = tokio::spawn(async move {
                let reader = BufReader::new(read_stdout);
                for line in reader.lines() {
                    let line = line.unwrap();
                    let parsed_event = serde_json::from_str(&line).unwrap();
                    broadcaster.lock().await.send(FrontendEvent {
                        name: format!(
                            "project://{}/claude/{}/message_recieved",
                            project_id, stack_id
                        ),
                        payload: parsed_event,
                    })
                }
            });

            let claude = Arc::new(Claude {
                child: Command::new("claude")
                    .args([
                        "-p",
                        "--output-format=stream-json",
                        "--input-format=stream-json",
                        "--verbose",
                    ])
                    .stdin(reader)
                    .stdout(writer)
                    .spawn()?,
                manager,
                message_in: Mutex::new(write_stdin),
            });

            self.processes.insert(stack_id, claude.clone());
            claude
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

        // The lock err type is a bit of a prima donna and can't be wraped in
        // anyhow, so we need to format it.
        process
            .message_in
            .try_lock()
            .map_err(|e| anyhow!("Failed to get lock: {}", e))?
            .write_all(formatted_packet.as_bytes())?;

        Ok(())
    }
}

impl Default for Claudes {
    fn default() -> Self {
        Self::new()
    }
}
