use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    pub label: String,
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplates {
    pub templates: Vec<PromptTemplate>,
}

impl Default for PromptTemplates {
    fn default() -> Self {
        Self {
            templates: vec![
                PromptTemplate {
                    label: "Bug Fix".to_string(),
                    template: "Please fix the bug in this code:\n\n```\n// Your code here\n```\n\nExpected behavior:\nActual behavior:\nSteps to reproduce:".to_string(),
                },
                PromptTemplate {
                    label: "Code Review".to_string(),
                    template: "Please review this code for:\n- Performance issues\n- Security vulnerabilities\n- Best practices\n- Code style\n\n```\n// Your code here\n```".to_string(),
                },
                PromptTemplate {
                    label: "Refactor".to_string(),
                    template: "Please refactor this code to improve:\n- Readability\n- Performance\n- Maintainability\n\n```\n// Your code here\n```\n\nRequirements:".to_string(),
                },
                PromptTemplate {
                    label: "Add Tests".to_string(),
                    template: "Please write comprehensive tests for this code:\n\n```\n// Your code here\n```\n\nTest cases should cover:\n- Happy path\n- Edge cases\n- Error conditions".to_string(),
                },
            ],
        }
    }
}

const PROMPT_TEMPLATES_FILE: &str = "prompt-templates.json";

fn get_prompt_templates_path() -> Result<std::path::PathBuf> {
    let config_dir = but_path::app_config_dir()?;
    Ok(config_dir.join(PROMPT_TEMPLATES_FILE))
}

pub fn load_prompt_templates() -> Result<PromptTemplates> {
    let templates_path = get_prompt_templates_path()?;

    if !templates_path.exists() {
        let default_templates = PromptTemplates::default();
        save_prompt_templates_to_path(&templates_path, &default_templates)?;
        return Ok(default_templates);
    }

    let content = std::fs::read_to_string(&templates_path)?;
    let templates: PromptTemplates = serde_json::from_str(&content)?;

    Ok(templates)
}

fn save_prompt_templates_to_path(path: &Path, templates: &PromptTemplates) -> Result<()> {
    let config_dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid config path"))?;

    std::fs::create_dir_all(config_dir)?;

    let content = serde_json::to_string_pretty(templates)?;
    std::fs::write(path, content)?;

    Ok(())
}

pub fn write_prompt_templates(templates: &PromptTemplates) -> Result<()> {
    let templates_path = get_prompt_templates_path()?;
    save_prompt_templates_to_path(&templates_path, templates)
}

pub fn get_prompt_templates_path_string() -> Result<String> {
    let templates_path = get_prompt_templates_path()?;

    // Ensure file exists (create with defaults if not)
    if !templates_path.exists() {
        let default_templates = PromptTemplates::default();
        save_prompt_templates_to_path(&templates_path, &default_templates)?;
    }

    Ok(templates_path.to_string_lossy().to_string())
}
