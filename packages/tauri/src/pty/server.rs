use anyhow::{Context, Result};
use tokio::{net, task};

use crate::app;

use super::connection;

pub async fn start_server(port: usize, app: app::App) -> Result<()> {
    let pty_ws_address = format!("127.0.0.1:{}", port);
    let listener = net::TcpListener::bind(&pty_ws_address)
        .await
        .with_context(|| format!("failed to bind to {}", pty_ws_address))?;

    tracing::info!("pty-ws: listening on {}", pty_ws_address);

    while let Ok((stream, _)) = listener.accept().await {
        let app_clone = app.clone();
        task::Builder::new().name("pty-connection").spawn(async {
            if let Err(e) = connection::accept_connection(app_clone, stream).await {
                tracing::error!("pty-ws: failed to accept connection {:#}", e);
            }
        })?;
    }

    tracing::info!("pty-ws: server stopped");

    Ok(())
}
