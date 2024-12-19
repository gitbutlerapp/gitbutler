use std::{
    path::PathBuf,
    sync::{mpsc, Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::Duration,
};

use crate::{app_settings, AppSettings};
use anyhow::Result;
use notify::{event::ModifyKind, Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

pub struct SettingsHandle {
    config_path: PathBuf,
    app_settings: Arc<RwLock<AppSettings>>,
    #[allow(clippy::type_complexity)]
    send_event: Arc<dyn Fn(AppSettings) -> Result<()> + Send + Sync + 'static>,
}

const SETTINGS_FILE: &str = "settings.json";

impl SettingsHandle {
    pub fn create(
        config_dir: impl Into<PathBuf>,
        send_event: impl Fn(AppSettings) -> Result<()> + Send + Sync + 'static,
    ) -> Result<Self> {
        let config_path = config_dir.into().join(SETTINGS_FILE);
        let app_settings = app_settings::AppSettings::load(config_path.clone())?;

        let app_settings = Arc::new(RwLock::new(app_settings));
        Ok(Self {
            config_path,
            app_settings,
            send_event: Arc::new(send_event),
        })
    }

    pub fn read(&self) -> Result<RwLockReadGuard<'_, AppSettings>> {
        self.app_settings
            .try_read()
            .map_err(|e| anyhow::anyhow!("Could not read settings: {:?}", e))
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, AppSettings>> {
        self.app_settings
            .try_write()
            .map_err(|e| anyhow::anyhow!("Could not write settings: {:?}", e))
    }

    pub fn config_path(&self) -> PathBuf {
        self.config_path.clone()
    }

    pub fn watch_in_background(&self) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let settings = self.app_settings.clone();
        let config_path = self.config_path.clone();
        let send_event = self.send_event.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let watcher_config = Config::default()
                .with_compare_contents(true)
                .with_poll_interval(Duration::from_secs(2));
            let mut watcher: RecommendedWatcher = Watcher::new(tx, watcher_config)?;
            watcher.watch(config_path.as_path(), RecursiveMode::NonRecursive)?;
            loop {
                match rx.recv() {
                    Ok(Ok(Event {
                        kind: notify::event::EventKind::Modify(ModifyKind::Data(_)),
                        ..
                    })) => {
                        let Ok(mut last_seen_settings) = settings.try_write() else {
                            continue;
                        };
                        if let Ok(update) = app_settings::AppSettings::load(config_path.clone()) {
                            if *last_seen_settings != update {
                                tracing::info!("settings.json modified; refreshing settings");
                                *last_seen_settings = update.clone();
                                send_event(update)?;
                            }
                        }
                    }

                    Err(e) => {
                        tracing::error!("Error watching config file {:?}: {:?}", config_path, e)
                    }

                    _ => {
                        // Noop
                    }
                }
            }
        });
        Ok(())
    }
}
