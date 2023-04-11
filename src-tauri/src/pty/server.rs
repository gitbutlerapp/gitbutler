use super::connection;
use crate::projects;
use anyhow::{Context, Result};
use tokio::net;

const PTY_WS_ADDRESS: &str = "127.0.0.1:7703";

pub async fn start_server(projects_store: projects::Storage) -> Result<()> {
    let listener = net::TcpListener::bind(&PTY_WS_ADDRESS)
        .await
        .with_context(|| format!("failed to bind to {}", PTY_WS_ADDRESS))?;

    log::info!("pty-ws: listening on {}", PTY_WS_ADDRESS);

    while let Ok((stream, _)) = listener.accept().await {
        let projects_store = projects_store.clone();
        tauri::async_runtime::spawn(async {
            if let Err(e) = connection::accept_connection(projects_store, stream).await {
                log::error!("pty-ws: failed to accept connection {:#}", e);
            }
        });
    }

    log::info!("pty-ws: server stopped");

    Ok(())
}
