use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
// Static map to store watcher handles and cancellation senders per project
type WatcherMap = Arc<Mutex<HashMap<ProjectId, (oneshot::Sender<()>, JoinHandle<()>)>>>;
static WATCHERS: Lazy<WatcherMap> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

use crate::{error::Error, ChangeForFrontend};
use but_db::poll::ItemKind;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(app_handle, projects, settings), err(Debug))]
pub fn start_watching_db(
    app_handle: tauri::AppHandle,
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> anyhow::Result<(), Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;

    // If already watching, stop previous watcher first
    if let Some((tx, handle)) = WATCHERS.lock().remove(&project_id) {
        let _ = tx.send(()); // signal cancellation
        handle.abort();
    }

    let (tx, handle) = watch_db_in_background(ctx, {
        let app_handle = app_handle.clone();
        move |item| ChangeForFrontend::from((project_id, item)).send(&app_handle)
    })?;
    WATCHERS.lock().insert(project_id, (tx, handle));
    Ok(())
}

fn watch_db_in_background(
    ctx: &mut CommandContext,
    send_event: impl Fn(ItemKind) -> anyhow::Result<()> + Send + Sync + 'static,
) -> anyhow::Result<(oneshot::Sender<()>, JoinHandle<()>)> {
    let db = ctx.db()?;
    let rx = db.poll_changes(
        ItemKind::Actions | ItemKind::Workflows | ItemKind::Assignments,
        Duration::from_millis(500),
    )?;

    let (cancel_tx, mut cancel_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        for item in rx {
            // Check for cancellation
            if cancel_rx.try_recv().is_ok() {
                tracing::info!("DB watcher cancelled");
                break;
            }
            match item {
                Ok(item) => {
                    tracing::debug!("Received item: {:?}", item);
                    send_event(item)
                        .unwrap_or_else(|e| tracing::error!("Error sending event: {:?}", e));
                }
                Err(e) => {
                    tracing::error!("Error receiving item: {:?}", e);
                    break;
                }
            }
        }
    });
    Ok((cancel_tx, handle))
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn stop_watching_db(project_id: ProjectId) -> anyhow::Result<(), Error> {
    if let Some((tx, handle)) = WATCHERS.lock().remove(&project_id) {
        let _ = tx.send(()); // signal cancellation
        handle.abort();
        tracing::info!("Stopped DB watcher for project: {:?}", project_id);
    }
    Ok(())
}
