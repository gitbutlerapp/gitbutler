use std::{
    path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use notify::{Config, Event, RecommendedWatcher, Watcher};
use tokio::sync::mpsc;

use crate::{watcher::events, projects};

#[derive(Debug, Clone)]
pub struct Dispatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    project_path: path::PathBuf,
    project_id: String,
}

impl Dispatcher {
    pub fn new(project: &projects::Project) -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
            project_path: path::PathBuf::from(&project.path),
            project_id: project.id.clone(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        if let Some(mut watcher) = self.watcher.lock().unwrap().take() {
            watcher
                .unwatch(std::path::Path::new(&self.project_path))
                .context(format!(
                    "failed to unwatch project path: {}",
                    self.project_path.display()
                ))?;
        }
        Ok(())
    }

    pub async fn start(&self, rtx: mpsc::UnboundedSender<events::Event>) -> Result<()> {
        let (mut watcher, mut rx) = async_watcher()?;
        watcher
            .watch(
                std::path::Path::new(&self.project_path),
                notify::RecursiveMode::Recursive,
            )
            .with_context(|| {
                format!(
                    "failed to watch project path: {}",
                    self.project_path.display()
                )
            })?;
        self.watcher.lock().unwrap().replace(watcher);

        log::info!("{}: file watcher started", self.project_id);

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => {
                    if !is_interesting_event(&event.kind) {
                        continue;
                    }
                    for file_path in event.paths {
                        match file_path.strip_prefix(&self.project_path) {
                            Ok(relative_file_path) => {
                                if let Err(e) = rtx.send(events::Event::FileChange(
                                    relative_file_path.to_path_buf(),
                                )) {
                                    log::error!(
                                        "{}: failed to send file change event: {:#}",
                                        self.project_id,
                                        e
                                    );
                                }
                            }
                            Err(err) => log::error!(
                                "{}: failed to strip prefix: {:#}",
                                self.project_id,
                                err
                            ),
                        }
                    }
                }
                Err(e) => log::error!("{}: file watcher error: {:#}", self.project_id, e),
            }
        }

        log::info!("{}: file watcher stopped", self.project_id);

        Ok(())
    }
}

fn is_interesting_event(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(notify::event::CreateKind::File)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            | notify::EventKind::Remove(notify::event::RemoveKind::File)
    )
}

fn async_watcher() -> notify::Result<(
    RecommendedWatcher,
    mpsc::UnboundedReceiver<notify::Result<Event>>,
)> {
    let (tx, rx) = mpsc::unbounded_channel();

    let watcher = RecommendedWatcher::new(
        move |res| {
            if let Err(err) = tx.send(res) {
                log::error!("failed to send file change event: {:#}", err);
            }
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}
