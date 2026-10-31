use gix::objs::Write;

/// Utilities for various `gix` types.
pub trait ObjectStorageExt {
    /// Write all in-memory objects into the object database of `repo`.
    fn persist(&self, repo: &gix::Repository) -> anyhow::Result<()>;
}

impl ObjectStorageExt for gix::odb::memory::Storage {
    fn persist(&self, repo: &gix::Repository) -> anyhow::Result<()> {
        for (kind, data) in self.values() {
            repo.write_buf(*kind, data)?;
        }
        Ok(())
    }
}
