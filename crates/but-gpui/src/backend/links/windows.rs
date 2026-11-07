//! A named pipe, named after the data directory, with an instance for each connection, each one
//! conversation. Making the first instance says which launch came first, as Zed's mutex does:
//! <https://github.com/zed-industries/zed/blob/28d0a3547f16b9609b301b48ab363bfca649c864/crates/zed/src/zed/windows_only_instance.rs>

use std::fs::File;
use std::io;
use std::os::windows::io::FromRawHandle as _;
use std::path::PathBuf;

use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_NO_DATA, ERROR_PIPE_CONNECTED, HANDLE,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::core::HSTRING;

pub type Connection = File;

/// The pipe, by name, and its instance waiting for the next connection.
pub struct Listener {
    name: HSTRING,
    next: HANDLE,
}

// SAFETY: the handle is the waiting instance's, which only the thread accepting on it uses.
unsafe impl Send for Listener {}

pub fn socket_path() -> io::Result<PathBuf> {
    but_path::app::socket_path(but_path::AppChannel::new()).map_err(io::Error::other)
}

/// An instance of the pipe `name`, the first one if `first`, which fails where another
/// process made the pipe already.
fn instance(name: &HSTRING, first: bool) -> io::Result<HANDLE> {
    let mut mode = PIPE_ACCESS_DUPLEX;
    if first {
        mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    // SAFETY: `name` is a valid name, and the handle is checked before it's used.
    let pipe = unsafe {
        CreateNamedPipeW(
            name,
            mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            None,
        )
    };
    if pipe.is_invalid() {
        return Err(io::Error::last_os_error());
    }
    Ok(pipe)
}

/// Listen for the CLI and later launches, or `None` where an app already does.
pub fn listen() -> io::Result<Option<Listener>> {
    let name = HSTRING::from(socket_path()?.as_os_str());
    match instance(&name, true) {
        Ok(next) => Ok(Some(Listener { name, next })),
        Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED.0 as i32) => Ok(None),
        Err(err) => Err(err),
    }
}

/// The next connection, once one comes.
pub fn accept(listener: &mut Listener) -> io::Result<File> {
    let pipe = listener.next;
    // SAFETY: `pipe` is the listener's instance, not connected yet.
    if let Err(err) = unsafe { ConnectNamedPipe(pipe, None) }
        // A client that connected before this waited is connected, or already gone with what
        // it wrote, as a launch handing over its links is.
        && err.code() != ERROR_PIPE_CONNECTED.to_hresult()
        && err.code() != ERROR_NO_DATA.to_hresult()
    {
        return Err(io::Error::other(err));
    }
    // Another instance before this one's handed on, so the pipe always has one a client can
    // find, if busy for a moment.
    listener.next = instance(&listener.name, false)?;
    // SAFETY: the connected instance is the file's alone from here, which closes it.
    Ok(unsafe { File::from_raw_handle(pipe.0) })
}

/// `connection` twice, to read from one and write to the other. A conversation reads a request
/// and then writes its answer, never both at once, which a pipe opened without overlapping I/O
/// would make wait on each other.
pub fn split(connection: File) -> io::Result<(File, File)> {
    Ok((connection.try_clone()?, connection))
}
