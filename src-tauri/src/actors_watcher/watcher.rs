use std::{
    collections::HashMap,
    path,
    sync::{Arc, Mutex}, time::Duration, 
};

use actix::{Actor, Context, Handler, Message};
use futures::executor::block_on;
use notify::{RecommendedWatcher, Watcher};
use tokio::{select, sync::mpsc::channel, time::interval};

use crate::projects;

#[derive(Default)]
pub struct BackgroundWatcher {
    file_watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
}

impl Actor for BackgroundWatcher {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("background watcher started");
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("background watcher stopped");
    }
}

pub struct WatchMessage(projects::Project);

impl From<&projects::Project> for WatchMessage {
    fn from(project: &projects::Project) -> Self {
        Self(project.clone())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WatchMessageError {
    #[error(transparent)]
    NotifyError(#[from] notify::Error),
}

impl Message for WatchMessage {
    type Result = Result<(), WatchMessageError>;
}

impl Handler<WatchMessage> for BackgroundWatcher {
    type Result = Result<(), WatchMessageError>;

    fn handle(&mut self, msg: WatchMessage, _ctx: &mut Self::Context) -> Self::Result {
        let (tx, mut rx) = channel(1);
        let mut watcher = notify::RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| match res {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        notify::EventKind::Create(notify::event::CreateKind::File)
                            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
                            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
                            | notify::EventKind::Remove(notify::event::RemoveKind::File)
                    ) {
                        for path in event.paths {
                            if let Err(error) = block_on(tx.send(path)) {
                                log::error!("failed to send file change event: {:#}", error);
                            }
                        }
                    }
                }
                Err(error) => log::error!("file watcher error: {:#}", error),
            },
            notify::Config::default(),
        )
        .map_err(WatchMessageError::from)?;

        watcher
            .watch(
                path::Path::new(&msg.0.path),
                notify::RecursiveMode::Recursive,
            )
            .map_err(WatchMessageError::from)?;

        log::info!("watching project {} on {}", &msg.0.id, msg.0.path);
        self.file_watchers.lock().unwrap().insert(msg.0.id, watcher);

        block_on(async move {
            let mut ticker = interval(Duration::from_secs(1));
            loop {
                select! {
                    instant =  ticker.tick() => {
                        println!("tick: {:?}", instant);
                    },
                    path = rx.recv() => {
                        if let Some(path) = path {
                            log::info!("file changed: {:?}", path);
                        }
                    }
                }
            }
        })
    }
}

pub struct UnwatchMessage(projects::Project);

impl From<&projects::Project> for UnwatchMessage {
    fn from(project: &projects::Project) -> Self {
        Self(project.clone())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnwatchMessageError {
    #[error("not watching")]
    NotWatching,
    #[error("transparent")]
    NotifyError(#[from] notify::Error),
}

impl Message for UnwatchMessage {
    type Result = Result<(), UnwatchMessageError>;
}

impl Handler<UnwatchMessage> for BackgroundWatcher {
    type Result = Result<(), UnwatchMessageError>;

    fn handle(&mut self, msg: UnwatchMessage, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(mut watcher) = self.file_watchers.lock().unwrap().remove(&msg.0.id) {
            watcher
                .unwatch(path::Path::new(&msg.0.path))
                .map_err(UnwatchMessageError::from)?;
            log::info!("stop watching {} on {}", &msg.0.id, &msg.0.path);
            Ok(())
        } else {
            Err(UnwatchMessageError::NotWatching)
        }
    }
}
