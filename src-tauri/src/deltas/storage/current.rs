use crate::{deltas, fs, projects};
use anyhow::{Context, Result};
use std::{collections::HashMap, path::Path};

#[derive(Clone)]
pub struct Store {
    project: projects::Project,
}

impl Store {
    pub fn new(project: projects::Project) -> Self {
        Self { project }
    }

    pub fn read<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Vec<deltas::Delta>>> {
        let file_deltas_path = self.project.deltas_path().join(file_path);
        if !file_deltas_path.exists() {
            return Ok(None);
        }

        let file_deltas = std::fs::read_to_string(&file_deltas_path).with_context(|| {
            format!(
                "failed to read file deltas from {}",
                file_deltas_path.to_str().unwrap()
            )
        })?;

        let deltas: Vec<deltas::Delta> = serde_json::from_str(&file_deltas).with_context(|| {
            format!(
                "failed to parse file deltas from {}",
                file_deltas_path.to_str().unwrap()
            )
        })?;

        Ok(Some(deltas))
    }

    pub fn write<P: AsRef<Path>>(&self, file_path: P, deltas: &Vec<deltas::Delta>) -> Result<()> {
        let delta_path = self.project.deltas_path().join(file_path);
        let delta_dir = delta_path.parent().unwrap();
        std::fs::create_dir_all(&delta_dir)?;
        log::info!(
            "{}: writing deltas to {}",
            &self.project.id,
            delta_path.to_str().unwrap()
        );
        let raw_deltas = serde_json::to_string(&deltas)?;
        std::fs::write(delta_path.clone(), raw_deltas).with_context(|| {
            format!(
                "failed to write file deltas to {}",
                delta_path.to_str().unwrap()
            )
        })?;

        Ok(())
    }

    // returns deltas for a current session from .gb/session/deltas tree
    pub fn list(&self, paths: Option<Vec<&str>>) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let deltas_path = self.project.deltas_path();
        if !deltas_path.exists() {
            return Ok(HashMap::new());
        }

        let file_paths = fs::list_files(&deltas_path).with_context(|| {
            format!("Failed to list files in {}", deltas_path.to_str().unwrap())
        })?;

        let deltas = file_paths
            .iter()
            .map_while(|file_path| {
                if let Some(paths) = &paths {
                    if !paths.contains(&file_path.to_str().unwrap()) {
                        return None;
                    }
                }
                let file_deltas = self.read(Path::new(file_path));
                match file_deltas {
                    Ok(Some(file_deltas)) => {
                        Some(Ok((file_path.to_str().unwrap().to_string(), file_deltas)))
                    }
                    Ok(None) => None,
                    Err(err) => Some(Err(err)),
                }
            })
            .collect::<Result<HashMap<String, Vec<deltas::Delta>>>>()?;

        Ok(deltas)
    }
}
