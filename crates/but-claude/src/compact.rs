//! After looking into this and seeing false positives in token numbers ect...,
//! I've determined that it is currently just not possible to use the real
//! /compact command from CC.
//!
//! Given that other tools like op-code and the Zed ACP integration don't
//! support the compact command, and some GH issues on the matter, it seems safe
//! to assume that it just does not work in it's current form.
//!
//! As such, we need to implement our own behaviour of this functionality.
//!
//! A reasonable implementation is to ask the LLM to give an in depth overview
//! of the conversation so far and then start a new session where the first
//! message contains the summary

use std::sync::Arc;

use anyhow::{Context, Result};
use but_broadcaster::Broadcaster;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gix::bstr::ByteSlice;
use serde::Deserialize;
use tokio::{
    process::Command,
    sync::{Mutex, mpsc::unbounded_channel},
};

use crate::{
    ClaudeSession, MessagePayload, SystemMessage, Transcript,
    bridge::{Claude, Claudes},
    db,
    rules::list_claude_assignment_rules,
    send_claude_message,
};

#[derive(Deserialize, Debug, Clone)]
struct ModelUsage {
    input_tokens: u32,
    output_tokens: u32,
    cache_read_input_tokens: Option<u32>,
    cache_creation_input_tokens: Option<u32>,
}

#[derive(Debug)]
struct Model<'a> {
    name: &'a str,
    subtype: Option<&'a str>,
    context: u32,
}

const COMPACTION_BUFFER: u32 = 15_000;

const MODELS: &[Model<'static>] = &[
    Model {
        name: "opus",
        subtype: None,
        context: 200_000,
    },
    // Ordering the 1m model before the 200k model so it matches first.
    Model {
        name: "sonnet",
        subtype: Some("[1m]"),
        context: 1_000_000,
    },
    Model {
        name: "sonnet",
        subtype: None,
        context: 200_000,
    },
    Model {
        name: "haiku",
        subtype: None,
        context: 200_000,
    },
];

impl Claudes {
    pub(crate) async fn compact(
        &self,
        ctx: Arc<Mutex<CommandContext>>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
    ) -> () {
        let res = self
            .compact_inner(ctx.clone(), broadcaster.clone(), stack_id)
            .await;
        self.requests.lock().await.remove(&stack_id);
        if let Err(res) = res {
            let mut ctx = ctx.lock().await;

            let rule = list_claude_assignment_rules(&mut ctx)
                .ok()
                .and_then(|rules| rules.into_iter().find(|rule| rule.stack_id == stack_id));

            if let Some(rule) = rule {
                let _ = send_claude_message(
                    &mut ctx,
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

    pub async fn compact_inner(
        &self,
        ctx: Arc<Mutex<CommandContext>>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
    ) -> Result<()> {
        let (send_kill, mut _recv_kill) = unbounded_channel();
        self.requests
            .lock()
            .await
            .insert(stack_id, Arc::new(Claude { kill: send_kill }));

        let rule = {
            let mut ctx = ctx.lock().await;
            list_claude_assignment_rules(&mut ctx)?
                .into_iter()
                .find(|rule| rule.stack_id == stack_id)
        };
        let Some(rule) = rule else {
            return Ok(());
        };

        let session = {
            let mut ctx = ctx.lock().await;
            db::get_session_by_id(&mut ctx, rule.session_id)?.context("Failed to find session")?
        };

        {
            let mut ctx = ctx.lock().await;
            send_claude_message(
                &mut ctx,
                broadcaster.clone(),
                rule.session_id,
                stack_id,
                MessagePayload::System(SystemMessage::CompactStart),
            )
            .await?;
        }
        let summary = generate_summary(ctx.clone(), &session).await?;
        {
            let mut ctx = ctx.lock().await;
            send_claude_message(
                &mut ctx,
                broadcaster.clone(),
                rule.session_id,
                stack_id,
                MessagePayload::System(SystemMessage::CompactFinished { summary }),
            )
            .await?;
        }

        Ok(())
    }

    pub(crate) async fn maybe_compact_context(
        &self,
        ctx: Arc<Mutex<CommandContext>>,
        broadcaster: Arc<tokio::sync::Mutex<Broadcaster>>,
        stack_id: StackId,
    ) -> Result<()> {
        let rule = {
            let mut ctx = ctx.lock().await;
            list_claude_assignment_rules(&mut ctx)?
                .into_iter()
                .find(|rule| rule.stack_id == stack_id)
        };
        let Some(rule) = rule else {
            return Ok(());
        };

        let messages = {
            let mut ctx = ctx.lock().await;
            db::list_messages_by_session(&mut ctx, rule.session_id)?
        };

        // Find the last result message
        let Some(output) = messages.into_iter().rev().find_map(|m| match m.payload {
            MessagePayload::Claude(o) => {
                if o.data["type"].as_str() == Some("assistant") {
                    Some(o.data)
                } else {
                    None
                }
            }
            _ => None,
        }) else {
            return Ok(());
        };

        let model_name = output["message"]["model"]
            .as_str()
            .context("could not find model property")?;

        if let Some(model) = find_model(model_name.to_owned()) {
            let usage: ModelUsage = serde_json::from_value(output["message"]["usage"].clone())?;

            let total = usage.cache_read_input_tokens.unwrap_or(0)
                + usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.input_tokens
                + usage.output_tokens;
            if total > (model.context - COMPACTION_BUFFER) {
                self.compact(ctx.clone(), broadcaster.clone(), stack_id)
                    .await;
            }
        };

        Ok(())
    }
}

fn find_model(name: String) -> Option<&'static Model<'static>> {
    MODELS
        .iter()
        .find(|&m| name.contains(m.name) && m.subtype.map(|s| name.contains(s)).unwrap_or(true))
}

pub async fn generate_summary(
    ctx: Arc<Mutex<CommandContext>>,
    session: &ClaudeSession,
) -> Result<String> {
    let app_settings = ctx.lock().await.app_settings().clone();
    let claude_executable = app_settings.claude.executable.clone();
    let session_id =
        Transcript::current_valid_session_id(ctx.lock().await.project().worktree_dir()?, session)
            .await?
            .context("Cant find current session id")?;

    let mut command = Command::new(claude_executable);

    /// Don't create a terminal window on windows.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.current_dir(ctx.lock().await.project().worktree_dir()?);
    command.args(["--resume", &format!("{session_id}")]);
    command.arg("-p");
    command.arg(SUMMARY_PROMPT);

    let output = command.output().await?.stdout.to_str_lossy().into_owned();
    Ok(output)
}

const SUMMARY_PROMPT: &str = "
Could you create an in-depth report of the conversation so far.

Please include the following in your report if they are relevant:

- The end goal
- The steps we are taking and the steps we have taken so far
- Key points in the conversation
- Important implmentation details
- Key files that are being worked on
- In depth summary of design goals
- Other context that would help another developer continue this work
- The last 3 or 4 of messages from the user. If the messages are long, consider summarising them.

This report should contain enough information for another developer to pick up the conversation where you left off.
";
