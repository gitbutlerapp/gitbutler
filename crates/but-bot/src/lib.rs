use but_ctx::Context;
use but_llm::OpenAiProvider;
use but_tools::emit::{Emittable, Emitter};
use gitbutler_project::ProjectId;

mod butbot;
mod state;

use crate::{agent::AgentGraph, butbot::ButBot};

pub mod agent;

pub fn bot(
    project_id: ProjectId,
    message_id: String,
    emitter: std::sync::Arc<Emitter>,
    ctx: &mut Context,
    openai: &OpenAiProvider,
    chat_messages: Vec<but_llm::ChatMessage>,
) -> anyhow::Result<String> {
    let mut but_bot = ButBot::new(ctx, emitter, message_id, project_id, openai, chat_messages);
    let mut graph = AgentGraph::default();
    graph.start(&mut but_bot)
}

#[allow(clippy::too_many_arguments)]
pub fn forge_branch_chat(
    project_id: ProjectId,
    branch: String,
    message_id: String,
    emitter: std::sync::Arc<Emitter>,
    openai: &OpenAiProvider,
    chat_messages: Vec<but_llm::ChatMessage>,
    reviews: Vec<but_forge::ForgeReview>,
) -> anyhow::Result<String> {
    let reviews_text = reviews
        .iter()
        .map(|r| format!("{}", r))
        .collect::<Vec<String>>()
        .join("\n\n");

    let current_time = chrono::Utc::now().to_rfc3339();
    let sys_prompt = format!(
        "<tone>
    You are an AI assistant specialized in explaining code changes made in the git branch '{branch}'.
</tone>

<current_time>
    {current_time}
</current_time>

<task>
    Answer the user's questions about the code changes made in the branch '{branch}'.
    Base your responses solely on the given code reviews associated with this branch.
</task>

<given_reviews>
{reviews_text}
</given_reviews>
");

    let message_id_cloned = message_id.clone();
    let project_id_cloned = project_id;
    let on_token_cb: std::boxed::Box<dyn Fn(&str) + Send + Sync + 'static> =
        std::boxed::Box::new({
            let emitter = emitter.clone();
            let message_id = message_id_cloned;
            let project_id = project_id_cloned;
            move |token: &str| {
                let token_update = but_tools::emit::TokenUpdate {
                    token: token.to_string(),
                    project_id,
                    message_id: message_id.clone(),
                };
                let (name, payload) = token_update.emittable();
                (emitter)(&name, payload);
            }
        });

    let response =
        but_llm::stream_response_blocking(openai, &sys_prompt, chat_messages, None, on_token_cb)?
            .unwrap_or_default();

    Ok(response)
}
