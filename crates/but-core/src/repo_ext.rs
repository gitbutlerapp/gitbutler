use anyhow::Context as _;
use but_error::Code;
use gix::prelude::ObjectIdExt;

use crate::{GitConfigSettings, commit::TreeKind};

/// Easy access of settings relevant to GitButler for retrieval and storage in Git settings.
pub trait RepositoryExt {
    /// Returns a bundle of settings by querying the git configuration itself, assuring fresh data is loaded.
    fn git_settings(&self) -> anyhow::Result<GitConfigSettings>;
    /// Set all fields in `config` that are not `None` to disk into local repository configuration, or none of them.
    fn set_git_settings(&self, config: &GitConfigSettings) -> anyhow::Result<()>;
    /// Return all signatures that would be needed to perform a commit as configured in Git: `(author, committer)`.
    fn commit_signatures(&self) -> anyhow::Result<(gix::actor::Signature, gix::actor::Signature)>;
    /// Return labels that would be written into the conflict markers when merging blobs.
    fn default_merge_labels(&self) -> gix::merge::blob::builtin_driver::text::Labels<'static>;

    /// Return the configuration freshly loaded from `.git/config` so that it can be changed in memory,
    /// and possibly written back with [Self::write_local_config()].
    fn local_common_config_for_editing(&self) -> anyhow::Result<gix::config::File<'static>>;
    /// Write the given `local_config` to `.git/config` of the common repository.
    /// Note that we never write linked worktree-local configuration.
    fn write_local_common_config(&self, local_config: &gix::config::File) -> anyhow::Result<()>;
    /// Cherry-pick the changes in the tree of `to_rebase_commit_id` onto `new_base_commit_id`.
    /// This method deals with the presence of conflicting commits to select the correct trees
    /// for the cheery-pick merge.
    /// Use `merge_options` to control how the underlying merge should be performed. This is useful
    /// to either make it always work, or to accept merge conflicts.
    /// Return the cherry-picked tree only, leaving the caller with embedding it into a new commit.
    fn cherry_pick_commits_to_tree(
        &self,
        new_base_commit_id: gix::ObjectId,
        to_rebase_commit_id: gix::ObjectId,
        merge_options: gix::merge::tree::Options,
    ) -> anyhow::Result<gix::merge::tree::Outcome<'_>>;
}

impl RepositoryExt for gix::Repository {
    fn cherry_pick_commits_to_tree(
        &self,
        new_base_commit_id: gix::ObjectId,
        to_rebase_commit_id: gix::ObjectId,
        merge_options: gix::merge::tree::Options,
    ) -> anyhow::Result<gix::merge::tree::Outcome<'_>> {
        // TODO: more tests for the handling of conlicting commits in particular
        let to_rebase_commit = crate::Commit::from_id(to_rebase_commit_id.attach(self))?;
        // If the commit we are picking is conflicted then we want to use the
        // original base that was used when it was first cherry-picked.
        //
        // If it is not conflicted, then we use the first parent as the base.
        let base = if to_rebase_commit.is_conflicted() {
            match to_rebase_commit.inner.parents.first() {
                None => gix::ObjectId::empty_tree(self.object_hash()),
                Some(parent_commit) => crate::Commit::from_id(parent_commit.attach(self))?
                    .tree_id_or_auto_resolution()?
                    .detach(),
            }
        } else {
            to_rebase_commit.tree_id_or_kind(TreeKind::Base)?.detach()
        };
        let ours = crate::Commit::from_id(new_base_commit_id.attach(self))?
            .tree_id_or_auto_resolution()?;
        let theirs = to_rebase_commit.tree_id_or_kind(TreeKind::Theirs)?;

        self.merge_trees(
            base,   /* the tree of the parent of the commit to cherry-pick */
            ours,   /* the new base to cherry-pick onto */
            theirs, /* the tree of the commit to cherry-pick */
            self.default_merge_labels(),
            merge_options,
        )
        .context("failed to merge trees for cherry pick")
    }

    fn default_merge_labels(&self) -> gix::merge::blob::builtin_driver::text::Labels<'static> {
        gix::merge::blob::builtin_driver::text::Labels {
            ancestor: Some("base".into()),
            current: Some("ours".into()),
            other: Some("theirs".into()),
        }
    }

    fn commit_signatures(&self) -> anyhow::Result<(gix::actor::Signature, gix::actor::Signature)> {
        let repo = gix::open(self.path())?;

        let author = repo
            .author()
            .transpose()?
            .context("No author is configured in Git")
            .context(Code::AuthorMissing)?;

        let commit_as_gitbutler = self
            .config_snapshot()
            .boolean("gitbutler.gitbutlerCommitter")
            .unwrap_or_default();
        let committer = if commit_as_gitbutler {
            committer_signature()
        } else {
            repo.committer()
                .transpose()?
                .and_then(|s| s.to_owned().ok())
                .unwrap_or_else(committer_signature)
        };

        Ok((author.into(), committer))
    }

    fn git_settings(&self) -> anyhow::Result<GitConfigSettings> {
        // TODO: Make it easy to load the latest configuration in `gix`.
        // Re-open just the local configuration to be sure it's fresh before writing it.
        let repo = gix::open_opts(self.path(), self.open_options().clone())?;
        let config = repo.config_snapshot();
        GitConfigSettings::try_from_snapshot(&config)
    }

    fn set_git_settings(&self, settings: &GitConfigSettings) -> anyhow::Result<()> {
        settings.persist_to_local_config(self)
    }

    fn local_common_config_for_editing(&self) -> anyhow::Result<gix::config::File<'static>> {
        let local_config_path = self.common_dir().join("config");
        let config = gix::config::File::from_path_no_includes(
            local_config_path.clone(),
            gix::config::Source::Local,
        )?;
        Ok(config)
    }

    fn write_local_common_config(&self, local_config: &gix::config::File) -> anyhow::Result<()> {
        use std::io::Write;
        // Note: we don't use a lock file here to not risk changing the mode, and it's what Git does.
        //       But we lock the file so there is no raciness.
        let local_config_path = self.common_dir().join("config");
        let _lock = gix::lock::Marker::acquire_to_hold_resource(
            &local_config_path,
            gix::lock::acquire::Fail::Immediately,
            None,
        )?;
        let mut config_file = std::io::BufWriter::new(
            std::fs::File::options()
                .write(true)
                .truncate(true)
                .create(false)
                .open(local_config_path)?,
        );
        local_config.write_to(&mut config_file)?;
        config_file.flush()?;
        Ok(())
    }
}

const GITBUTLER_COMMIT_AUTHOR_NAME: &str = "GitButler";
const GITBUTLER_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";

/// Provide a signature with the GitButler author, and the current time or the time overridden
/// depending on the value for `purpose`.
fn committer_signature() -> gix::actor::Signature {
    gix::actor::Signature {
        name: GITBUTLER_COMMIT_AUTHOR_NAME.into(),
        email: GITBUTLER_COMMIT_AUTHOR_EMAIL.into(),
        time: commit_time("GIT_COMMITTER_DATE"),
    }
}

/// Return the time of a commit as `now` unless the `overriding_variable_name` contains a parseable date,
/// which is used instead.
fn commit_time(overriding_variable_name: &str) -> gix::date::Time {
    std::env::var(overriding_variable_name)
        .ok()
        .and_then(|time| gix::date::parse(&time, Some(std::time::SystemTime::now())).ok())
        .unwrap_or_else(gix::date::Time::now_local_or_utc)
}
