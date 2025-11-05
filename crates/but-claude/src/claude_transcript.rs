use std::{
    collections::HashMap,
    io::BufRead,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::ClaudeSession;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserMessage {
    pub role: Option<String>,
    pub content: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub role: Option<String>,
    pub model: Option<String>,
    pub content: Option<serde_json::Value>,
    pub stop_reason: Option<serde_json::Value>,
    pub stop_sequence: Option<serde_json::Value>,
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum Record {
    #[serde(rename = "summary")]
    Summary {
        summary: String,
        #[serde(rename = "leafUuid")]
        leaf_uuid: String,
    },
    #[serde(rename = "user")]
    User {
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        #[serde(rename = "isSidechain")]
        is_sidechain: Option<bool>,
        #[serde(rename = "userType")]
        user_type: Option<String>,
        cwd: Option<String>,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        version: Option<String>,
        message: Option<UserMessage>,
        uuid: Option<String>,
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: HashMap<String, serde_json::Value>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        #[serde(rename = "isSidechain")]
        is_sidechain: Option<bool>,
        #[serde(rename = "userType")]
        user_type: Option<String>,
        cwd: Option<String>,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        version: Option<String>,
        message: Option<AssistantMessage>,
        #[serde(rename = "requestId")]
        request_id: Option<String>,
        uuid: Option<String>,
        timestamp: Option<String>,
        #[serde(rename = "toolUseResult")]
        tool_use_result: Option<serde_json::Value>,
        #[serde(flatten)]
        extra: HashMap<String, serde_json::Value>,
    },
    #[serde(other)]
    Other,
}

pub struct Transcript {
    records: Vec<Record>,
}

impl Transcript {
    pub async fn current_valid_session_id(
        path: &Path,
        session: &ClaudeSession,
    ) -> Result<Option<uuid::Uuid>> {
        let mut session_ids = session.session_ids.clone();
        let mut current_id = None;

        loop {
            if session_ids.is_empty() {
                break;
            }

            let next_id = session_ids.pop();
            if let Some(next_id) = next_id
                && Self::transcript_exists_and_likely_valid(path, next_id).await?
            {
                current_id = Some(next_id);
                break;
            }
        }

        Ok(current_id)
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let records = Transcript::from_file_raw(path)?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<_, _>>()?;
        Ok(Transcript { records })
    }

    pub fn from_file_raw(path: &Path) -> Result<Vec<serde_json::Value>> {
        let file =
            std::fs::File::open(path).map_err(|e| anyhow::anyhow!("Failed to open file: {}", e))?;
        let reader = std::io::BufReader::new(file);
        let mut records = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|e| anyhow::anyhow!("Failed to read line: {}", e))?;
            if !line.trim().is_empty() {
                let record: serde_json::Value = serde_json::from_str(&line)
                    .map_err(|e| anyhow::anyhow!("Failed to parse JSON line: {}", e))?;
                records.push(record);
            }
        }
        Ok(records)
    }

    /// A little bit of magic to try and find the transcript path
    /// Users _might_ customize their config directory and CWD though
    /// environment variables, but... this is for the prototype. I've tweeted at
    /// anthropic asking for a command we can run to obtain this.
    ///
    /// By inspecting the folder structure I've determined that the transcripts
    /// live at:
    /// $HOME/.claude/projects/{formattedProjectCwd}/{sessionId}.jsonl
    ///
    /// Where formattedProjectCwd is the absolute path to the folder, with any
    /// non-alphanumeric characters removed. and where the session is the uuid
    /// printed out in hex.
    ///
    /// https://github.com/anthropics/claude-code/issues/5165 could remove any
    /// need for this speculation.
    pub fn get_transcript_path(project_cwd: &Path, session_id: uuid::Uuid) -> Result<PathBuf> {
        let formatted_cwd = project_cwd
            .display()
            .to_string()
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' => c,
                _ => '-',
            })
            .collect::<String>();

        let file_name = format!("{session_id}.jsonl");

        let home_dir = dirs::home_dir().context("Failed to find home dir")?;

        Ok(home_dir
            .join(".claude/projects")
            .join(formatted_cwd)
            .join(file_name))
    }

    pub fn dir(&self) -> Result<String> {
        for record in self.records.iter() {
            if let Record::User { cwd: Some(cwd), .. } = record {
                return Ok(cwd.to_string());
            }
        }
        Err(anyhow::anyhow!("No user record with cwd found"))
    }

    pub fn summary(&self) -> Option<String> {
        for record in self.records.iter().rev() {
            if let Record::Summary { summary, .. } = record {
                return Some(summary.to_string());
            }
        }
        None
    }

    pub fn prompt(&self) -> Option<String> {
        for record in self.records.iter().rev() {
            if let Record::User {
                message: Some(msg), ..
            } = record
                && let Some(content) = &msg.content
                && let Some(text) = content.as_str()
            {
                return Some(text.to_string());
            }
        }
        None
    }

    async fn transcript_exists_and_likely_valid(
        project_path: &Path,
        session_id: uuid::Uuid,
    ) -> Result<bool> {
        let path = Self::get_transcript_path(project_path, session_id)?;
        if fs::try_exists(&path).await? {
            let file = fs::read_to_string(&path).await?;
            // Sometimes a transcript gets written out that only as a summary and is
            // only 1 line long. These can be considered invalid sessions
            if file.lines().count() > 1 {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
