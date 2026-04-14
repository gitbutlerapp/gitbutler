use anyhow::Context as _;
use bstr::BStr;
use but_error::Code;
use gix::{
    merge::tree::{Options, TreatAsUnresolved},
    prelude::ObjectIdExt,
    refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    },
};
use std::path::PathBuf;

use crate::{GitConfigSettings, commit::TreeKind};

/// Update `HEAD` to `new_target` and write a reflog entry composed from `operation`, `message`,
/// and `num_parents`.
///
/// If `deref` is `true` and `HEAD` is symbolic, update the reference `HEAD` currently points to
/// instead of rewriting `HEAD` itself. For example, if `HEAD` points to `refs/heads/main`,
/// `deref = true` with `Target::Object(<commit>)` updates `refs/heads/main` to that commit while
/// keeping `HEAD` symbolic.
///
/// If `deref` is `false`, update `HEAD` itself. This is what you want when changing which branch
/// `HEAD` symbolically points to, such as switching it to `refs/heads/gitbutler/edit`.
pub fn update_head_reference(
    repo: &gix::Repository,
    new_target: Target,
    deref: bool,
    operation: &str,
    message: &BStr,
    num_parents: usize,
) -> anyhow::Result<Vec<RefEdit>> {
    Ok(repo.edit_reference(RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: gix::reference::log::message(operation, message, num_parents),
            },
            // We use this helper only under higher-level repository coordination, so we intentionally
            // keep the expected value loose here and rely on ref locking for the actual write.
            expected: PreviousValue::Any,
            new: new_target,
        },
        name: "HEAD".try_into().expect("root refs are always valid"),
        deref,
    })?)
}

/// Easy access of settings relevant to GitButler for retrieval and storage in Git settings.
pub trait RepositoryExt: Sized {
    /// Returns a bundle of settings by querying the git configuration itself, assuring fresh data is loaded.
    fn git_settings(&self) -> anyhow::Result<GitConfigSettings>;
    /// Return the path to store per-project GitButler data, which is guaranteed to be inside
    /// of the `.git` directory, or in a unique folder outside of it.
    ///
    /// Resolution:
    /// * `gitbutler.storagePath` on release builds, or `gitbutler.<channel>.storagePath`
    ///   on non-release builds, with values like `gitbutler-alt`, `gitbutler-alt/nested`, or
    ///   `~/gitbutler-projects`.
    /// * If it is relative, it is interpreted relative to [`gix::Repository::git_dir`].
    ///   Paths that stay inside `.git` must live under a top-level directory whose name starts
    ///   with `gitbutler`.
    /// * If the resolved path is outside of [`gix::Repository::git_dir`], the storage path
    ///   becomes `<configured-path>/<project-handle>` so multiple projects can share one base path
    ///   without clobbering each other. This also applies to relative paths like `../../shared`.
    /// * Otherwise defaults to `<git-dir>/gitbutler` on all channels.
    //    The idea is to support one storage location per channel once we can make sure that the previously
    //    used metadata doesn't get lost, like the target branch, for instance by copying it over from stable.
    fn gitbutler_storage_path(&self) -> anyhow::Result<PathBuf>;
    /// Set all fields in `config` that are not `None` to disk into local repository configuration, or none of them.
    fn set_git_settings(&self, config: &GitConfigSettings) -> anyhow::Result<()>;
    /// Return all signatures that would be needed to perform a commit as configured in Git: `(author, committer)`.
    fn commit_signatures(&self) -> anyhow::Result<(gix::actor::Signature, gix::actor::Signature)>;

    /// Return the configuration freshly loaded from `.git/config` together with an acquired lock
    /// for that file so it can be changed in memory and safely written back without another writer
    /// racing the read-modify-write cycle.
    fn local_common_config_for_editing(
        &self,
    ) -> anyhow::Result<(gix::config::File<'static>, gix::lock::File)>;
    /// Write the given `local_config` to the file at `lock` of the while consuming
    /// the lock previously acquired with [`Self::local_common_config_for_editing()`].
    /// Note that only local configuraiton is written, so it's safe to use it with `repo.config_snapshot_mut()`.
    fn write_locked_config(
        &self,
        local_config: &gix::config::File,
        lock: gix::lock::File,
    ) -> anyhow::Result<()>;
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

    /// Configure the repository for diff operations between trees.
    /// This means it needs an object cache relative to the amount of files in the repository.
    fn for_tree_diffing(self) -> anyhow::Result<Self>;

    /// Create a tree that represents the current worktree and index state on top of `HEAD^{tree}`.
    ///
    /// This includes conflicted index entries and optionally skips untracked files larger than
    /// `untracked_limit_in_bytes` if that limit is non-zero.
    #[deprecated = "Gitizen alert: Do not soak up the entire working tree including untracked files, find a different solution"]
    fn create_wd_tree(&self, untracked_limit_in_bytes: u64) -> anyhow::Result<gix::ObjectId>;

    /// Return a repository configured for commit shortening,
    /// i.e. with an object database configured to *not* check for new packs.
    fn for_commit_shortening(self) -> Self;

    /// Just like the above, but with `gix` types.
    fn merges_cleanly(
        &self,
        ancestor_tree: gix::ObjectId,
        our_tree: gix::ObjectId,
        their_tree: gix::ObjectId,
    ) -> anyhow::Result<bool>;

    /// Return default label names when merging trees.
    ///
    /// Note that these should probably rather be branch names, but that's for another day.
    fn default_merge_labels(&self) -> gix::merge::blob::builtin_driver::text::Labels<'static> {
        gix::merge::blob::builtin_driver::text::Labels {
            ancestor: Some("base".into()),
            current: Some("ours".into()),
            other: Some("theirs".into()),
        }
    }

    /// Tree merge options that enforce undecidable conflicts to be forcefully resolved
    /// to favor ours, both when dealing with content merges and with tree merges.
    fn merge_options_force_ours(&self) -> anyhow::Result<gix::merge::tree::Options>;

    /// Tree merge options that enforce undecidable file/content conflicts to be
    /// forcefully resolved to favor theirs.
    ///
    /// `gix` does not currently expose a tree-level `Theirs` mode, so tree
    /// conflicts keep the default tree behavior.
    fn merge_options_force_theirs(&self) -> anyhow::Result<gix::merge::tree::Options>;

    /// Return options suitable for merging so that the merge stops immediately after the first conflict.
    /// It also returns the conflict kind to use when checking for unresolved conflicts.
    fn merge_options_fail_fast(
        &self,
    ) -> anyhow::Result<(
        gix::merge::tree::Options,
        gix::merge::tree::TreatAsUnresolved,
    )>;

    /// Just like [`Self::merge_options_fail_fast()`], but additionally don't perform rename tracking.
    /// This is useful if the merge result isn't going to be used, and we are only interested in knowing
    /// if a merge would succeed.
    fn merge_options_no_rewrites_fail_fast(
        &self,
    ) -> anyhow::Result<(gix::merge::tree::Options, TreatAsUnresolved)>;
}

impl RepositoryExt for gix::Repository {
    fn git_settings(&self) -> anyhow::Result<GitConfigSettings> {
        GitConfigSettings::try_from_snapshot(&self.config_snapshot())
    }

    fn gitbutler_storage_path(&self) -> anyhow::Result<PathBuf> {
        but_project_handle::gitbutler_storage_path(self)
    }

    fn set_git_settings(&self, settings: &GitConfigSettings) -> anyhow::Result<()> {
        settings.persist_to_local_config(self)
    }

    fn commit_signatures(&self) -> anyhow::Result<(gix::actor::Signature, gix::actor::Signature)> {
        let author = self
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
            self.committer()
                .transpose()?
                .and_then(|s| s.to_owned().ok())
                .unwrap_or_else(committer_signature)
        };

        Ok((author.into(), committer))
    }

    fn local_common_config_for_editing(
        &self,
    ) -> anyhow::Result<(gix::config::File<'static>, gix::lock::File)> {
        let local_config_path = self.common_dir().join("config");
        let lock = gix::lock::File::acquire_to_update_resource(
            &local_config_path,
            gix::lock::acquire::Fail::Immediately,
            None,
        )?;
        let config = gix::config::File::from_path_no_includes(
            local_config_path.clone(),
            gix::config::Source::Local,
        )?;
        Ok((config, lock))
    }

    fn write_locked_config(
        &self,
        local_config: &gix::config::File,
        lock: gix::lock::File,
    ) -> anyhow::Result<()> {
        crate::git_config::write_locked_config(local_config, lock)
    }

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

    fn for_tree_diffing(mut self) -> anyhow::Result<Self> {
        let bytes = self.compute_object_cache_size_for_tree_diffs(&***self.index_or_empty()?);
        self.object_cache_size_if_unset(bytes);
        Ok(self)
    }

    fn create_wd_tree(&self, untracked_limit_in_bytes: u64) -> anyhow::Result<gix::ObjectId> {
        use std::collections::HashSet;

        use bstr::ByteSlice;
        use gix::{
            bstr::BStr,
            status,
            status::index_worktree,
            status::plumbing::index_as_worktree::{Change, EntryStatus},
        };

        let (mut pipeline, index) = self.filter_pipeline(None)?;
        let mut added_worktree_file = |rela_path: &BStr,
                                       head_tree_editor: &mut gix::object::tree::Editor<'_>|
         -> anyhow::Result<bool> {
            let Some((id, kind, md)) = pipeline.worktree_file_to_object(rela_path, &index)? else {
                head_tree_editor.remove(rela_path)?;
                return Ok(false);
            };
            if untracked_limit_in_bytes != 0 && md.len() > untracked_limit_in_bytes {
                return Ok(false);
            }
            head_tree_editor.upsert(rela_path, kind, id)?;
            Ok(true)
        };
        let head_tree = self.head_tree_id_or_empty()?;
        let mut head_tree_editor = self.edit_tree(head_tree)?;
        let status_changes = self
            .status(gix::progress::Discard)?
            .tree_index_track_renames(gix::status::tree_index::TrackRenames::Disabled)
            .index_worktree_rewrites(None)
            .index_worktree_submodules(gix::status::Submodule::Given {
                ignore: gix::submodule::config::Ignore::Dirty,
                check_dirty: true,
            })
            .index_worktree_options_mut(|opts| {
                if let Some(opts) = opts.dirwalk_options.as_mut() {
                    opts.set_emit_ignored(None)
                        .set_emit_pruned(false)
                        .set_emit_tracked(false)
                        .set_emit_untracked(gix::dir::walk::EmissionMode::Matching)
                        .set_emit_collapsed(None);
                }
            })
            .into_iter(None)?
            .filter_map(|change| change.ok())
            .collect::<Vec<_>>();

        let mut worktreepaths_changed = HashSet::new();
        let mut untracked_items = Vec::new();
        for change in status_changes {
            match change {
                status::Item::TreeIndex(gix::diff::index::Change::Deletion {
                    location, ..
                }) => {
                    if !worktreepaths_changed.contains(location.as_bstr()) {
                        head_tree_editor.remove(location.as_ref())?;
                    }
                }
                status::Item::TreeIndex(
                    gix::diff::index::Change::Addition {
                        location,
                        entry_mode,
                        id,
                        ..
                    }
                    | gix::diff::index::Change::Modification {
                        location,
                        entry_mode,
                        id,
                        ..
                    },
                ) => {
                    if let Some(entry_mode) = entry_mode
                        .to_tree_entry_mode()
                        .filter(|_| !worktreepaths_changed.contains(location.as_bstr()))
                    {
                        head_tree_editor.upsert(
                            location.as_ref(),
                            entry_mode.kind(),
                            id.as_ref(),
                        )?;
                    }
                }
                status::Item::IndexWorktree(index_worktree::Item::Modification {
                    rela_path,
                    status: EntryStatus::Change(Change::Removed),
                    ..
                }) => {
                    head_tree_editor.remove(rela_path.as_bstr())?;
                    worktreepaths_changed.insert(rela_path);
                }
                status::Item::IndexWorktree(index_worktree::Item::Modification {
                    rela_path,
                    status:
                        EntryStatus::Change(Change::Type { .. } | Change::Modification { .. })
                        | EntryStatus::Conflict { .. }
                        | EntryStatus::IntentToAdd,
                    ..
                }) => {
                    if added_worktree_file(rela_path.as_ref(), &mut head_tree_editor)? {
                        worktreepaths_changed.insert(rela_path);
                    }
                }
                status::Item::IndexWorktree(index_worktree::Item::DirectoryContents {
                    entry:
                        gix::dir::Entry {
                            rela_path,
                            status: gix::dir::entry::Status::Untracked,
                            ..
                        },
                    ..
                }) => {
                    untracked_items.push(rela_path);
                }
                status::Item::IndexWorktree(index_worktree::Item::Modification {
                    rela_path,
                    status: EntryStatus::Change(Change::SubmoduleModification(change)),
                    ..
                }) => {
                    if let Some(possibly_changed_head_commit) = change.checked_out_head_id {
                        head_tree_editor.upsert(
                            rela_path.as_bstr(),
                            gix::object::tree::EntryKind::Commit,
                            possibly_changed_head_commit,
                        )?;
                        worktreepaths_changed.insert(rela_path);
                    }
                }
                status::Item::IndexWorktree(index_worktree::Item::Rewrite { .. })
                | status::Item::TreeIndex(gix::diff::index::Change::Rewrite { .. }) => {
                    unreachable!("disabled")
                }
                status::Item::IndexWorktree(
                    index_worktree::Item::Modification {
                        status: EntryStatus::NeedsUpdate(_),
                        ..
                    }
                    | index_worktree::Item::DirectoryContents {
                        entry:
                            gix::dir::Entry {
                                status:
                                    gix::dir::entry::Status::Tracked
                                    | gix::dir::entry::Status::Pruned
                                    | gix::dir::entry::Status::Ignored(_),
                                ..
                            },
                        ..
                    },
                ) => {}
            }
        }

        for rela_path in untracked_items {
            added_worktree_file(rela_path.as_ref(), &mut head_tree_editor)?;
        }

        Ok(head_tree_editor.write()?.detach())
    }

    fn for_commit_shortening(mut self) -> Self {
        self.objects.refresh = gix::odb::store::RefreshMode::Never;
        self
    }

    fn merges_cleanly(
        &self,
        ancestor_tree: gix::ObjectId,
        our_tree: gix::ObjectId,
        their_tree: gix::ObjectId,
    ) -> anyhow::Result<bool> {
        let (options, conflict_kind) = self.merge_options_no_rewrites_fail_fast()?;
        let merge_outcome = self
            .merge_trees(
                ancestor_tree,
                our_tree,
                their_tree,
                Default::default(),
                options,
            )
            .context("failed to merge trees")?;
        Ok(!merge_outcome.has_unresolved_conflicts(conflict_kind))
    }

    fn merge_options_force_ours(&self) -> anyhow::Result<Options> {
        Ok(self
            .tree_merge_options()?
            .with_tree_favor(Some(gix::merge::tree::TreeFavor::Ours))
            .with_file_favor(Some(gix::merge::tree::FileFavor::Ours)))
    }

    fn merge_options_force_theirs(&self) -> anyhow::Result<Options> {
        Ok(self
            .tree_merge_options()?
            .with_file_favor(Some(gix::merge::tree::FileFavor::Theirs)))
    }

    fn merge_options_fail_fast(
        &self,
    ) -> anyhow::Result<(gix::merge::tree::Options, TreatAsUnresolved)> {
        let conflict_kind = TreatAsUnresolved::forced_resolution();
        let options = self
            .tree_merge_options()?
            .with_fail_on_conflict(Some(conflict_kind));
        Ok((options, conflict_kind))
    }

    fn merge_options_no_rewrites_fail_fast(
        &self,
    ) -> anyhow::Result<(gix::merge::tree::Options, TreatAsUnresolved)> {
        let (options, conflict_kind) = self.merge_options_fail_fast()?;
        Ok((options.with_rewrites(None), conflict_kind))
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
