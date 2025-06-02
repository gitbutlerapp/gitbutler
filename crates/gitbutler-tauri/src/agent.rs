use std::sync::{Arc, Mutex};

use but_agent::agent::{agent_perform, AgentConfig};
use but_agent::openai_like_llm::OpenAILikeLLM;
use but_agent::store::ConversationStore as _;
use but_agent::types::{Action, ConversationId, Message};
use but_db::conversation_store::ConversationStoreAccess as _;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn agent_list_all_conversations(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<std::collections::HashMap<ConversationId, Vec<Message>>, Error> {
    let project = projects.get(project_id)?;
    let mut ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let conversations = ctx
        .db()?
        .conversation_store()
        .read_all()
        .map_err(|e| anyhow::anyhow!("Failed to read conversation store: {:?}", e))?;
    Ok(conversations)
}

// How to deal with agent configurations.
// Fuck it, let's do this later.

#[tauri::command(async)]
#[instrument(skip(token), err(Debug))]
pub fn agent_set_open_router_token(token: Option<&str>) -> Result<(), Error> {
    set_token(token);
    Ok(())
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn agent_is_open_router_token_set() -> Result<bool, Error> {
    let token = get_token();
    Ok(token.is_some())
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn agent_create_conversation(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<ConversationId, Error> {
    let project = projects.get(project_id)?;
    let mut ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let db = ctx.db().unwrap();
    let mut conversation_store = db.conversation_store();
    let conversation_id: Arc<Mutex<Option<ConversationId>>> = Arc::new(Mutex::new(None));
    let move_conversation_id = conversation_id.clone();
    let mut config = AgentConfig {
        llm: Box::new(OpenAILikeLLM {
            completion_url: "https://openrouter.ai/api/v1/chat/completions".into(),
            model: "deepseek/deepseek-r1-distill-llama-70b".into(),
            provider: Some("cerebras".into()),
            token: Some(get_token().unwrap().0),
        }),
        conversation_store: &mut conversation_store,
        callback: move |thing| {
            let mut conversation_id = move_conversation_id.lock().unwrap();
            *conversation_id = Some(thing.id());
        },
        system_prompt: "Help me with my code".into(),
        tools: Vec::new(),
    };
    agent_perform(&mut config, Action::StartNewThread);
    let conversation_id = conversation_id.lock().unwrap().unwrap();
    Ok(conversation_id)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub async fn agent_send_message(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    conversation_id: ConversationId,
    message: String,
) -> Result<ConversationId, Error> {
    let project = projects.get(project_id)?;
    let mut ctx = CommandContext::open(&project, settings.get().unwrap().clone()).unwrap();

    let handle = tokio::task::spawn_blocking(move || {
        let db = ctx.db().unwrap();
        let mut conversation_store = db.conversation_store();
        let mut config = AgentConfig {
            llm: Box::new(OpenAILikeLLM {
                completion_url: "https://openrouter.ai/api/v1/chat/completions".into(),
                model: "deepseek/deepseek-r1-distill-llama-70b".into(),
                provider: Some("cerebras".into()),
                token: Some(get_token().unwrap().0),
            }),
            conversation_store: &mut conversation_store,
            callback: |_| {},
            system_prompt: "Help me with my code".into(),
            tools: Vec::new(),
        };

        agent_perform(
            &mut config,
            Action::SendMessage {
                id: conversation_id,
                message,
            },
        );
    });

    handle.await.unwrap();

    Ok(conversation_id)
}

// MockLLM {
//     callback: |params| LLMResponse::Message {
//         message: format!(
//             "Oh, hi there!\n You said: {}",
//             params.messages.last().unwrap().content
//         ),
//     },
// }

fn get_token() -> Option<gitbutler_secret::Sensitive<String>> {
    // return Some(gitbutler_secret::Sensitive("this is secret".into()));
    gitbutler_secret::secret::retrieve(
        "gitbutler-agent-open-router-token",
        gitbutler_secret::secret::Namespace::Global,
    )
    .unwrap()
}

fn set_token(token: Option<&str>) {
    if let Some(token) = token {
        gitbutler_secret::secret::persist(
            "gitbutler-agent-open-router-token",
            &gitbutler_secret::Sensitive(token.into()),
            gitbutler_secret::secret::Namespace::Global,
        )
        .unwrap();
    } else {
        gitbutler_secret::secret::delete(
            "gitbutler-agent-open-router-token",
            gitbutler_secret::secret::Namespace::Global,
        )
        .unwrap();
    }
}
