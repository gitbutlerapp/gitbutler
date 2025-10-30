use std::path::Path;

use anyhow::Result;
use diesel::{prelude::*, sql_query, sql_types::Text};
use serde::{Deserialize, Serialize};

use crate::workspace_identifier::get_single_folder_workspace_identifier;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    #[serde(rename = "generationUUID")]
    pub generation_uuid: String,
    #[serde(rename = "textDescription")]
    pub text_description: String,
    #[serde(rename = "type")]
    pub generation_type: String,
    #[serde(rename = "unixMs")]
    pub unix_ms: i64,
}

/// Get the base directory for Cursor workspace storage based on the platform
fn get_cursor_base_dir(nightly: bool) -> Result<std::path::PathBuf> {
    let cursor_name = if nightly { "Cursor Nightly" } else { "Cursor" };

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| anyhow::anyhow!("APPDATA environment variable not found"))?;
        Ok(std::path::PathBuf::from(appdata)
            .join(cursor_name)
            .join("User")
            .join("workspaceStorage"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")
            .map_err(|_| anyhow::anyhow!("HOME environment variable not found"))?;
        Ok(std::path::PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(cursor_name)
            .join("User")
            .join("workspaceStorage"))
    }

    #[cfg(target_os = "linux")]
    {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                std::path::PathBuf::from(home).join(".config")
            });
        Ok(config_dir
            .join(cursor_name)
            .join("User")
            .join("workspaceStorage"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        anyhow::bail!("Unsupported platform");
    }
}

/// Get the path to the Cursor database file for the given repository
fn get_cursor_db_path(repo_path: &Path, nightly: bool) -> Result<std::path::PathBuf> {
    let base_dir = get_cursor_base_dir(nightly)?;
    let workspace_id = get_single_folder_workspace_identifier(repo_path)?;

    Ok(base_dir.join(workspace_id).join("state.vscdb"))
}

/// Parse the JSON value from the database into a Vec<Generation>
fn parse_generations_json(json_str: &str) -> Result<Vec<Generation>> {
    let generations: Vec<Generation> = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse generations JSON: {}", e))?;
    Ok(generations)
}

/// Result struct for SQL query
#[derive(QueryableByName)]
struct GenerationQueryResult {
    #[diesel(sql_type = Text)]
    value: String,
}

/// Get AI service generations from the Cursor database for the given repository
pub fn get_generations(repo_path: &Path, nightly: bool) -> Result<Vec<Generation>> {
    let db_path = get_cursor_db_path(repo_path, nightly)?;

    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let db_url = format!("file:{}", db_path.to_string_lossy());
    let mut conn = SqliteConnection::establish(&db_url)
        .map_err(|e| anyhow::anyhow!("Failed to connect to database at {:?}: {}", db_path, e))?;

    let query_result: Result<Vec<GenerationQueryResult>, diesel::result::Error> =
        sql_query("SELECT value FROM ItemTable WHERE key = ?")
            .bind::<Text, _>("aiService.generations")
            .load(&mut conn);

    match query_result {
        Ok(results) => {
            if let Some(result) = results.first() {
                parse_generations_json(&result.value)
            } else {
                Ok(Vec::new()) // Key not found
            }
        }
        Err(e) => Err(anyhow::anyhow!("Database query failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cursor_base_dir_regular() {
        let result = get_cursor_base_dir(false);
        assert!(result.is_ok());
        let path = result.unwrap();

        #[cfg(target_os = "macos")]
        assert!(
            path.to_string_lossy()
                .contains("Library/Application Support/Cursor")
        );

        #[cfg(target_os = "windows")]
        assert!(path.to_string_lossy().contains("\\Cursor\\"));

        #[cfg(target_os = "linux")]
        assert!(
            path.to_string_lossy().contains(".config/Cursor")
                || path.to_string_lossy().contains("Cursor")
        );
    }

    #[test]
    fn test_get_cursor_base_dir_nightly() {
        let result = get_cursor_base_dir(true);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("Cursor Nightly"));
    }

    #[test]
    fn test_parse_generations_json() {
        let json_str = r#"[{
            "generationUUID": "ade2d936-9af0-457d-b16a-7293ec309f5f",
            "textDescription": "Add Esteban 6",
            "type": "composer",
            "unixMs": 1758115352488
        }]"#;

        let result = parse_generations_json(json_str);
        if let Err(e) = &result {
            eprintln!("JSON parsing failed: {e}");
        }
        assert!(result.is_ok());

        let generations = result.unwrap();
        assert_eq!(generations.len(), 1);

        let generation = &generations[0];
        assert_eq!(
            generation.generation_uuid,
            "ade2d936-9af0-457d-b16a-7293ec309f5f"
        );
        assert_eq!(generation.text_description, "Add Esteban 6");
        assert_eq!(generation.generation_type, "composer");
        assert_eq!(generation.unix_ms, 1758115352488);
    }

    #[test]
    fn test_get_cursor_db_path() {
        // Use current directory which should exist
        let repo_path = std::env::current_dir().unwrap();
        let result = get_cursor_db_path(&repo_path, false);
        if let Err(e) = &result {
            eprintln!("get_cursor_db_path failed: {e}");
        }
        assert!(result.is_ok());

        let db_path = result.unwrap();
        assert!(db_path.to_string_lossy().ends_with("state.vscdb"));
    }

    #[test]
    fn test_get_generations_nonexistent_db() {
        // Use current directory but the database file won't exist
        let repo_path = std::env::current_dir().unwrap();
        let result = get_generations(&repo_path, false);
        if let Err(e) = &result {
            eprintln!("get_generations failed: {e}");
        }
        assert!(result.is_ok());

        let generations = result.unwrap();
        assert_eq!(generations.len(), 0);
    }
}
