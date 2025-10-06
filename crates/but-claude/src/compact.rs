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
use tokio::{
    process::Command,
    sync::{Mutex, mpsc::unbounded_channel},
};

use crate::{
    ClaudeMessageContent, ClaudeSession, GitButlerMessage, Transcript,
    bridge::{Claude, Claudes},
    db,
    rules::list_claude_assignment_rules,
    send_claude_message,
};

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
                ClaudeMessageContent::GitButlerMessage(GitButlerMessage::CompactStart),
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
                ClaudeMessageContent::GitButlerMessage(GitButlerMessage::CompactFinished {
                    summary,
                }),
            )
            .await?;
        }

        Ok(())
    }
}

pub async fn generate_summary(
    ctx: Arc<Mutex<CommandContext>>,
    session: &ClaudeSession,
) -> Result<String> {
    let app_settings = ctx.lock().await.app_settings().clone();
    let claude_executable = app_settings.claude.executable.clone();
    let session_id =
        Transcript::current_valid_session_id(&ctx.lock().await.project().path, session)
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

    command.current_dir(&ctx.lock().await.project().path);
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
