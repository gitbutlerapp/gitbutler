//! After looking into this and seeing false positives in token numbers etc...,
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

use std::{process::Command, sync::Arc};

use anyhow::{Context as _, Result};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use gix::bstr::ByteSlice;
use serde::Deserialize;

use crate::{
    Broadcaster, ClaudeSession, MessagePayload, SystemMessage, Transcript,
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
    pub(crate) fn compact(&self, ctx: &mut Context, broadcaster: &Broadcaster, stack_id: StackId) {
        let res = self.compact_inner(ctx, broadcaster, stack_id);
        self.requests.lock().remove(&stack_id);
        if let Err(res) = res {
            let rule = list_claude_assignment_rules(ctx)
                .ok()
                .and_then(|rules| rules.into_iter().find(|rule| rule.stack_id == stack_id));

            if let Some(rule) = rule {
                let _ = send_claude_message(
                    ctx,
                    broadcaster,
                    rule.session_id,
                    stack_id,
                    MessagePayload::System(crate::SystemMessage::UnhandledException {
                        message: format!("{res}"),
                    }),
                );
            }
        };
    }

    pub fn compact_inner(&self, ctx: &mut Context, broadcaster: &Broadcaster, stack_id: StackId) -> Result<()> {
        let (send_kill, _recv_kill) = flume::unbounded();
        self.requests
            .lock()
            .insert(stack_id, Arc::new(Claude { kill: send_kill }));

        let rule = list_claude_assignment_rules(ctx)?
            .into_iter()
            .find(|rule| rule.stack_id == stack_id);
        let Some(rule) = rule else {
            return Ok(());
        };

        let session = db::get_session_by_id(ctx, rule.session_id)?.context("Failed to find session")?;

        send_claude_message(
            ctx,
            broadcaster,
            rule.session_id,
            stack_id,
            MessagePayload::System(SystemMessage::CompactStart),
        )?;

        let summary = generate_summary(ctx, &session)?;
        send_claude_message(
            ctx,
            broadcaster,
            rule.session_id,
            stack_id,
            MessagePayload::System(SystemMessage::CompactFinished { summary }),
        )?;

        Ok(())
    }

    pub(crate) fn maybe_compact_context(
        &self,
        ctx: &mut Context,
        broadcaster: &Broadcaster,
        stack_id: StackId,
    ) -> Result<()> {
        let messages = {
            let rule = {
                list_claude_assignment_rules(ctx)?
                    .into_iter()
                    .find(|rule| rule.stack_id == stack_id)
            };
            let Some(rule) = rule else {
                return Ok(());
            };

            db::list_messages_by_session(ctx, rule.session_id)?
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
                self.compact(ctx, broadcaster, stack_id);
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

pub fn generate_summary(ctx: &Context, session: &ClaudeSession) -> Result<String> {
    let mut command = {
        let worktree_dir = ctx.workdir_or_fail()?;
        let session_id =
            Transcript::current_valid_session_id(&worktree_dir, session)?.context("Cannot find current session id")?;
        let claude_executable = ctx.settings.claude.executable.clone();

        let mut cmd = Command::new(claude_executable);

        // Don't create a terminal window on windows.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.current_dir(worktree_dir);
        cmd.args(["--resume", &format!("{session_id}")]);
        cmd.arg("-p");
        cmd.arg(SUMMARY_PROMPT);
        cmd
    };

    let output = command.output()?.stdout.to_str_lossy().into_owned();
    Ok(output)
}

const SUMMARY_PROMPT: &str = "
Could you create an in-depth report of the conversation so far.

Please include the following in your report if they are relevant:

- The end goal
- The steps we are taking and the steps we have taken so far
- Key points in the conversation
- Important implementation details
- Key files that are being worked on
- In depth summary of design goals
- Other context that would help another developer continue this work
- The last 3 or 4 of messages from the user. If the messages are long, consider summarising them.

This report should contain enough information for another developer to pick up the conversation where you left off.
";
