//! A datagram socket in the channel's data directory, only for this user. As Zed's:
//! <https://github.com/zed-industries/zed/blob/28d0a3547f16b9609b301b48ab363bfca649c864/crates/zed/src/zed/open_listener.rs>

use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

pub type Listener = UnixDatagram;

/// Room for a launch's message, links to the longest paths included.
const MESSAGE_SIZE: usize = 64 * 1024;

pub fn socket_path() -> io::Result<PathBuf> {
    let dir = but_path::app_data_dir().map_err(io::Error::other)?;
    Ok(dir.join("gpui-links.sock"))
}

/// Listen for later launches, or `None` where an app already does.
pub fn listen() -> io::Result<Option<UnixDatagram>> {
    let path = socket_path()?;
    match UnixDatagram::unbound()?.connect(&path) {
        Ok(()) => return Ok(None),
        // An app that quit left its socket behind.
        Err(err) if err.kind() == io::ErrorKind::ConnectionRefused => {
            std::fs::remove_file(&path)?;
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let listener = UnixDatagram::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(Some(listener))
}

pub fn send(message: &[u8]) -> io::Result<()> {
    UnixDatagram::unbound()?.send_to(message, socket_path()?)?;
    Ok(())
}

/// Hand `each` every message as it comes, until the socket fails.
pub fn receive(listener: UnixDatagram, mut each: impl FnMut(Vec<u8>)) -> io::Result<()> {
    let mut buffer = vec![0; MESSAGE_SIZE];
    loop {
        match listener.recv(&mut buffer) {
            Ok(len) => each(buffer.iter().take(len).copied().collect()),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
}
