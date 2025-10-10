use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use gitbutler_project::Project;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    pub label: String,
    pub template: String,
}

fn default_template() -> Vec<PromptTemplate> {
    vec![
        PromptTemplate {
            label: "bug-fix".to_string(),
            template: "Please fix the bug in this code:\n\n```\n// Your code here\n```\n\nExpected behavior:\nActual behavior:\nSteps to reproduce:".to_string(),
        },
        PromptTemplate {
            label: "code-review".to_string(),
            template: "Please review this code for:\n- Performance issues\n- Security vulnerabilities\n- Best practices\n- Code style\n\n```\n// Your code here\n```".to_string(),
        },
        PromptTemplate {
            label: "refactor".to_string(),
            template: "Please refactor this code to improve:\n- Readability\n- Performance\n- Maintainability\n\n```\n// Your code here\n```\n\nRequirements:".to_string(),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDir {
    pub label: String,
    pub path: PathBuf,
    // What the paths should end with
    pub filters: Vec<String>,
}

/// Fetch the directories where we look up the user provided templates.
///
/// We want the precidence to be Global < Project < Project Local
///
/// As such, items last in the array take precidence, and filters last in the
/// filters list also take precidence over earlier ones.
///
/// The point of labeling these dirs is so we can also display where to find
/// these directories in the frontend.
pub fn prompt_dirs(project: &Project) -> Result<Vec<PromptDir>> {
    Ok(vec![
        PromptDir {
            label: "Global".into(),
            path: but_path::app_config_dir()?.join("prompt-templates"),
            filters: vec![".md".into()],
        },
        PromptDir {
            label: "Project".into(),
            path: project.path.join(".gitbutler/prompt-templates"),
            filters: vec![".md".into(), ".local.md".into()],
        },
    ])
}

pub fn list_templates(project: &Project) -> Result<Vec<PromptTemplate>> {
    let dirs = prompt_dirs(project)?;
    let mut out = HashMap::new();

    for dir in dirs {
        if dir.path.try_exists()? {
            let entries = dir
                .path
                .read_dir()?
                .filter_map(|entry| entry.ok())
                .collect::<Vec<_>>();
            // We could do the iteration the other way, which would be
            // marginally faster, but this way we get the desired precidence.
            for filter in &dir.filters {
                let longer_filters = dir
                    .filters
                    .iter()
                    .filter(|f| *f != filter && f.len() > filter.len())
                    .collect::<Vec<_>>();
                for entry in &entries {
                    let file_name = entry.file_name();
                    let file_name = file_name.to_string_lossy();
                    let captured_by_longer_filter = !longer_filters.is_empty()
                        && longer_filters.iter().any(|f| file_name.ends_with(*f));
                    if entry.file_type()?.is_file()
                        && let Some(label) = file_name.strip_suffix(filter)
                        && !captured_by_longer_filter
                    {
                        let template = std::fs::read_to_string(entry.path())?;
                        out.insert(
                            label.to_owned(),
                            PromptTemplate {
                                label: label.to_owned(),
                                template,
                            },
                        );
                    }
                }
            }
        } else {
            // Special case the global dir to create defaults
            if dir.label == "Global" {
                for template in write_global_defaults(&dir.path)? {
                    out.insert(template.label.clone(), template);
                }
            }
        }
    }

    let mut out = out.into_values().collect::<Vec<_>>();
    out.sort_by_key(|a| a.label.clone());

    Ok(out)
}

pub fn maybe_create_dir(project: &Project, path: &str) -> Result<()> {
    let path = Path::new(path);
    let path = if path.is_absolute() {
        path
    } else {
        &project.path.join(path)
    };

    if path.try_exists()? {
        return Ok(());
    }

    std::fs::create_dir_all(path)?;

    Ok(())
}

fn write_global_defaults(path: &Path) -> Result<Vec<PromptTemplate>> {
    let defaults = default_template();
    std::fs::create_dir_all(path)?;

    for default in &defaults {
        std::fs::write(
            path.join(format!("{}.md", default.label)),
            default.template.clone(),
        )?;
    }

    Ok(defaults)
}
