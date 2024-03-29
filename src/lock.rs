use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Dir {
    inner: Arc<Inner>,
}

impl Dir {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
        Inner::new(path).map(Arc::new).map(|inner| Self { inner })
    }

    pub fn batch<R>(
        &self,
        action: impl FnOnce(&std::path::Path) -> R,
    ) -> Result<R, std::io::Error> {
        self.inner.batch(action)
    }
}

#[derive(Debug)]
struct Inner {
    path: std::path::PathBuf,
    flock: Mutex<fslock::LockFile>,
}

impl Inner {
    fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        } else if !path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} is not a directory", path.display()),
            ));
        }
        let flock = fslock::LockFile::open(&path.with_extension("lock")).map(Mutex::new)?;
        Ok(Self { path, flock })
    }

    fn batch<R>(&self, action: impl FnOnce(&std::path::Path) -> R) -> Result<R, std::io::Error> {
        let mut flock = self.flock.lock().unwrap();

        flock.lock()?;
        let result = action(&self.path);
        flock.unlock()?;

        Ok(result)
    }
}
