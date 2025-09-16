use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::fs::{self, DirEntry};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgent {
    name: String,
    description: String,
    tools: Option<Vec<String>>,
    model: Option<String>,
}

pub async fn read_claude_sub_agents(project_path: &Path) -> Vec<SubAgent> {
    let mut out = vec![];

    if let Some(home_dir) = dirs::home_dir().map(|p| p.join(".claude/agents"))
        && let Some(agents) = read_agents(&home_dir).await
    {
        for agent in agents {
            out.push(agent);
        }
    }

    if let Some(agents) = read_agents(&project_path.join(".claude/agents")).await {
        for agent in agents {
            out.push(agent);
        }
    }

    out
}

async fn read_agents(path: &Path) -> Option<Vec<SubAgent>> {
    if !fs::try_exists(path).await.unwrap_or(false) {
        return None;
    }

    let mut out = vec![];

    let mut entries = fs::read_dir(path).await.ok()?;
    loop {
        match entries.next_entry().await {
            Ok(Some(entry)) => {
                if let Some(agent) = read_entry(entry).await {
                    out.push(agent)
                }
            }
            Ok(None) => break,
            Err(_) => continue,
        }
    }

    Some(out)
}

async fn read_entry(entry: DirEntry) -> Option<SubAgent> {
    let entry_type = entry.file_type().await.ok()?;
    if !entry_type.is_file() {
        return None;
    };
    let string = fs::read_to_string(entry.path()).await.ok()?;
    let mut in_frontmatter = false;
    let mut agent = SubAgent {
        name: "".into(),
        description: "".into(),
        tools: None,
        model: None,
    };
    for line in string.lines() {
        if !in_frontmatter && line == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter && line == "---" {
            break;
        }
        // Parse the NOT YAML
        if in_frontmatter && let Some((key, value)) = line.split_once(": ") {
            match key {
                "name" => agent.name = value.into(),
                "description" => agent.description = value.into(),
                "tools" => {
                    agent.tools = Some(value.split(", ").map(Into::into).collect::<Vec<_>>())
                }
                "model" => agent.model = Some(value.into()),
                _ => {}
            }
        }
    }
    Some(agent)
}
