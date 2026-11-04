mod checkout;
#[cfg(unix)]
mod checkout_vs_git;
#[cfg(unix)]
mod checkout_with_override;

mod utils {
    /// Using the `repo` `HEAD` commit, build a new commit based on its tree with `edit` and `message`, and return the new commit.
    pub fn build_commit<'repo>(
        repo: &'repo gix::Repository,
        mut edit: impl FnMut(&mut gix::object::tree::Editor) -> anyhow::Result<()>,
        message: &str,
    ) -> anyhow::Result<gix::Commit<'repo>> {
        let head_commit = repo.head_commit()?;

        repo.write_blob([])?;
        let mut editor = head_commit.tree()?.edit()?;
        edit(&mut editor)?;

        let new_commit_id = repo
            .write_object(gix::objs::Commit {
                tree: editor.write()?.detach(),
                parents: [head_commit.id].into(),
                message: message.into(),
                ..head_commit.decode()?.to_owned()?
            })?
            .detach();
        Ok(repo.find_commit(new_commit_id)?)
    }

    /// Like [`build_commit()`], writing each `(path, Some(content))` as a file and removing each
    /// `(path, None)`, and return the new commit's id.
    pub fn commit_files(
        repo: &gix::Repository,
        message: &str,
        files: &[(&str, Option<&str>)],
    ) -> anyhow::Result<gix::ObjectId> {
        let commit = build_commit(
            repo,
            |tree| {
                for (path, content) in files {
                    match content {
                        Some(content) => {
                            let blob_id = repo.write_blob(content)?;
                            tree.upsert(*path, gix::object::tree::EntryKind::Blob, blob_id)?;
                        }
                        None => {
                            tree.remove(*path)?;
                        }
                    }
                }
                Ok(())
            },
            message,
        )?;
        Ok(commit.id)
    }
}
