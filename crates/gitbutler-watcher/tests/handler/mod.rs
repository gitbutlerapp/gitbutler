use tempfile::TempDir;

mod support {
    use gitbutler_core::{assets, deltas, git, sessions, virtual_branches};
    use tempfile::TempDir;

    /// Like [`gitbutler_testsupport::Suite`], but with all the instances needed to build a handler
    pub struct Fixture {
        inner: gitbutler_testsupport::Suite,
        pub sessions_db: sessions::Database,
        pub deltas_db: deltas::Database,
        pub vbranch_controller: virtual_branches::Controller,
        pub assets_proxy: assets::Proxy,

        /// Keeps changes emitted from the last created handler.
        changes: Option<std::sync::mpsc::Receiver<gitbutler_watcher::Change>>,
        /// Storage for the databases, to be dropped last.
        _tmp: TempDir,
    }

    impl std::ops::Deref for Fixture {
        type Target = gitbutler_testsupport::Suite;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl Default for Fixture {
        fn default() -> Self {
            let (db, tmp) = gitbutler_testsupport::test_database();
            let inner = gitbutler_testsupport::Suite::default();
            let sessions_db = sessions::Database::new(db.clone());
            let deltas_db = deltas::Database::new(db);
            let git_credentials_helper =
                git::credentials::Helper::new(inner.keys.clone(), inner.users.clone(), None);
            let vbranch_controller = virtual_branches::Controller::new(
                inner.projects.clone(),
                inner.users.clone(),
                inner.keys.clone(),
                git_credentials_helper,
            );
            let assets_proxy = assets::Proxy::new(tmp.path().to_owned());
            Fixture {
                inner,
                sessions_db,
                deltas_db,
                vbranch_controller,
                assets_proxy,
                changes: None,
                _tmp: tmp,
            }
        }
    }

    impl Fixture {
        /// Must be mut as handler events are collected into the fixture automatically.
        ///
        /// Note that this only works for the most recent created handler.
        pub fn new_handler(&mut self) -> gitbutler_watcher::Handler {
            let (tx, rx) = std::sync::mpsc::channel();
            self.changes = Some(rx);
            gitbutler_watcher::Handler::new(
                self.local_app_data().to_owned(),
                self.users.clone(),
                self.projects.clone(),
                self.vbranch_controller.clone(),
                self.assets_proxy.clone(),
                self.sessions_db.clone(),
                self.deltas_db.clone(),
                move |event| tx.send(event.clone()).map_err(Into::into),
            )
        }

        /// Returns the events that were emitted to the tauri app.
        pub fn events(&mut self) -> Vec<gitbutler_watcher::Change> {
            let Some(rx) = self.changes.as_ref() else {
                return Vec::new();
            };
            let mut out = Vec::new();
            // For safety, in case the `handler` is still alive, blocking consumption.
            while let Ok(event) = rx.try_recv() {
                out.push(event);
            }
            out
        }
    }
}

use gitbutler_testsupport::init_opts_bare;

fn test_remote_repository() -> anyhow::Result<(git2::Repository, TempDir)> {
    let tmp = tempfile::tempdir()?;
    let repo_a = git2::Repository::init_opts(&tmp, &init_opts_bare())?;
    Ok((repo_a, tmp))
}

mod calculate_delta;
mod fetch_gitbutler_data;
mod git_file_change;
mod push_project_to_gitbutler;
