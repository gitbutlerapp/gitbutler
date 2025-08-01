use crate::AppSettings;
use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, mpsc},
    time::Duration,
};

/// A monitor for [`AppSettings`] on disk which will keep its internal state in sync with
/// what's on disk.
///
/// It will also distribute the latest version of the application settings.
#[derive(Clone)]
pub struct AppSettingsWithDiskSync {
    config_path: PathBuf,
    /// The source of truth for the application settings, as previously read from disk.
    snapshot: Arc<RwLock<AppSettings>>,
}

/// Allow changes to the most recent [`AppSettings`] and force them to be saved.
pub(crate) struct AppSettingsEnforceSaveToDisk<'a> {
    config_path: &'a Path,
    snapshot: RwLockWriteGuard<'a, AppSettings>,
    saved: bool,
}

impl AppSettingsEnforceSaveToDisk<'_> {
    pub fn save(&mut self) -> Result<()> {
        // Mark as completed first so failure to save will not make us complain about not saving.
        self.saved = true;
        self.snapshot.save(self.config_path)?;
        Ok(())
    }
}

impl Deref for AppSettingsEnforceSaveToDisk<'_> {
    type Target = AppSettings;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl DerefMut for AppSettingsEnforceSaveToDisk<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.snapshot
    }
}

impl Drop for AppSettingsEnforceSaveToDisk<'_> {
    fn drop(&mut self) {
        assert!(
            self.saved,
            "BUG: every change must immediately be saved to disk."
        );
    }
}

pub(crate) const SETTINGS_FILE: &str = "settings.json";

impl AppSettingsWithDiskSync {
    /// Create a new instance without actually starting to [watch in the background](Self::watch_in_background()).
    ///
    /// * `config_dir` contains the application settings file.
    /// * `subscriber` receives any change to it.
    pub fn new(config_dir: impl AsRef<Path>) -> Result<Self> {
        let config_path = dbg!(config_dir.as_ref().join(SETTINGS_FILE));
        let app_settings = AppSettings::load(&config_path)?;
        let app_settings = Arc::new(RwLock::new(app_settings));

        Ok(Self {
            config_path,
            snapshot: app_settings,
        })
    }

    /// Return a reference to the most recently loaded [`AppSettings`].
    pub fn get(&self) -> Result<RwLockReadGuard<'_, AppSettings>> {
        self.snapshot
            .read()
            .map_err(|e| anyhow::anyhow!("Could not read settings: {:?}", e))
    }

    /// Allow changes only from within this crate to implement all possible settings updates [here](crate::api).
    pub(crate) fn get_mut_enforce_save(&self) -> Result<AppSettingsEnforceSaveToDisk<'_>> {
        self.snapshot
            .write()
            .map(|snapshot| AppSettingsEnforceSaveToDisk {
                snapshot,
                config_path: &self.config_path,
                saved: false,
            })
            .map_err(|e| anyhow::anyhow!("Could not write settings: {:?}", e))
    }

    /// The path from which application settings will be read from disk.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Start watching [`Self::config_path()`] for changes and inform
    pub fn watch_in_background(
        &mut self,
        send_event: impl Fn(AppSettings) -> Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let snapshot = self.snapshot.clone();
        let config_path = self.config_path.to_owned();
        let watcher_config = Config::default()
            .with_compare_contents(true)
            .with_poll_interval(Duration::from_secs(2));
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut watcher: RecommendedWatcher = Watcher::new(tx, watcher_config)?;
            watcher.watch(&config_path, RecursiveMode::NonRecursive)?;
            loop {
                match rx.recv() {
                    Ok(Ok(Event {
                        kind: notify::event::EventKind::Modify(ModifyKind::Data(_)),
                        ..
                    })) => {
                        let Ok(mut last_seen_settings) = snapshot.write() else {
                            continue;
                        };
                        if let Ok(update) = AppSettings::load(&config_path) {
                            tracing::info!("settings.json modified; refreshing settings");
                            *last_seen_settings = update.clone();
                            send_event(update)?;
                        }
                    }

                    Err(_) => {
                        tracing::error!(
                            "Error watching config file {:?} - watcher terminated",
                            config_path
                        );
                        break;
                    }

                    _ => {
                        // Noop
                    }
                }
            }
            Ok(())
        });
        Ok(())
    }
}
