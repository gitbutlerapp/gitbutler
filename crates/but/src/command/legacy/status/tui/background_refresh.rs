use std::{
    path::Path,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{Receiver, Sender},
    },
    time::{Duration, Instant},
};

use but_ctx::Context;

use crate::setup::{build_background_sync_command, determine_sync_operations};

#[derive(Debug)]
pub(super) struct BackgroundRefresh {
    num_refreshes: usize,
    last_refresh_at: Option<Instant>,
    rx: Receiver<()>,
    tx: Sender<()>,
    is_refreshing: Arc<AtomicBool>,
}

impl BackgroundRefresh {
    pub(super) fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            num_refreshes: 0,
            last_refresh_at: None,
            rx,
            tx,
            is_refreshing: Default::default(),
        }
    }

    pub(super) fn update(&mut self, ctx: &mut Context, current_dir: &Path) {
        let should_refresh = self
            .last_refresh_at
            .is_none_or(|t| t.elapsed() > Duration::from_secs(10));

        // let should_refresh = match self.num_refreshes {
        //     // do one refresh right away
        //     0 => true,
        //     // the first 10 minutes refresh every minute
        //     1..10 => self
        //         .last_refresh_at
        //         .is_none_or(|t| t.elapsed() > Duration::from_secs(60 * 5)),
        //     // afterwards refresh every 15 minutes
        //     _ => self
        //         .last_refresh_at
        //         .is_none_or(|t| t.elapsed() > Duration::from_secs(60 * 15)),
        // };

        if !should_refresh {
            return;
        }

        self.num_refreshes += 1;
        self.last_refresh_at = Some(Instant::now());

        let sync_operations = determine_sync_operations(ctx, 1, None);

        if !sync_operations.has_work() {
            return;
        }

        if let Some(cmd) = build_background_sync_command(current_dir, sync_operations) {
            let tx = self.tx.clone();
            let is_refreshing = Arc::clone(&self.is_refreshing);
            std::thread::spawn(move || -> std::io::Result<()> {
                let mut child = cmd.into_std().spawn()?;
                is_refreshing.store(true, std::sync::atomic::Ordering::SeqCst);
                if child.wait().is_ok() {
                    _ = tx.send(());
                }
                is_refreshing.store(false, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            });
        }
    }

    pub(super) fn needs_reload(&mut self) -> bool {
        self.rx.try_recv().is_ok()
    }

    pub(super) fn is_refreshing(&self) -> bool {
        self.is_refreshing.load(std::sync::atomic::Ordering::SeqCst)
    }
}
