use crate::DbHandle;
use bitflags::bitflags;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

bitflags! {
    /// What kind of data to listen to
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct ItemKind: u8 {
        const Actions = 1 << 0;
        const Workflows = 1 << 1;
        const Assignments = 1 << 2;
        const Rules = 1 << 3;
        const ClaudePermissionRequests = 1 << 4;
    }
}

impl DbHandle {
    /// Register polling at `interval` for any `kind` of data and return a channel to be informed about the changes
    /// for the respective kind.
    /// Drop the receiver for the polling to stop.
    /// Note that this opens a new connection.
    ///
    /// ### Shortcoming
    ///
    /// The current implementation will send a change event the first time it starts up.
    pub fn poll_changes(
        &self,
        kind: ItemKind,
        interval: std::time::Duration,
    ) -> anyhow::Result<std::sync::mpsc::Receiver<anyhow::Result<ItemKind>>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut this = DbHandle::new_at_url(&self.url)?;
        std::thread::Builder::new()
            .name("Gitbutler-DB-watcher".into())
            .spawn(move || {
                let mut prev_assignments = Vec::new();
                let mut prev_workflows = Vec::new();
                let mut prev_actions = Vec::new();
                let mut prev_rules = Vec::new();
                let mut prev_claude_requests = Vec::new();
                'outer: loop {
                    std::thread::sleep(interval);
                    for to_check in ItemKind::all().iter() {
                        let send_result = if kind & to_check == ItemKind::Actions {
                            let res = this.butler_actions().list(0, i64::MAX);
                            match res {
                                Ok((_num_items, items)) => {
                                    if items != prev_actions {
                                        prev_actions = items;
                                        tx.send(Ok(ItemKind::Actions))
                                    } else {
                                        continue;
                                    }
                                }
                                Err(e) => tx.send(Err(e)),
                            }
                        } else if kind & to_check == ItemKind::Workflows {
                            let res = this.workflows().list(0, i64::MAX);
                            match res {
                                Ok((_num_items, items)) => {
                                    if items != prev_workflows {
                                        prev_workflows = items;
                                        tx.send(Ok(ItemKind::Workflows))
                                    } else {
                                        continue;
                                    }
                                }
                                Err(e) => tx.send(Err(e)),
                            }
                        } else if kind & to_check == ItemKind::Assignments {
                            let res = this.hunk_assignments().list_all();
                            match res {
                                Ok(items) => {
                                    if items != prev_assignments {
                                        prev_assignments = items;
                                        tx.send(Ok(ItemKind::Assignments))
                                    } else {
                                        continue;
                                    }
                                }
                                Err(e) => tx.send(Err(e)),
                            }
                        } else if kind & to_check == ItemKind::Rules {
                            let res = this.workspace_rules().list();
                            match res {
                                Ok(items) => {
                                    if items != prev_rules {
                                        prev_rules = items;
                                        tx.send(Ok(ItemKind::Rules))
                                    } else {
                                        continue;
                                    }
                                }
                                Err(e) => tx.send(Err(anyhow::Error::from(e))),
                            }
                        } else if kind & to_check == ItemKind::ClaudePermissionRequests {
                            let res = this.claude_permission_requests().list();
                            match res {
                                Ok(items) => {
                                    if items != prev_claude_requests {
                                        prev_claude_requests = items;
                                        tx.send(Ok(ItemKind::ClaudePermissionRequests))
                                    } else {
                                        continue;
                                    }
                                }
                                Err(e) => tx.send(Err(anyhow::Error::from(e))),
                            }
                        } else {
                            eprintln!("BUG: didn't implement a branch for {to_check:?}");
                            break 'outer;
                        };
                        if send_result.is_err() {
                            break 'outer;
                        }
                    }
                }
            })?;
        Ok(rx)
    }

    /// Register async polling at `interval` for any `kind` of data and return a channel to be informed about the changes
    /// for the respective kind. Uses async tokio task and channel for efficiency.
    pub fn poll_changes_async(
        &self,
        kind: ItemKind,
        interval: std::time::Duration,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<anyhow::Result<ItemKind>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        let url = self.url.clone();
        tokio::spawn(async move {
            let mut this = match DbHandle::new_at_url(&url) {
                Ok(db) => db,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            };
            let mut prev_assignments = Vec::new();
            let mut prev_workflows = Vec::new();
            let mut prev_actions = Vec::new();
            let mut prev_rules = Vec::new();
            let mut prev_claude_requests = Vec::new();
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                for to_check in ItemKind::all().iter() {
                    let send_result = if kind & to_check == ItemKind::Actions {
                        let res = this.butler_actions().list(0, i64::MAX);
                        match res {
                            Ok((_num_items, items)) => {
                                if items != prev_actions {
                                    prev_actions = items;
                                    tx.send(Ok(ItemKind::Actions)).await
                                } else {
                                    continue;
                                }
                            }
                            Err(e) => tx.send(Err(e)).await,
                        }
                    } else if kind & to_check == ItemKind::Workflows {
                        let res = this.workflows().list(0, i64::MAX);
                        match res {
                            Ok((_num_items, items)) => {
                                if items != prev_workflows {
                                    prev_workflows = items;
                                    tx.send(Ok(ItemKind::Workflows)).await
                                } else {
                                    continue;
                                }
                            }
                            Err(e) => tx.send(Err(e)).await,
                        }
                    } else if kind & to_check == ItemKind::Assignments {
                        let res = this.hunk_assignments().list_all();
                        match res {
                            Ok(items) => {
                                if items != prev_assignments {
                                    prev_assignments = items;
                                    tx.send(Ok(ItemKind::Assignments)).await
                                } else {
                                    continue;
                                }
                            }
                            Err(e) => tx.send(Err(e)).await,
                        }
                    } else if kind & to_check == ItemKind::Rules {
                        let res = this.workspace_rules().list();
                        match res {
                            Ok(items) => {
                                if items != prev_rules {
                                    prev_rules = items;
                                    tx.send(Ok(ItemKind::Rules)).await
                                } else {
                                    continue;
                                }
                            }
                            Err(e) => tx.send(Err(anyhow::Error::from(e))).await,
                        }
                    } else if kind & to_check == ItemKind::ClaudePermissionRequests {
                        let res = this.claude_permission_requests().list();
                        match res {
                            Ok(items) => {
                                if items != prev_claude_requests {
                                    prev_claude_requests = items;
                                    tx.send(Ok(ItemKind::ClaudePermissionRequests)).await
                                } else {
                                    continue;
                                }
                            }
                            Err(e) => tx.send(Err(anyhow::Error::from(e))).await,
                        }
                    } else {
                        eprintln!("BUG: didn't implement a branch for {to_check:?}");
                        return;
                    };
                    if send_result.is_err() {
                        return;
                    }
                }
            }
        });
        Ok(rx)
    }
}

pub struct DBWatcherHandle {
    pub cancel_tx: Option<oneshot::Sender<()>>,
    pub handle: JoinHandle<()>,
}

impl Drop for DBWatcherHandle {
    fn drop(&mut self) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(()); // signal cancellation
            self.handle.abort();
        }
        tracing::info!("Stopped DB watcher");
    }
}

pub fn watch_in_background(
    db: &mut DbHandle,
    send_event: impl Fn(ItemKind) -> anyhow::Result<()> + Send + Sync + 'static,
) -> anyhow::Result<DBWatcherHandle, anyhow::Error> {
    let mut rx = db.poll_changes_async(
        ItemKind::Actions | ItemKind::Workflows | ItemKind::Assignments | ItemKind::Rules,
        std::time::Duration::from_millis(500),
    )?;

    let (cancel_tx, mut cancel_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut cancel_rx => {
                    tracing::info!("DB watcher cancelled");
                    break;
                }
                maybe_item = rx.recv() => {
                    match maybe_item {
                        Some(Ok(item)) => {
                            tracing::debug!("Received item: {:?}", item);
                            send_event(item)
                                .unwrap_or_else(|e| tracing::error!("Error sending event: {:?}", e));
                        }
                        Some(Err(e)) => {
                            tracing::error!("Error receiving item: {:?}", e);
                            break;
                        }
                        None => {
                            // Channel closed
                            break;
                        }
                    }
                }
            }
        }
    });

    let watcher_handle = DBWatcherHandle {
        cancel_tx: Some(cancel_tx),
        handle,
    };

    Ok(watcher_handle)
}
