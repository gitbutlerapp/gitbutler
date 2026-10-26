//! The `but` CLI, which Settings and the welcome offer to install on macOS.

use gpui::Context;

use super::Store;
use crate::backend;

impl Store {
    /// Look for `but` again, as Settings does when it opens, in case a terminal installed it.
    /// Only on macOS, the one platform the app installs it on, so elsewhere nothing offers it.
    pub fn read_cli(&mut self, cx: &mut Context<Self>) {
        if !cfg!(target_os = "macos") {
            return;
        }
        cx.spawn(async move |this, cx| {
            let Ok(installed) = backend::run(|| Ok(backend::cli::installed())).await else {
                return;
            };
            this.update(cx, |store, cx| {
                store.cli = Some(installed);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn install_cli(&mut self, cx: &mut Context<Self>) {
        if self.installing_cli {
            return;
        }
        self.installing_cli = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let installed = backend::run(backend::cli::install).await;
            this.update(cx, |store, cx| {
                store.installing_cli = false;
                match installed {
                    Ok(true) => store.snack("CLI installed", cx),
                    // Cancelled at the password prompt.
                    Ok(false) => cx.notify(),
                    Err(err) => store.failed("Couldn't install the CLI", &err, cx),
                }
                store.read_cli(cx);
            })
            .ok();
        })
        .detach();
    }
}
