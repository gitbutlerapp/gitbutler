use std::{
    path::Path,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant, SystemTime},
};

use but_ctx::Context;

use crate::setup::{build_background_sync_command, determine_sync_operations};

#[derive(Debug)]
pub(super) struct BackgroundRefresh {
    num_refreshes: usize,
    last_fetch: Option<std::time::SystemTime>,
    rx: Receiver<()>,
    tx: Sender<()>,
    force_refresh: bool,
}

impl BackgroundRefresh {
    pub(super) fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            num_refreshes: 0,
            last_fetch: None,
            rx,
            tx,
            force_refresh: true,
        }
    }

    pub(super) fn update(&mut self, ctx: &mut Context, current_dir: &Path) {
        if std::env::var("NO_BG_TASKS").is_ok() {
            return;
        }

        let sync_operations = if std::mem::take(&mut self.force_refresh) {
            let mut sync_operations = determine_sync_operations(
                ctx,
                ctx.settings.fetch.auto_fetch_interval_minutes,
                None,
            );
            sync_operations.pr = true;
            sync_operations.ci = true;
            sync_operations
        } else {
            determine_sync_operations(
                ctx,
                ctx.settings.fetch.auto_fetch_interval_minutes,
                self.last_fetch,
            )
        };

        self.num_refreshes += 1;
        self.last_fetch = Some(SystemTime::now());

        if !sync_operations.has_work() {
            return;
        }

        if let Some(cmd) = build_background_sync_command(current_dir, sync_operations) {
            let tx = self.tx.clone();
            std::thread::spawn(move || -> std::io::Result<()> {
                let mut child = cmd.into_std().spawn()?;
                if child.wait().is_ok() {
                    _ = tx.send(());
                }
                Ok(())
            });
        }
    }

    pub(super) fn force_refresh(&mut self) {
        self.force_refresh = true;
    }

    pub(super) fn needs_reload(&mut self) -> bool {
        self.rx.try_recv().is_ok()
    }
}
