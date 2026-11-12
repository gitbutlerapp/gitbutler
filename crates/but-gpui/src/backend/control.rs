use std::path::Path;

use anyhow::{Context as _, Result, ensure};

/// Only the user can reach the socket. A stale one no app listens on is replaced.
pub fn bind(path: &Path) -> Result<std::os::unix::net::UnixListener> {
    use std::os::unix::fs::FileTypeExt as _;
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
    // `bind` takes the socket's permissions from the umask, so it's private from the start.
    // A file another thread makes meanwhile comes out private too. The lock keeps two binds
    // from putting back each other's mask.
    static UMASK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _umask = UMASK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: `umask` only swaps the process's file mode mask.
    let umask = unsafe { libc::umask(0o077) };
    let listener = UnixListener::bind(path);
    // SAFETY: as above, putting the mask back.
    unsafe { libc::umask(umask) };
    listener.context("binding")
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
    fn only_the_user_can_reach_the_socket() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gpui.sock");
        let _listener = bind(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "no one else gets in: {mode:o}");
    }

    #[test]
    fn a_socket_nothing_listens_on_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gpui.sock");
        drop(bind(&path).unwrap());

        assert!(bind(&path).is_ok(), "the socket left behind gives way");
    }
}
