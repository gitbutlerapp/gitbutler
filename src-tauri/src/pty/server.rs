use crate::pty::connection;
use anyhow::{Context, Result};
use tokio::net;

const PTY_WS_ADDRESS: &str = "127.0.0.1:7703";

pub async fn start_server() -> Result<()> {
    let listener = net::TcpListener::bind(&PTY_WS_ADDRESS)
        .await
        .with_context(|| format!("failed to bind to {}", PTY_WS_ADDRESS))?;

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr()?;
        log::info!("peer address: {}", peer);

        tauri::async_runtime::spawn(connection::accept_connection(stream));
    }

    Ok(())
}
