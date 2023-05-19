use anyhow::Result;

use crate::watcher::events;

pub struct Handler {}

impl Handler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle<P: AsRef<std::path::Path>>(&self, path: P) -> Result<Vec<events::Event>> {
        let path = path.as_ref();
        if !path.starts_with(".git") {
            Ok(vec![events::Event::ProjectFileChange(path.to_path_buf())])
        } else {
            Ok(vec![events::Event::GitFileChange(
                path.strip_prefix(".git").unwrap().to_path_buf(),
            )])
        }
    }
}
