use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::BufRead, path::PathBuf};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
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

pub(crate) fn first_cwd(records: &Vec<Record>) -> Result<String, anyhow::Error> {
    for record in records {
        if let Record::User { cwd: Some(cwd), .. } = record {
            return Ok(cwd.to_string());
        }
    }
    Err(anyhow::anyhow!("No user record with cwd found"))
}

pub(crate) fn summary(records: &[Record]) -> Option<String> {
    for record in records.iter().rev() {
        if let Record::Summary { summary, .. } = record {
            return Some(summary.to_string());
        }
    }
    None
}

/// Gets the user message from a record if it exists.
pub(crate) fn prompt(records: &[Record]) -> Option<String> {
    for record in records.iter().rev() {
        if let Record::User {
            message: Some(msg), ..
        } = record
        {
            if let Some(content) = &msg.content {
                if let Some(text) = content.as_str() {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn parse_jsonl(path: String) -> Result<Vec<Record>, anyhow::Error> {
    let file =
        std::fs::File::open(path).map_err(|e| anyhow::anyhow!("Failed to open file: {}", e))?;
    let reader = std::io::BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| anyhow::anyhow!("Failed to read line: {}", e))?;
        if !line.trim().is_empty() {
            let record: Record = serde_json::from_str(&line)
                .map_err(|e| anyhow::anyhow!("Failed to parse JSON line: {}", e))?;
            records.push(record);
        }
    }
    Ok(records)
}

impl TryFrom<PathBuf> for Record {
    type Error = anyhow::Error;
    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        let value = std::fs::read_to_string(value)
            .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;
        serde_json::from_str(&value).map_err(|e| anyhow::anyhow!("Failed to parse record: {}", e))
    }
}

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
