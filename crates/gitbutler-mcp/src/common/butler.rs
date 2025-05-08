use rmcp::{
    const_string, model::*, schemars, service::RequestContext, tool, Error as McpError, RoleServer,
    ServerHandler,
};
use serde_json::json;
use std::path::PathBuf;
use tracing;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateBranchRequest {
    pub working_directory: String,
    pub full_prompt: String,
    pub summary: String,
}

#[derive(Clone)]
pub struct Butler {}

#[tool(tool_box)]
impl Butler {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {}
    }

    fn _create_resource_text(&self, uri: &str, name: &str) -> Resource {
        RawResource::new(uri, name.to_string()).no_annotation()
    }

    #[tool(description = "Update a branch with the given prompt and summary")]
    fn update_branch(
        &self,
        #[tool(aggr)] UpdateBranchRequest {
            working_directory,
            full_prompt,
            summary,
        }: UpdateBranchRequest,
    ) -> Result<CallToolResult, McpError> {
        tracing::info!("Updating branch with prompt: {}", summary);

        // Check if the working directory exists
        let project_path = PathBuf::from(&working_directory);
        if !project_path.exists() {
            return Err(McpError::invalid_params(
                "Invalid working directory",
                Some(json!({ "error": "Working directory does not exist" })),
            ));
        }

        // In a real implementation, we would use GitButler's branch management APIs
        // But for now, we'll simulate a successful branch update
        tracing::info!(
            "Would update branch in {} using prompt: {} with summary: {}",
            working_directory,
            full_prompt,
            summary
        );

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Branch has been updated with summary: {}",
            summary
        ))]))
    }
}

const_string!(UpdateBranch = "updateBranch");

#[tool(tool_box)]
impl ServerHandler for Butler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides a branch update tool that can process prompts and update branches accordingly.".to_string()),
        }
    }

    async fn list_resources(
        &self,
        _request: std::option::Option<rmcp::model::PaginatedRequestParamInner>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        Err(McpError::resource_not_found(
            "resource_not_found",
            Some(json!({
                "uri": uri
            })),
        ))
    }

    async fn list_prompts(
        &self,
        _request: std::option::Option<rmcp::model::PaginatedRequestParamInner>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            next_cursor: None,
            prompts: vec![],
        })
    }

    async fn get_prompt(
        &self,
        GetPromptRequestParam {
            name: _name,
            arguments: _arguments,
        }: GetPromptRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        Err(McpError::invalid_params("prompt not found", None))
    }

    async fn list_resource_templates(
        &self,
        _request: std::option::Option<rmcp::model::PaginatedRequestParamInner>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            next_cursor: None,
            resource_templates: Vec::new(),
        })
    }
}
