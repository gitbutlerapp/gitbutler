use gix::objs::Write;

/// Utilities for various `gix` types.
pub trait ObjectStorageExt {
    /// Write all in-memory objects into the object database of `repo`.
    fn persist(&self, repo: &gix::Repository) -> anyhow::Result<()>;
}

impl ObjectStorageExt for gix::odb::memory::Storage {
    fn persist(&self, repo: &gix::Repository) -> anyhow::Result<()> {
        // Each write first checks if the object exists, and a new object never does.
        // By default every such miss lists the pack directory again, which takes
        // minutes with hundreds of packs. Not looking for new packs is safe here,
        // as the worst case is a loose copy of an object that was packed meanwhile.
        let mut repo = repo.clone();
        repo.objects.refresh = gix::odb::store::RefreshMode::Never;
        for (kind, data) in self.values() {
            repo.write_buf(*kind, data)?;
        }
        Ok(())
    }
}
