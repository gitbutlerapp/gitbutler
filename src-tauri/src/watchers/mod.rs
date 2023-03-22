mod delta;
mod files;
mod git;
mod session;

#[cfg(test)]
mod delta_test;
#[cfg(test)]
mod test;

use crate::{deltas, events, projects, search, sessions, users};
use anyhow::Result;
use std::{path::Path, sync::Arc};

pub struct Watcher {
    session_watcher: session::SessionWatcher,
    files_watcher: files::FileWatchers,
}

impl Watcher {
    pub fn new(
        projects_storage: projects::Storage,
        users_storage: users::Storage,
        deltas_searcher: search::Deltas,
    ) -> Self {
        let session_watcher =
            session::SessionWatcher::new(projects_storage, users_storage, deltas_searcher);
        let files_watcher = files::FileWatchers::new();
        Self {
            session_watcher,
            files_watcher,
        }
    }

    pub fn watch(
        &mut self,
        sender: tokio::sync::mpsc::Sender<events::Event>,
        project: &projects::Project,
        deltas_storage: &deltas::Store,
        sessions_storage: &sessions::Store,
    ) -> Result<()> {
        // shared mutex to prevent concurrent write to gitbutler interal state by multiple watchers
        // at the same time
        let lock_file = fslock::LockFile::open(
            &Path::new(&project.path)
                .join(".git")
                .join(format!("gb-{}", project.id))
                .join(".lock"),
        )?;

        let repo = git2::Repository::open(project.path.clone())?;
        repo.add_ignore_rule("*.lock")?;

        let mut fsevents = self.files_watcher.watch(project.clone())?;

        let shared_sender = Arc::new(sender.clone());
        let shared_deltas_store = Arc::new(deltas_storage.clone());
        let shared_lock_file = Arc::new(tokio::sync::Mutex::new(lock_file));

        self.session_watcher.watch(
            sender,
            project.clone(),
            shared_lock_file.clone(),
            deltas_storage,
            sessions_storage,
        )?;

        tauri::async_runtime::spawn(async move {
            let sender = shared_sender;
            let deltas_storage = shared_deltas_store;
            let lock_file = shared_lock_file;
            while let Ok(event) = fsevents.recv().await {
                match event {
                    files::Event::FileChange((project, path)) => {
                        if path.starts_with(Path::new(".git")) {
                            if let Err(e) = git::on_git_file_change(&sender, &project, &path).await
                            {
                                log::error!("{}: {:#}", project.id, e);
                            }
                        } else {
                            if let Err(e) = delta::on_file_change(
                                &sender,
                                lock_file.clone(),
                                &project,
                                &deltas_storage,
                                &path,
                            )
                            .await
                            {
                                log::error!("{}: {:#}", project.id, e);
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub fn unwatch(&mut self, project: projects::Project) -> Result<()> {
        self.files_watcher.unwatch(&project)?;
        // TODO: how to unwatch session ?
        Ok(())
    }
}
