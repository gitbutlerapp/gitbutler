use std::env;
use std::io::{Read, Write};

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use mt_logger::*;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

const PTY_SERVER_ADDRESS: &str = "127.0.0.1:7703";
const TERM: &str = "xterm-256color";

#[derive(Deserialize, Debug)]
struct WindowSize {
    /// The number of lines of text
    pub rows: u16,
    /// The number of columns of text
    pub cols: u16,
    /// The width of a cell in pixels.  Note that some systems never
    /// fill this value and ignore it.
    pub pixel_width: u16,
    /// The height of a cell in pixels.  Note that some systems never
    /// fill this value and ignore it.
    pub pixel_height: u16,
}

async fn handle_client(stream: TcpStream) {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");
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
        .unwrap();

    let cmd = if cfg!(target_os = "windows") {
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
        let user_default_shell = env::var("SHELL").unwrap();

        mt_log!(Level::Info, "user_default_shell={}", user_default_shell);

        let user_scripts = &Value::Null;

        let scripts = json!({
          "cwd": "$(pwd)",
          "user_scripts": user_scripts
        });
        let scripts = scripts.to_string();
        let scripts_str = serde_json::to_string(&scripts).unwrap();

        mt_log!(Level::Info, "scripts={}", scripts_str);

        let prompt_command_scripts = format!(r#"echo -en "\033]0; [manter] "{}" \a""#, scripts_str);

        let mut cmd = CommandBuilder::new(user_default_shell);
        cmd.env("PROMPT_COMMAND", prompt_command_scripts);
        cmd.env("TERM", TERM);
        cmd.args(["-i"]);
        cmd
    };

    let mut pty_child_process = pty_pair.slave.spawn_command(cmd).unwrap();

    let mut pty_reader = pty_pair.master.try_clone_reader().unwrap();
    let mut pty_writer = pty_pair.master.take_writer().unwrap();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut buffer = BytesMut::with_capacity(1024);
            buffer.resize(1024, 0u8);
            loop {
                buffer[0] = 0u8;
                let mut tail = &mut buffer[1..];

                match pty_reader.read(&mut tail) {
                    Ok(0) => {
                        // EOF
                        mt_log!(Level::Info, "0 bytes read from pty. EOF.");
                        break;
                    }
                    Ok(n) => {
                        if n == 0 {
                            // this may be redundant because of Ok(0), but not sure
                            break;
                        }
                        let mut data_to_send = Vec::with_capacity(n + 1);
                        data_to_send.extend_from_slice(&buffer[..n + 1]);
                        let message = Message::Binary(data_to_send);
                        ws_sender.send(message).await.unwrap();
                    }
                    Err(e) => {
                        mt_log!(Level::Error, "Error reading from pty: {}", e);
                        mt_log!(Level::Error, "PTY child process may be closed.");
                        break;
                    }
                }
            }

            mt_log!(Level::Info, "PTY child process killed.");
        })
    });

    while let Some(message) = ws_receiver.next().await {
        let message = message.unwrap();
        match message {
            Message::Binary(msg) => {
                let msg_bytes = msg.as_slice();
                match msg_bytes[0] {
                    0 => {
                        if msg_bytes.len().gt(&0) {
                            pty_writer.write_all(&msg_bytes[1..]).unwrap();
                        }
                    }
                    1 => {
                        let resize_msg: WindowSize =
                            serde_json::from_slice(&msg_bytes[1..]).unwrap();
                        let pty_size = PtySize {
                            rows: resize_msg.rows,
                            cols: resize_msg.cols,
                            pixel_width: resize_msg.pixel_width,
                            pixel_height: resize_msg.pixel_height,
                        };
                        pty_pair.master.resize(pty_size).unwrap();
                    }
                    _ => mt_log!(Level::Error, "Unknown command {}", msg_bytes[0]),
                }
            }
            Message::Close(_) => {
                mt_log!(Level::Info, "Closing the websocket connection...");

                mt_log!(Level::Info, "Killing PTY child process...");
                pty_child_process.kill().unwrap();

                mt_log!(Level::Info, "Breakes the loop. This will terminate the ws socket thread and the ws will close");
                break;
            }
            _ => mt_log!(Level::Error, "Unknown received data type"),
        }
    }

    mt_log!(
        Level::Info,
        "The Websocket was closed and the thread for WS listening will end soon."
    );
}

pub async fn pty_serve() {
    let listener = TcpListener::bind(PTY_SERVER_ADDRESS)
        .await
        .expect("Can't listen");

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        mt_log!(Level::Info, "Peer address: {}", peer);

        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                handle_client(stream).await;
            });
        });
    }
}
