use crate::projects;
use anyhow::{Context, Result};
use serde::Serialize;
use serde_jsonlines::WriteExt;
use std::fs::OpenOptions;

pub struct Recorder {
    file: std::fs::File,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Type {
    Input,
    Output,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub timestamp: u128,
    #[serde(rename = "type")]
    pub typ: Type,
    pub bytes: Vec<u8>,
}

impl Recorder {
    pub fn open(project: projects::Project) -> Result<Self> {
        let dir = project.session_path();
        std::fs::create_dir_all(&dir).with_context(|| "failed to create session directory")?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&dir.join("pty.jsonl"))
            .with_context(|| "failed to open pty.jsonl file")?;
        Ok(Self { file })
    }

    pub fn record(&mut self, typ: Type, bytes: &Vec<u8>) -> Result<()> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let record = Record {
            timestamp,
            typ,
            bytes: bytes.to_vec(),
        };

        self.file
            .write_json_lines(vec![record])
            .with_context(|| "failed to write to pty.jsonl file")?;

        Ok(())
    }
}
