use bytes::BytesMut;
use std::{
    env,
    io::{Read, Write},
};

use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::net;
use tokio_tungstenite::{
    self,
    tungstenite::handshake::server::{Request, Response},
};

use crate::app;

use super::recorder;

const TERM: &str = "xterm-256color";

pub async fn accept_connection(app: app::App, stream: net::TcpStream) -> Result<()> {
    let mut project = None;
    let copy_uri_callback = |req: &Request, response: Response| {
        let path = req.uri().path().to_string();
        if let Some(project_id) = path.split('/').last() {
            project = match app.get_project(project_id) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("failed to get project: {}", e);
                    None
                }
            };
        }
        Ok(response)
    };

    let mut ws_stream = tokio_tungstenite::accept_hdr_async(stream, copy_uri_callback)
        .await
        .with_context(|| "failed to accept connection".to_string())?;

    if project.is_none() {
        ws_stream
            .close(None)
            .await
            .with_context(|| "failed to close connection".to_string())?;
        return Ok(());
    }
    let project = project.unwrap();

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let pty_system = native_pty_system();

    let pty_pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            // Not all systems support pixel_width, pixel_height,
            // but it is good practice to set it to something
            // that matches the size of the selected font.  That
            // is more complex than can be shown here in this
            // brief example though!
            pixel_width: 0,
            pixel_height: 0,
        })
        .with_context(|| "failed to open pty".to_string())?;

    let mut cmd = if cfg!(target_os = "windows") {
        // CommandBuilder::new(r"powershell")
        // CommandBuilder::new(r"C:\Program Files\Git\bin\bash.exe")
        // CommandBuilder::new(r"ubuntu.exe") // if WSL is active
        // on UI the user should have the option to choose

        let mut cmd = CommandBuilder::new(r"cmd");

        // this is needed only for cmd.exe
        // because the prompt does not have an empty space at the end
        // the prompt should be sepratared from the command being typed, for command parsing
        cmd.env("PROMPT", "$P$G ");

        cmd
    } else {
        let user_default_shell = env::var("SHELL")?;
        let mut cmd = CommandBuilder::new(user_default_shell);
        cmd.env("TERM", TERM);
        cmd.args(["-i"]);
        cmd
    };

    // set to project path
    cmd.cwd(project.path.clone());

    let mut pty_child_process = pty_pair.slave.spawn_command(cmd)?;

    let mut pty_reader = pty_pair.master.try_clone_reader()?;
    let mut pty_writer = pty_pair.master.take_writer()?;

    let shared_project_id = project.id.clone();
    let shared_app = app.clone();
    // it's important to spawn a new thread for the pty reader
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut buffer = BytesMut::with_capacity(1024);

            buffer.resize(1024, 0u8);
            loop {
                buffer[0] = 0u8;
                let tail = &mut buffer[1..];

                match pty_reader.read(tail) {
                    Ok(0) => {
                        // EOF
                        tracing::info!("0 bytes read from pty. EOF.");
                        if let Err(e) = ws_sender
                            .send(tokio_tungstenite::tungstenite::Message::Close(None))
                            .await
                        {
                            tracing::error!(
                                "{}: error sending data to websocket: {:#}",
                                shared_project_id,
                                e
                            );
                        }
                        break;
                    }
                    Ok(n) => {
                        let data = &buffer[..n + 1];
                        if let Err(e) = ws_sender
                            .send(tokio_tungstenite::tungstenite::Message::Binary(
                                data.to_vec(),
                            ))
                            .await
                        {
                            tracing::error!(
                                "{}: error sending data to websocket: {:#}",
                                shared_project_id,
                                e
                            );
                        }

                        if let Err(e) =
                            shared_app.record_pty(&shared_project_id, recorder::Type::Output, data)
                        {
                            tracing::error!("{}: error recording data: {:#}", shared_project_id, e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading from pty: {:#}", e);
                        break;
                    }
                }
            }

            tracing::info!("PTY child process killed.");
        });
    });

    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(tokio_tungstenite::tungstenite::Message::Binary(msg)) => {
                let msg_bytes = msg.as_slice();
                match (msg_bytes[0], msg_bytes[1..].to_vec()) {
                    (0, data) => {
                        if msg_bytes.len().gt(&0) {
                            pty_writer.write_all(&data)?;
                            if let Err(e) =
                                app.record_pty(&project.id, recorder::Type::Input, &data.to_vec())
                            {
                                tracing::error!("{}: error recording data: {:#}", &project.id, e);
                            }
                        }
                    }
                    (1, data) => {
                        let pty_size: PtySize = serde_json::from_slice(&data)?;
                        pty_pair.master.resize(pty_size)?;
                    }
                    (code, _) => tracing::error!("Unknown command {}", code),
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                tracing::info!("Closing the websocket connection...");

                tracing::info!("Killing PTY child process...");
                pty_child_process
                    .kill()
                    .with_context(|| "failed to kill pty child process".to_string())?;
                break;
            }
            Ok(_) => tracing::error!("Unknown received data type"),
            Err(e) => {
                tracing::error!("Error receiving data: {}", e);
                break;
            }
        }
    }
    Ok(())
}
