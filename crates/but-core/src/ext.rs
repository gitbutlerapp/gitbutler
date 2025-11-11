use gix::objs::Write;

/// Utilities for various `gix` types.
pub trait ObjectStorageExt {
    /// Write all in-memory objects into the given writer.
    fn persist(&self, out: impl gix::objs::Write) -> anyhow::Result<()>;
}

impl ObjectStorageExt for gix::odb::memory::Storage {
    fn persist(&self, out: impl Write) -> anyhow::Result<()> {
        for (kind, data) in self.values() {
            out.write_buf(*kind, data)
                .map_err(anyhow::Error::from_boxed)?;
        }
        Ok(())
    }
}
