use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, Name, NameType, ToFsName, ToNsName,
};

const GB_SOCKET: &str = "gitbutler.sock";

fn socket_name(name: &str) -> anyhow::Result<Name<'_>> {
    if GenericNamespaced::is_supported() {
        name.to_ns_name::<GenericNamespaced>()
    } else {
        format!("/tmp/{}", name).to_fs_name::<GenericFilePath>()
    }
    .map_err(Into::into)
}

pub fn send_reload_signal(project_id: String) -> anyhow::Result<()> {
    use {
        interprocess::local_socket::{Stream, prelude::*},
        std::io::{BufReader, prelude::*},
    };
    let socket = socket_name(GB_SOCKET)?;
    let conn = Stream::connect(socket)?;
    let mut conn = BufReader::new(conn);
    conn.get_mut().write_all(project_id.as_bytes())?;
    Ok(())
}

pub fn listen_for_reload_signal(
    signal_received: impl Fn(String) -> anyhow::Result<()> + Send + Sync + 'static,
) -> anyhow::Result<()> {
    use {
        interprocess::local_socket::{ListenerOptions, tokio::prelude::*},
        tokio::io::{AsyncBufReadExt, BufReader},
    };

    let file_path = format!("/tmp/{}", GB_SOCKET);
    std::fs::remove_file(&file_path).ok(); // clean up any existing socket file

    let socket = socket_name(GB_SOCKET)?;
    let opts = ListenerOptions::new().name(socket);
    let listener = opts.create_tokio()?;

    tokio::spawn(async move {
        let mut buffer = String::with_capacity(128);
        loop {
            if let Ok(conn) = listener.accept().await {
                let mut conn = BufReader::new(conn);
                let _ = conn.read_line(&mut buffer).await;
                print!("{buffer}");
                let _ = signal_received(buffer.trim().to_string());
                buffer.clear();
            }
        }
    });
    Ok(())
}
