mod checkout;

mod utils {
    /// Using the `repo` `HEAD` commit, build a new commit based on its tree with `edit` and `message`, and return the `(current_commit, new_commit)`.
    pub fn build_commit<'repo>(
        repo: &'repo gix::Repository,
        mut edit: impl FnMut(&mut gix::object::tree::Editor) -> anyhow::Result<()>,
        message: &str,
    ) -> anyhow::Result<(gix::Commit<'repo>, gix::Commit<'repo>)> {
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
        Ok((head_commit, repo.find_commit(new_commit_id)?))
    }
}
