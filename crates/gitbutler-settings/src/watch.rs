use std::{
    path::PathBuf,
    sync::{mpsc::channel, Arc, OnceLock, RwLock},
    time::Duration,
};

use crate::{app_settings, AppSettings};
use anyhow::Result;
use notify::{event::ModifyKind, Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

pub struct SettingsHandle {
    config_path: PathBuf,
    app_settings: &'static RwLock<AppSettings>,
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

        static CONFIG: OnceLock<RwLock<AppSettings>> = OnceLock::new();
        let app_settings = CONFIG.get_or_init(|| RwLock::new(app_settings));

        Ok(Self {
            config_path,
            app_settings,
            send_event: Arc::new(send_event),
        })
    }

    pub fn settings(&self) -> &'static RwLock<AppSettings> {
        self.app_settings
    }

    pub fn watch_in_background(&self) -> Result<()> {
        let (tx, rx) = channel();
        let settings = self.app_settings;
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
                        if let Ok(mut s) = settings.try_write() {
                            if let Ok(update) = app_settings::AppSettings::load(config_path.clone())
                            {
                                if *s != update {
                                    tracing::info!("settings.json modified; refreshing settings");
                                    *s = update.clone();
                                    send_event(update)?;
                                }
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
