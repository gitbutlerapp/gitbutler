//! A stream socket in the channel's data directory, only for this user, each connection to it
//! one conversation, as Neovim's server is:
//! <https://github.com/neovim/neovim/blob/864f788/src/nvim/msgpack_rpc/server.c>

use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

pub type Listener = UnixListener;
pub type Connection = UnixStream;

pub fn socket_path() -> io::Result<PathBuf> {
    but_path::app::socket_path(but_path::AppChannel::new()).map_err(io::Error::other)
}

/// Listen for the CLI and later launches, or `None` where an app already does.
pub fn listen() -> io::Result<Option<UnixListener>> {
    let path = socket_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        // Connecting says which: an app answers, or one that quit left its socket behind. The
        // app that answers sees a connection that asks nothing.
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => match UnixStream::connect(&path) {
            Ok(_) => return Ok(None),
            Err(err) if err.kind() == io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(&path)?;
                UnixListener::bind(&path)?
            }
            Err(err) => return Err(err),
        },
        Err(err) => return Err(err),
    };
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(Some(listener))
}

/// The next connection, once one comes.
pub fn accept(listener: &mut UnixListener) -> io::Result<UnixStream> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
}

/// `connection` twice, to read from one and write to the other.
pub fn split(connection: UnixStream) -> io::Result<(UnixStream, UnixStream)> {
    Ok((connection.try_clone()?, connection))
}
