use std::fs::File;

use rustix::fs::FlockOperation;
use tracing::instrument;

pub struct FileLock<'a>(&'a File);

impl<'a> FileLock<'a> {
    #[instrument(level = "debug")]
    pub fn lock(file: &'a File) -> FileLock {
        // Create lockfile, or open pre-existing one
        // If the lock was already held, wait for it to be released
        rustix::fs::flock(file, FlockOperation::LockExclusive).expect("failed to lock lockfile");
        Self(file)
    }
}

impl Drop for FileLock<'_> {
    fn drop(&mut self) {
        // Unblock any processes that tried to acquire the lock while we held it.
        // They're responsible for creating and locking a new lockfile, since we
        // just deleted this one.
        _ = rustix::fs::flock(self.0, FlockOperation::Unlock);
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::thread;
    use std::time::Duration;
    use std::{cmp::max, fs};

    use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

    use super::*;

    #[test]
    fn lock_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        assert!(!lock_path.exists());
        let lock_file = fs::File::create(lock_path.clone()).unwrap();
        {
            let _lock = FileLock::lock(&lock_file);
            assert!(lock_path.exists());
        }
        assert!(lock_path.exists());
    }

    #[test]
    fn lock_concurrent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_path = temp_dir.path().join("test");
        let lock_path = temp_dir.path().join("test.lock");
        let mut data_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(data_path.clone())
            .unwrap();
        data_file.write_u32::<LittleEndian>(0).unwrap();
        let num_threads = max(num_cpus::get(), 4);
        thread::scope(|s| {
            for _ in 0..num_threads {
                let data_path = data_path.clone();
                let lock_file = fs::File::create(lock_path.clone()).unwrap();
                s.spawn(move || {
                    let _lock = FileLock::lock(&lock_file);
                    let mut data_file = OpenOptions::new()
                        .read(true)
                        .open(data_path.clone())
                        .unwrap();
                    let value = data_file.read_u32::<LittleEndian>().unwrap();
                    thread::sleep(Duration::from_millis(1));
                    let mut data_file = OpenOptions::new().write(true).open(data_path).unwrap();
                    data_file.write_u32::<LittleEndian>(value + 1).unwrap();
                });
            }
        });
        let mut data_file = OpenOptions::new().read(true).open(data_path).unwrap();
        let value = data_file.read_u32::<LittleEndian>().unwrap();
        assert_eq!(value, num_threads as u32);
    }
}
