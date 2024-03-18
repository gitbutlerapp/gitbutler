//! # gitbutler-pidlock
//! Provides a PID-based lockfile for use where a task must ensure
//! it's the only operation working on a resource, typically a filesystem
//! resource.
//!
//! The PID file first checks if the PID file exists, pulls the PID out,
//! and checks if the process is still running. If all of those checks pass,
//! a lock blocks creation until any of those things is no longer true.
//!
//! It then (over)writes the PID file with its own PID, returns a lock,
//! and when the lock is dropped, it removes the PID file.
//!
//! Note that the above checks are not atomic; there *is* a small chance for
//! a race condition. Please keep this in mind.
#![deny(missing_docs)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use ::polonius_the_crab::prelude::*;
use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus};

/// The error type returned by the lockfile types.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The lockfile failed to get the current process's PID.
    #[error("failed to get current process ID: {0}")]
    GetPid(String),
    /// The lockfile is being accessed by something else within
    /// this process.
    #[error("concurrent access to lockfile (same process)")]
    ConcurrentAccess,
    /// I/O error (generic, from [`std::io::Error`]).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Another process has locked the resource.
    #[error("resource is locked by another process: {0}")]
    ProcessLocked(u32),
}

/// Implements a PID lockfile to be used as a filesystem-based lock for a
/// resource, typically a filesystem resource.
///
/// See this crate's main documentation for information about the implementation.
pub struct PidLock {
    filepath: PathBuf,
    our_pid: Pid,
    poll_rate: Duration,
    access_mutex: Mutex<()>,
}

impl PidLock {
    /// Creates a new `PidLock` instance given a filepath at which the PID file
    /// should be stored.
    #[cold]
    pub fn new<P: AsRef<Path>>(filepath: P, poll_rate: Duration) -> Result<Self, Error> {
        let our_pid = sysinfo::get_current_pid().map_err(|s| Error::GetPid(s.to_string()))?;

        Ok(PidLock {
            filepath: filepath.as_ref().to_owned(),
            our_pid,
            access_mutex: Mutex::new(()),
            poll_rate,
        })
    }

    /// Returns the path at which this PID lockfile is stored.
    #[inline]
    #[cold]
    pub fn filepath(&self) -> &Path {
        &self.filepath
    }

    /// Attempts to acquire the lock, blocking until it can be acquired.
    pub fn try_lock(&mut self) -> Result<PidLockGuard, Error> {
        {
            let _access_lock = self
                .access_mutex
                .try_lock()
                .map_err(|_| Error::ConcurrentAccess)?;

            // Attempt to read the PID file.
            let locked_pid = match fs::read_to_string(&self.filepath) {
                Ok(s) => s.trim().parse::<Pid>().ok(),
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        None
                    } else {
                        return Err(e.into());
                    }
                }
            };

            // If the PID file exists, check if the process is still running.
            if let Some(locked_pid) = locked_pid {
                let mut sys = sysinfo::System::new();
                // We refresh with `::new()` here because we don't care about getting
                // any of the information about the process other than its status.
                sys.refresh_pids_specifics(&[locked_pid], ProcessRefreshKind::new());
                if sys
                    .process(locked_pid)
                    .map(|p| p.status() != ProcessStatus::Zombie)
                    .unwrap_or(false)
                {
                    return Err(Error::ProcessLocked(locked_pid.as_u32()));
                }
            }
        }

        // Otherwise, write our PID and return a lock.
        fs::write(&self.filepath, self.our_pid.to_string())?;
        Ok(PidLockGuard { lock: self })
    }

    /// Acquires the lock, blocking until it can be acquired.
    pub fn lock<'a>(&'a mut self) -> Result<PidLockGuard<'a>, Error> {
        let poll_rate = self.poll_rate;
        let mut this = self;
        loop {
            polonius!(|this| -> Result<PidLockGuard<'polonius>, Error> {
                match this.try_lock() {
                    Ok(guard) => polonius_return!(Ok(guard)),
                    Err(Error::ProcessLocked(_) | Error::ConcurrentAccess) => {
                        std::thread::sleep(poll_rate)
                    }
                    Err(e) => polonius_return!(Err(e)),
                }
            });
        }
    }
}

/// A guard returned by [`PidLock::try_lock`] and [`PidLock::lock`], which removes the PID file
/// when dropped.
///
/// **NOTE:** The removal is not guaranteed to work. If for some reason the removal fails, it will
/// be silently ignored.
pub struct PidLockGuard<'a> {
    lock: &'a mut PidLock,
}

impl Drop for PidLockGuard<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock.filepath);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_ok_lock_drop() {
        let td = tempfile::tempdir().unwrap();
        let mut lock = PidLock::new(
            td.path().join("test-ok-lock-drop.pid"),
            Duration::from_millis(100),
        )
        .unwrap();
        {
            let _guard = lock.try_lock().unwrap();
        }
        {
            let _guard = lock.try_lock().unwrap();
        }
        {
            let _guard = lock.try_lock().unwrap();
        }
    }

    #[test]
    fn test_err_process_lock() {
        let td = tempfile::tempdir().unwrap();
        let path = td.path().join("test-err-process-lock.pid");
        let mut lock = PidLock::new(&path, Duration::from_millis(100)).unwrap();
        let _guard = lock.try_lock().unwrap();
        let mut lock2 = PidLock::new(&path, Duration::from_millis(100)).unwrap();
        let guard2 = lock2.try_lock();
        match guard2 {
            Ok(_) => panic!("expected process locked error, got lock"),
            Err(Error::ProcessLocked(pid)) => {
                assert_eq!(pid, sysinfo::get_current_pid().unwrap().as_u32())
            }
            Err(e) => panic!("expected process locked error, got {:?}", e),
        }
    }

    #[test]
    fn test_err_process_locked_different_pid() {
        let td = tempfile::tempdir().unwrap();
        let path = td.path().join("test-err-lock-process-locked.pid");
        let mut lock = PidLock::new(&path, Duration::from_millis(50)).unwrap();
        let _guard = lock.try_lock().unwrap();

        // Write a fake PID to the file. In our case, we create a zombie process
        // that is suspended upon boot (either forcefully, via a loop, or by the kernel).
        let mut command = {
            #[cfg(unix)]
            unsafe {
                // On Unix, we do a sleep loop before exec is invoked.
                use std::{os::unix::process::CommandExt, process::Command};
                Command::new("sh")
                    .arg("-c")
                    .arg(":")
                    .pre_exec(|| {
                        std::thread::sleep(Duration::from_millis(1000));
                        Ok(())
                    })
                    .spawn()
                    .unwrap()
            }

            #[cfg(windows)]
            {
                // On Windows, we start the program suspended and never resume it.
                use std::{os::windows::process::CommandExt, process::Command};
                Command::new("echo")
                    .creation_flags(0x00000004) // CREATE_SUSPENDED
                    .spawn()
                    .unwrap()
            }
        };

        let fake_pid = command.id();
        ::std::fs::write(&path, fake_pid.to_string()).unwrap();

        let mut lock2 = PidLock::new(&path, Duration::from_millis(1000)).unwrap();
        let guard2 = lock2.try_lock();

        command.kill().unwrap();

        match guard2 {
            Ok(_) => panic!("expected process locked error, got lock"),
            Err(Error::ProcessLocked(pid)) => assert_eq!(pid, fake_pid),
            Err(e) => panic!("expected process locked error, got {:?}", e),
        }
    }

    #[test]
    fn test_err_process_lock_wait() {
        let joined = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let td = tempfile::tempdir().unwrap();
        let path = td.path().join("test-err-process-lock-wait.pid");
        let mut lock = PidLock::new(&path, Duration::from_millis(100)).unwrap();
        let guard = lock.try_lock().unwrap();

        let other_thread = std::thread::spawn({
            let joined = joined.clone();
            move || {
                let mut lock2 = PidLock::new(&path, Duration::from_millis(10)).unwrap();
                let guard2 = lock2.lock().unwrap();
                joined.store(true, std::sync::atomic::Ordering::SeqCst);
                drop(guard2);
            }
        });

        // Sleep for a while and then drop the guard.
        std::thread::sleep(Duration::from_millis(300));
        assert!(!joined.load(std::sync::atomic::Ordering::SeqCst));
        drop(guard);

        other_thread.join().unwrap();
        assert!(joined.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_ensure_remove_lock_file_on_guard_drop() {
        let td = tempfile::tempdir().unwrap();
        let path = td
            .path()
            .join("test-ensure-remove-lock-file-on-guard-drop.pid");
        let mut lock = PidLock::new(&path, Duration::from_millis(100)).unwrap();
        let guard = lock.try_lock().unwrap();
        assert!(path.exists());
        drop(guard);
        assert!(!path.exists());
    }
}
