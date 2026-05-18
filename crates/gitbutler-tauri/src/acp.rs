use but_api::{json::Error, legacy::acp};
use but_settings::AppSettingsWithDiskSync;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(app_settings_sync), err(Debug))]
pub async fn acp_list_agents(
    app_settings_sync: State<'_, AppSettingsWithDiskSync>,
) -> Result<but_acp::AcpDiscovery, Error> {
    let settings = { app_settings_sync.get()?.clone() };
    acp::acp_list_agents(settings).await.map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn acp_prompt(
    agent: but_acp::AcpCommandConfig,
    cwd: Option<std::path::PathBuf>,
    prompt: String,
) -> Result<String, Error> {
    let params = acp::AcpPromptParams { agent, cwd, prompt };
    acp::acp_prompt(params).await.map_err(Into::into)
}
