use anyhow::Context as _;
use but_api::json::Error;
use tauri::{Emitter, EventTarget, WebviewWindow};
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(window), err(Debug))]
pub fn show_notification(
    window: WebviewWindow,
    id: String,
    title: String,
    body: String,
) -> Result<(), Error> {
    if window.is_focused().context("failed to read window focus")? {
        return Ok(());
    }
    show(window, id, &title, &body)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn show(window: WebviewWindow, id: String, title: &str, body: &str) -> anyhow::Result<()> {
    use mac_usernotifications::{Notification, block_on, request_auth};
    if !block_on(request_auth()).context("failed to request notification permission")? {
        return Ok(());
    }
    let handle = block_on(Notification::new().title(title).message(body).send())
        .context("failed to show notification")?;
    std::thread::spawn(move || match block_on(handle.response()) {
        Ok(response) if response.is_default_action() => focus_and_report_click(&window, id),
        Ok(_) => {}
        Err(err) => tracing::error!(error = ?err, "desktop notification failed"),
    });
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn show(window: WebviewWindow, id: String, title: &str, body: &str) -> anyhow::Result<()> {
    use notify_rust::{Notification, NotificationResponse};
    let mut notification = Notification::new();
    notification.summary(title).body(body);
    #[cfg(unix)]
    notification.action("default", "Open");
    #[cfg(windows)]
    if !tauri::is_dev() {
        notification.app_id(&tauri::Manager::config(&window).identifier);
    }
    let handle = notification.show().context("failed to show notification")?;
    std::thread::spawn(move || {
        let waited = handle.wait_for_response(|response: &NotificationResponse| {
            if matches!(response, NotificationResponse::Default) {
                focus_and_report_click(&window, id);
            }
        });
        if let Err(err) = waited {
            tracing::error!(error = ?err, "desktop notification failed");
        }
    });
    Ok(())
}

fn focus_and_report_click(window: &WebviewWindow, id: String) {
    let focused = window
        .unminimize()
        .and_then(|()| window.show())
        .and_then(|()| window.set_focus());
    if let Err(err) = focused {
        tracing::error!(error = ?err, "failed to focus window for notification click");
    }
    if let Err(err) = window.emit_to(
        EventTarget::webview_window(window.label()),
        "notification-click",
        id,
    ) {
        tracing::error!(error = ?err, "failed to emit notification click");
    }
}
