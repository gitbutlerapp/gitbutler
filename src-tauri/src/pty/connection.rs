use anyhow::Result;
use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;
use tokio::net;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

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

pub async fn accept_connection(stream: net::TcpStream) -> Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let pty_system = native_pty_system();
    let pty_pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        // Not all systems support pixel_width, pixel_height,
        // but it is good practice to set it to something
        // that matches the size of the selected font.  That
        // is more complex than can be shown here in this
        // brief example though!
        pixel_width: 0,
        pixel_height: 0,
    })?;

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
        let user_default_shell = env::var("SHELL")?;

        log::info!("user_default_shell={}", user_default_shell);

        let user_scripts = &Value::Null;

        let scripts = json!({
          "cwd": "$(pwd)",
          "user_scripts": user_scripts
        });
        let scripts = scripts.to_string();
        let scripts_str = serde_json::to_string(&scripts)?;

        log::info!("scripts={}", scripts_str);

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

    // set to cwd
    tauri::async_runtime::spawn(async move {
        let mut buffer = BytesMut::with_capacity(1024);
        buffer.resize(1024, 0u8);
        loop {
            buffer[0] = 0u8;
            let mut tail = &mut buffer[1..];

            match pty_reader.read(&mut tail) {
                Ok(0) => {
                    // EOF
                    log::info!("0 bytes read from pty. EOF.");
                    break;
                }
                Ok(n) => {
                    if n == 0 {
                        // this may be redundant because of Ok(0), but not sure
                        break;
                    }
                    let mut data_to_send = Vec::with_capacity(n + 1);
                    data_to_send.extend_from_slice(&buffer[..n + 1]);
                    record_data(&data_to_send);
                    let message = Message::Binary(data_to_send);
                    ws_sender.send(message).await.unwrap();
                }
                Err(e) => {
                    log::info!("Error reading from pty: {}", e);
                    log::info!("PTY child process may be closed.");
                    break;
                }
            }
        }

        log::info!("PTY child process killed.");
    });

    while let Some(message) = ws_receiver.next().await {
        let message = message.unwrap();
        match message {
            Message::Binary(msg) => {
                let msg_bytes = msg.as_slice();
                match msg_bytes[0] {
                    0 => {
                        if msg_bytes.len().gt(&0) {
                            record_data(&msg);
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
                    2 => {
                        // takes the directory we should be recording data to
                        if msg_bytes.len().gt(&0) {
                            // convert bytes to string
                            let command = String::from_utf8_lossy(&msg_bytes[1..]);
                            let project_path = PathBuf::from(command.as_ref());
                        }
                    }
                    _ => log::error!("Unknown command {}", msg_bytes[0]),
                }
            }
            Message::Close(_) => {
                log::info!("Closing the websocket connection...");

                log::info!("Killing PTY child process...");
                pty_child_process.kill().unwrap();

                log::info!("Breakes the loop. This will terminate the ws socket thread and the ws will close");
                break;
            }
            _ => log::error!("Unknown received data type"),
        }
    }

    log::info!("The Websocket was closed and the thread for WS listening will end soon.");
    Ok(())
}

// this sort of works, but it's not how we want to do it
// it just appends the data from every pty to the same file
// what we want to do is set the directory to record to, but since
// the reader is in a spawe thread, it's difficult to pass the directory to it
// I also can't seem to send data to the pty on opening a new one, so I can't
// easily initialize the cwd, which is where we want to write this data (under .git)
// HELP
fn record_data(_data: &Vec<u8>) {
    /*
    // A little too aggressive:
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open("data.txt")
        .unwrap();
    file.write_all(data).unwrap();
    */
}
