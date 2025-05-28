use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool,
};

pub(crate) async fn start() -> Result<()> {
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = Mcp::new().serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Mcp;

#[tool(tool_box)]
impl Mcp {
    pub fn new() -> Self {
        Self
    }
    #[tool(description = "Handle the changes that are currently uncommitted for the repository.")]
    pub fn handle_changes(&self, #[tool(aggr)] request: HandleChangesRequest) -> String {
        todo!("Handle changes request: {}", request.context);
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HandleChangesRequest {
    #[schemars(
        description = "Information about what has changed and why - i.e. the user prompt etc."
    )]
    pub context: String,
}

#[tool(tool_box)]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("GitButler MCP server".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "GitButler MCP Server".into(),
                version: "1.0.0".into(),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
