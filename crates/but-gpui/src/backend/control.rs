//! Binding the control socket.

use std::path::Path;

use anyhow::{Context as _, Result, ensure};

/// A socket at `path` only the user can reach, in the place of one no app listens on anymore.
#[cfg(unix)]
pub fn bind(path: &Path) -> Result<std::os::unix::net::UnixListener> {
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::{UnixListener, UnixStream};

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).context("making its folder")?;
    }
    if path.exists() {
        ensure!(
            UnixStream::connect(path).is_err(),
            "another app listens there"
        );
        std::fs::remove_file(path).context("removing the socket left behind")?;
    }
    let listener = UnixListener::bind(path).context("binding")?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}
