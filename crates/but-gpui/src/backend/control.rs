use std::path::Path;

use anyhow::{Context as _, Result, ensure};

/// Only the user can reach the socket. A stale one no app listens on is replaced.
pub fn bind(path: &Path) -> Result<std::os::unix::net::UnixListener> {
    use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};
    use std::os::unix::net::{UnixListener, UnixStream};

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).context("making its folder")?;
    }
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        ensure!(
            metadata.file_type().is_socket(),
            "something that isn't a socket is there"
        );
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

#[cfg(test)]
mod tests {
    use super::bind;

    #[test]
    fn a_file_in_the_sockets_place_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gpui.sock");
        std::fs::write(&path, "not a socket").unwrap();

        assert!(bind(&path).is_err(), "it isn't a socket left behind");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "not a socket",
            "and it's left as it was"
        );
    }

    #[test]
    fn a_socket_nothing_listens_on_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gpui.sock");
        drop(bind(&path).unwrap());

        assert!(bind(&path).is_ok(), "the socket left behind gives way");
    }
}
