use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool,
};
use tokio::io::{stdin, stdout};

pub(crate) async fn start() -> Result<()> {
    let transport = (stdin(), stdout());
    let service = PublicMcp::new();
    service.serve(transport).await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PublicMcp;

#[tool(tool_box)]
impl PublicMcp {
    pub fn new() -> Self {
        Self
    }

    #[tool(description = "Greetings from the MCP server")]
    pub fn hello(&self, #[tool(aggr)] GreetRequest { name }: GreetRequest) -> String {
        format!("Hello, {}!", name)
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GreetRequest {
    #[schemars(description = "Request for a greeting")]
    pub name: String,
}

#[tool(tool_box)]
impl ServerHandler for PublicMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("GitButler MCP server".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
