use std::sync::Arc;

use schemars::{JsonSchema, schema_for};

use crate::tool::Tool;

#[derive(Debug, Clone, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
struct MyToolInput {
    message: String,
}

pub struct MyTool;

impl Tool for MyTool {
    type Output = String;

    fn name(&self) -> String {
        "MyTool".to_string()
    }

    fn description(&self) -> String {
        "This is a custom tool for demonstration purposes.".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(MyToolInput);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(self: Arc<Self>, parameters: serde_json::Value) -> anyhow::Result<Self::Output> {
        let input: MyToolInput = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;
        Ok(input.message)
    }
}

pub struct BranchDetails;

#[derive(Debug, Clone, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
struct BranchDetailsInput {
    branch_name: String,
}

impl Tool for BranchDetails {
    type Output = String;

    fn name(&self) -> String {
        "BranchDetails".to_string()
    }

    fn description(&self) -> String {
        "Provides an overview of a particular branch. This includes a list of the commits that belong to the branch, each containing the list of files that are part of the commit".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let schema = schema_for!(BranchDetailsInput);
        serde_json::to_value(&schema).unwrap_or_default()
    }

    fn call(self: Arc<Self>, parameters: serde_json::Value) -> anyhow::Result<Self::Output> {
        let input: BranchDetailsInput = serde_json::from_value(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse input parameters: {}", e))?;
        
        // Here you would implement the logic to gather branch information.
        Ok("Branch overview is not implemented yet.".to_string())
    }
}
