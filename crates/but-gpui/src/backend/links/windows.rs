//! A named mutex says which launch came first, and a named pipe takes later launches' links.
//! As Zed's: <https://github.com/zed-industries/zed/blob/28d0a3547f16b9609b301b48ab363bfca649c864/crates/zed/src/zed/windows_only_instance.rs>

use std::io;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_BUSY,
    ERROR_PIPE_CONNECTED, GENERIC_WRITE, GetLastError, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING, PIPE_ACCESS_INBOUND,
    ReadFile, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_WAIT, WaitNamedPipeW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::HSTRING;

/// The pipe later launches connect to.
pub struct Listener(HANDLE);

// SAFETY: the handle is the pipe's, which only the thread that receives on it uses.
unsafe impl Send for Listener {}

/// Named after the data directory: pipe names are machine-wide, so this keeps users, channels
/// and test runs apart.
fn names() -> io::Result<(HSTRING, HSTRING)> {
    let dir = but_path::app_data_dir().map_err(io::Error::other)?;
    let id = (dir.to_string_lossy())
        .replace(['\\', '/', ':'], "-")
        .to_lowercase();
    let id = format!("gitbutler-links-{id}");
    Ok((
        HSTRING::from(format!(r"Local\{id}")),
        HSTRING::from(format!(r"\\.\pipe\{id}")),
    ))
}

/// Listen for later launches, or `None` where an app already does.
pub fn listen() -> io::Result<Option<Listener>> {
    let (mutex, pipe) = names()?;
    // SAFETY: `mutex` is a valid name, and the handle stays open until the app quits, so
    // later launches find it.
    unsafe { CreateMutexW(None, false, &mutex) }.map_err(io::Error::other)?;
    // SAFETY: reads this thread's last error, which `CreateMutexW` just set.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(None);
    }
    // SAFETY: `pipe` is a valid name, and the handle is checked before it's used.
    let pipe = unsafe {
        CreateNamedPipeW(
            &pipe,
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            0,
            64 * 1024,
            0,
            None,
        )
    };
    if pipe.is_invalid() {
        return Err(io::Error::last_os_error());
    }
    Ok(Some(Listener(pipe)))
}

pub fn send(message: &[u8]) -> io::Result<()> {
    let (_, name) = names()?;
    let pipe = loop {
        // SAFETY: `name` is a valid name, and the handle is closed below.
        match unsafe {
            CreateFileW(
                &name,
                GENERIC_WRITE.0,
                FILE_SHARE_MODE::default(),
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES::default(),
                None,
            )
        } {
            Ok(pipe) => break pipe,
            // Another launch is handing its links over: wait for the app to take ours.
            Err(err) if err.code() == ERROR_PIPE_BUSY.to_hresult() => {
                // SAFETY: `name` is a valid name.
                if !unsafe { WaitNamedPipeW(&name, 1000) }.as_bool() {
                    return Err(io::Error::other(err));
                }
            }
            Err(err) => return Err(io::Error::other(err)),
        }
    };
    let written = match message.is_empty() {
        true => Ok(()),
        // SAFETY: `pipe` is open for writing, and `message` outlives the call.
        false => unsafe { WriteFile(pipe, Some(message), None, None) },
    };
    // SAFETY: `pipe` is open, and not used after.
    unsafe { CloseHandle(pipe) }.ok();
    written.map_err(io::Error::other)
}

/// Hand `each` every message as it comes, until the pipe fails.
pub fn receive(listener: Listener, mut each: impl FnMut(Vec<u8>)) -> io::Result<()> {
    let pipe = listener.0;
    loop {
        // SAFETY: `pipe` is the listener's open pipe.
        if let Err(err) = unsafe { ConnectNamedPipe(pipe, None) }
            // A launch that connected before this waited is connected, or already gone with its
            // links written.
            && err.code() != ERROR_PIPE_CONNECTED.to_hresult()
            && err.code() != ERROR_NO_DATA.to_hresult()
        {
            return Err(io::Error::other(err));
        }
        let message = read_until_closed(pipe);
        // SAFETY: `pipe` is the listener's open pipe.
        unsafe { DisconnectNamedPipe(pipe) }.ok();
        match message {
            Ok(message) => each(message),
            Err(err) => tracing::warn!("couldn't read a later launch's links: {err}"),
        }
    }
}

/// What the connected launch writes, until it closes its end.
fn read_until_closed(pipe: HANDLE) -> io::Result<Vec<u8>> {
    let mut message = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let mut read = 0u32;
        // SAFETY: `pipe` is connected, and `buffer` and `read` outlive the call.
        match unsafe { ReadFile(pipe, Some(&mut buffer), Some(&raw mut read), None) } {
            Ok(()) if read == 0 => return Ok(message),
            Ok(()) => message.extend(buffer.iter().take(read as usize)),
            Err(err) if err.code() == ERROR_BROKEN_PIPE.to_hresult() => return Ok(message),
            Err(err) => return Err(io::Error::other(err)),
        }
    }
}
