use but_action::OpenAiProvider;
use but_tools::emit::Emitter;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;

mod state;

use crate::agent::{Agent, ButBot};

pub mod agent;

pub fn bot(
    project_id: ProjectId,
    message_id: String,
    emitter: std::sync::Arc<Emitter>,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    chat_messages: Vec<but_action::ChatMessage>,
) -> anyhow::Result<String> {
    let mut but_bot = ButBot::new(ctx, emitter, message_id, project_id, openai);
    but_bot.evaluate(chat_messages)
}
