use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    #[serde(rename = "type")]
    pub activity_type: String,
    pub timestamp_ms: u128,
    pub message: String,
}

pub fn parse_reflog_line(line: &str) -> Result<Activity> {
    match line.split("\t").collect::<Vec<&str>>()[..] {
        [meta, message] => {
            let meta_parts = meta.split_whitespace().collect::<Vec<&str>>();
            let timestamp_s = meta_parts[meta_parts.len() - 2].parse::<u64>()?;

            match message.split(": ").collect::<Vec<&str>>()[..] {
                [entry_type, msg] => Ok(Activity {
                    activity_type: entry_type.to_string(),
                    message: msg.to_string(),
                    timestamp_ms: timestamp_s as u128 * 1000,
                }),
                _ => Err(anyhow!("failed to parse reflog line: {}", line)),
            }
        }
        _ => Err(anyhow!("failed to parse reflog line: {}", line)),
    }
}
