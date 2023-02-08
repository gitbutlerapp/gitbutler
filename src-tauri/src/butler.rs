use crate::{crdt, fs};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub fn deltas_path(project_path: &Path) -> Result<PathBuf, std::io::Error> {
    let path = project_path.join(".git/gb/session/deltas");
    std::fs::create_dir_all(path.clone())?;
    Ok(path)
}

pub fn get_file_deltas(
    project_path: &Path,
    file_path: &Path,
) -> Result<Option<Vec<crdt::Delta>>, Box<dyn std::error::Error>> {
    let project_deltas_path = &deltas_path(project_path)?;
    let delta_path = project_deltas_path.join(file_path.to_path_buf());
    if delta_path.exists() {
        let raw_deltas = std::fs::read_to_string(delta_path.clone())?;
        let deltas: Vec<crdt::Delta> = serde_json::from_str(&raw_deltas)?;
        Ok(Some(deltas))
    } else {
        Ok(None)
    }
}

pub fn save_file_deltas(
    repo: &git2::Repository,
    file_path: &Path,
    deltas: &Vec<crdt::Delta>,
) -> Result<(), std::io::Error> {
    if deltas.is_empty() {
        Ok(())
    } else {
        let project_deltas_path = &deltas_path(repo.workdir().unwrap())?;
        let delta_path = project_deltas_path.join(file_path.to_path_buf());
        log::info!("Writing delta to {}", delta_path.to_str().unwrap());
        let raw_deltas = serde_json::to_string(&deltas).unwrap();
        std::fs::write(delta_path, raw_deltas)?;
        Ok(())
    }
}

pub fn list_deltas(
    project_path: &Path,
) -> Result<HashMap<String, Vec<crdt::Delta>>, Box<dyn std::error::Error>> {
    let deltas_path = &deltas_path(project_path)?;
    let file_paths = fs::list_files(&deltas_path)?;
    let mut deltas = HashMap::new();
    for file_path in file_paths {
        let file_path = Path::new(&file_path);
        let file_deltas = get_file_deltas(project_path, file_path)?;
        if let Some(file_deltas) = file_deltas {
            deltas.insert(file_path.to_str().unwrap().to_string(), file_deltas);
        }
    }
    Ok(deltas)
}
