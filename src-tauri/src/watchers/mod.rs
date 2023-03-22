mod delta;
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
    delta_watcher: delta::DeltaWatchers,
    git_watcher: git::GitWatchers,
}

impl Watcher {
    pub fn new(
        projects_storage: projects::Storage,
        users_storage: users::Storage,
        deltas_searcher: search::Deltas,
    ) -> Self {
        let session_watcher =
            session::SessionWatcher::new(projects_storage, users_storage, deltas_searcher);
        let delta_watcher = delta::DeltaWatchers::new();
        let git_watcher = git::GitWatchers::new();
        Self {
            session_watcher,
            delta_watcher,
            git_watcher,
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
        let lock_file = Arc::new(tokio::sync::Mutex::new(fslock::LockFile::open(
            &Path::new(&project.path)
                .join(".git")
                .join(format!("gb-{}", project.id))
                .join(".lock"),
        )?));
        let repo = git2::Repository::open(project.path.clone())?;
        repo.add_ignore_rule("*.lock")?;

        self.delta_watcher.watch(
            sender.clone(),
            project.clone(),
            lock_file.clone(),
            deltas_storage,
        )?;
        self.session_watcher.watch(
            sender.clone(),
            project.clone(),
            lock_file.clone(),
            deltas_storage,
            sessions_storage,
        )?;
        self.git_watcher.watch(sender.clone(), project.clone())?;

        Ok(())
    }

    pub fn unwatch(&mut self, project: projects::Project) -> Result<()> {
        self.delta_watcher.unwatch(&project)?;
        self.git_watcher.unwatch(&project)?;
        // TODO: how to unwatch session ?
        Ok(())
    }
}
