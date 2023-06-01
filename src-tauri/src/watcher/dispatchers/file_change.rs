use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use futures::{
    channel::mpsc::{channel, Receiver},
    SinkExt, StreamExt,
};
use notify::{Config, Event, RecommendedWatcher, Watcher};

use crate::watcher::events;

#[derive(Debug, Clone)]
pub struct Dispatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    project_path: PathBuf,
    project_id: String,
}

impl Dispatcher {
    pub fn new<P: AsRef<std::path::Path>>(project_id: String, path: P) -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
            project_path: path.as_ref().to_path_buf(),
            project_id,
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

    pub async fn start(&self, rtx: crossbeam_channel::Sender<events::Event>) -> Result<()> {
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

        while let Some(res) = rx.next().await {
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

fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                if let Err(err) = tx.send(res).await {
                    log::error!("failed to send file change event: {:#}", err);
                }
                println!("sent");
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}
