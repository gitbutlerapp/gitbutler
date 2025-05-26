use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool,
};

pub(crate) async fn start() -> Result<()> {
    let transport = (tokio::io::stdin(), tokio::io::stdout());

    let service = PublicMcp::new().serve(transport).await?;

    service.waiting().await?;
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
            server_info: Implementation {
                name: "GitButler MCP Server".into(),
                version: "1.0.0".into(),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
