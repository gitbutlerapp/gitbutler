use std::sync;

use anyhow::{Context, Result};

use crate::{app::gb_repository, events, projects};

use super::{git_file_change, project_file_change};

pub struct Listener<'listener> {
    project_file_change_listener: project_file_change::Listener<'listener>,
    git_file_change_listener: git_file_change::Listener,
}

impl<'listener> Listener<'listener> {
    pub fn new(
        project_id: String,
        project_store: projects::Storage,
        gb_repository: &'listener gb_repository::Repository,
        events: sync::mpsc::Sender<events::Event>,
    ) -> Self {
        Self {
            project_file_change_listener: project_file_change::Listener::new(
                project_id.clone(),
                project_store.clone(),
                gb_repository,
                events.clone(),
            ),
            git_file_change_listener: git_file_change::Listener::new(
                project_id,
                project_store,
                events,
            ),
        }
    }

    pub fn register<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if !path.starts_with(".git") {
            self.project_file_change_listener
                .register(&path)
                .with_context(|| {
                    format!(
                        "failed to register project file change for path: {}",
                        path.display()
                    )
                })
        } else {
            self.git_file_change_listener
                .register(path)
                .with_context(|| {
                    format!(
                        "failed to register git file change for path: {}",
                        path.display()
                    )
                })
        }
    }
}
