use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgent {
    name: String,
    description: String,
    tools: Option<Vec<String>>,
    model: Option<String>,
}

pub fn read_claude_sub_agents(project_path: &Path) -> Vec<SubAgent> {
    let mut out = vec![];

    if let Some(home_dir) = dirs::home_dir().map(|p| p.join(".claude/agents"))
        && let Some(agents) = read_agents(&home_dir)
    {
        for agent in agents {
            out.push(agent);
        }
    }

    if let Some(agents) = read_agents(&project_path.join(".claude/agents")) {
        for agent in agents {
            out.push(agent);
        }
    }

    out
}

fn read_agents(path: &Path) -> Option<Vec<SubAgent>> {
    if !path.try_exists().unwrap_or(false) {
        return None;
    }

    let mut out = vec![];

    let entries = fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        if let Some(agent) = read_entry(&entry) {
            out.push(agent)
        }
    }

    Some(out)
}

fn read_entry(entry: &fs::DirEntry) -> Option<SubAgent> {
    let entry_type = entry.file_type().ok()?;
    if !entry_type.is_file() {
        return None;
    };
    let string = fs::read_to_string(entry.path()).ok()?;
    let mut agent = SubAgent {
        name: "".into(),
        description: "".into(),
        tools: None,
        model: None,
    };
    let mut in_frontmatter = false;
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
                "tools" => agent.tools = Some(value.split(", ").map(Into::into).collect::<Vec<_>>()),
                "model" => agent.model = Some(value.into()),
                _ => {}
            }
        }
    }
    Some(agent)
}
