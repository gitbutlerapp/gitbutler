use anyhow::{Context, Result};
use tokio::net;

use crate::projects;

use super::connection;

const PTY_WS_ADDRESS: &str = "127.0.0.1:7703";

pub async fn start_server(projects_store: projects::Storage) -> Result<()> {
    let listener = net::TcpListener::bind(&PTY_WS_ADDRESS)
        .await
        .with_context(|| format!("failed to bind to {}", PTY_WS_ADDRESS))?;

    while let Ok((stream, _)) = listener.accept().await {
        let projects_store = projects_store.clone();
        tauri::async_runtime::spawn(async {
            connection::accept_connection(projects_store, stream)
                .await
                .with_context(|| format!("failed to accept connection"))
                .map_err(|e| log::error!("{:#}", e))
        });
    }

    Ok(())
}
