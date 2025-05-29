use but_agent::store::ConversationStore as _;
use but_agent::types::{ConversationId, Message};
use but_agent_shared::ConversationStoreAccess as _;
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
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let conversations = ctx
        .conversation_store()
        .read_all()
        .map_err(|e| anyhow::anyhow!("Failed to read conversation store: {:?}", e))?;
    Ok(conversations)
}
