use anyhow::Result;
use but_settings::AppSettings;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpPromptParams {
    pub agent: but_acp::AcpCommandConfig,
    pub cwd: Option<std::path::PathBuf>,
    pub prompt: String,
}

pub async fn acp_list_agents(app_settings: AppSettings) -> Result<but_acp::AcpDiscovery> {
    but_acp::discover_agents(app_settings.acp.agents).await
}

pub async fn acp_prompt(params: AcpPromptParams) -> Result<String> {
    let cwd = params.cwd.unwrap_or(std::env::current_dir()?);
    but_acp::prompt_once(params.agent, cwd, params.prompt).await
}
