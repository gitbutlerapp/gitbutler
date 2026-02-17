use but_ctx::Context;
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
    llm: &but_llm::LLMProvider,
    chat_messages: Vec<but_llm::ChatMessage>,
) -> anyhow::Result<String> {
    let mut but_bot = ButBot::new(ctx, emitter, message_id, project_id, llm, chat_messages);
    let mut graph = AgentGraph::default();
    graph.start(&mut but_bot)
}

#[allow(clippy::too_many_arguments)]
pub fn forge_branch_chat(
    project_id: ProjectId,
    branch: String,
    message_id: String,
    emitter: std::sync::Arc<Emitter>,
    llm: &but_llm::LLMProvider,
    model: String,
    chat_messages: Vec<but_llm::ChatMessage>,
    reviews: Vec<but_forge::ForgeReview>,
) -> anyhow::Result<String> {
    let reviews_text = reviews
        .iter()
        .map(|r| format!("{r}"))
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
"
    );

    let message_id_cloned = message_id.clone();
    let project_id_cloned = project_id;

    let response = llm
        .stream_response(&sys_prompt, chat_messages, &model, {
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
        })?
        .unwrap_or_default();

    Ok(response)
}
