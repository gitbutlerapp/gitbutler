use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use but_oxidize::ObjectIdExt;
use gix::{prelude::ObjectIdExt as _, refs::Target};
use tracing::instrument;

use crate::update_head_reference;

use super::{Options, Outcome};

fn check_blob(file: Option<git2::DiffFile<'_>>) -> Option<git2::DiffFile<'_>> {
    let Some(file) = file else {
        return None;
    };
    if !file.is_valid_id()
        || !matches!(
            file.mode(),
            git2::FileMode::Blob | git2::FileMode::BlobExecutable
        )
    {
        return None;
    }
    Some(file)
}

fn upsert_into_index_and_tree_updaters(
    git2_repo: &git2::Repository,
    path: Option<&Path>,
    baseline: Option<git2::DiffFile<'_>>,
    target: Option<git2::DiffFile<'_>>,
    workdir: Option<git2::DiffFile<'_>>,
    index_changes: &mut HashMap<PathBuf, git2::Oid>,
    baseline_tree_updater: &mut git2::build::TreeUpdateBuilder,
    target_tree_updater: &mut git2::build::TreeUpdateBuilder,
) -> anyhow::Result<()> {
    let Some(path) = path else {
        anyhow::bail!("BUG: path not provided in notification");
    };
    if let Some(baseline) = check_blob(baseline)
        && let Some(target) = check_blob(target)
        && let Some(workdir) = check_blob(workdir)
    {
        eprintln!("file {} line {} base {}", file!(), line!(), baseline.id());
        let baseline_blob = git2_repo.find_blob(baseline.id())?;
        eprintln!("file {} line {} target {}", file!(), line!(), target.id());
        let target_blob = git2_repo.find_blob(target.id())?;
        eprintln!("file {} line {} workdir {}", file!(), line!(), workdir.id());
        let workdir_blob = git2_repo.find_blob(workdir.id())?;
        eprintln!("file {} line {}", file!(), line!());
        let result = git2::merge_file(
            git2::MergeFileInput::new().content(baseline_blob.content()),
            git2::MergeFileInput::new().content(target_blob.content()),
            git2::MergeFileInput::new().content(workdir_blob.content()),
            None,
        )?;
        eprintln!("file {} line {}", file!(), line!());
        if !result.is_automergeable() {
            anyhow::bail!("checkout would cause a conflict in {}", path.display());
        }
        // Update the contents of trees, but preserve the modes.
        index_changes.insert(path.to_owned(), workdir_blob.id().clone());
        baseline_tree_updater.upsert(path, workdir_blob.id(), baseline.mode());
        let result_blob = git2_repo.blob(result.content())?;
        target_tree_updater.upsert(path, result_blob, target.mode());
    } else {
        anyhow::bail!("conflict at {}: not all are blobs", path.display());
    }
    Ok(())
}

/// Perform all file operations necessary to turn the *worktree* of `repo` into
/// `new_head_id^{tree}`.
///
/// If `new_head_id` is a *commit*, we will also set `HEAD` (or the ref it points to if symbolic) to the `new_head_id`.
/// We will also update the `.git/index` to match the `new_head_id^{tree}`.
/// GitButler-conflicted commits are rejected by default before any worktree, index, or ref update.
///
/// We will always handle changes in the worktree safely to avoid loss of uncommitted information. This also means that deletions
/// never cause us to conflict. Conflicted files that would be checked out will cause an error.
///
/// #### Note: No rename tracking
///
/// To keep it simpler, we don't do rename tracking, so deletions and additions are always treated separately.
/// If this changes, then the source sid of a rename could also cause conflicts, maybe? It's a bit unclear what it would mean
/// in practice, but I guess that we bring deleted files back instead of conflicting.
#[instrument(skip(repo), err(Debug))]
pub fn safe_checkout_from_head(
    new_head_id: gix::ObjectId,
    repo: &gix::Repository,
    Options {
        skip_head_update,
        merge_base_override,
        allow_conflicted_commit_checkout,
    }: Options,
) -> anyhow::Result<Outcome> {
    let new_object = new_head_id.attach(repo).object()?;
    if !allow_conflicted_commit_checkout
        && new_object.kind.is_commit()
        && crate::Commit::from_id(new_head_id.attach(repo))?.is_conflicted()
    {
        bail!("Refusing to check out conflicted commit {new_head_id}");
    }

    let git2_repo = git2::Repository::open(repo.git_dir())?;
    let (baseline, index) = if let Some(merge_base_override) = merge_base_override {
        let baseline = git2_repo
            .find_object(merge_base_override.to_git2(), None)?
            .peel_to_tree()?;
        // libgit2 only pretends that HEAD's tree (and not the index) is the
        // given baseline. We also need it to pretend that the index is the
        // given baseline. That is not possible with the current API, so for
        // now, write to the index to make it the given baseline.
        let mut index = git2_repo.index()?;
        index.read_tree(&baseline)?;
        (Some(baseline), Some(index))
    } else {
        (None, None)
    };

    {
        let target = git2_repo
            .find_object(new_head_id.to_git2(), None)?
            .peel_to_tree()?;
        let mut index_changes = HashMap::new();
        let mut baseline_tree_updater = git2::build::TreeUpdateBuilder::new();
        let mut target_tree_updater = git2::build::TreeUpdateBuilder::new();
        let mut conflict_err: Option<anyhow::Error> = None;
        {
            let mut opts = git2::build::CheckoutBuilder::new();
            if let Some(ref baseline) = baseline {
                opts.baseline(baseline);
            }
            // opts.dry_run();
            // Uncomment the line above, run `cargo test -p but-core
            // partial_commit_with_adjacent_lines_conflicts_on_checkout` and see
            // that we are not notified of a conflict
            opts.notify_on(git2::CheckoutNotificationType::CONFLICT);
            opts.notify(|_must_be_conflict, path, baseline, target, workdir| {
                eprintln!("notified about {:?}", path);
                match upsert_into_index_and_tree_updaters(
                    &git2_repo,
                    path,
                    baseline,
                    target,
                    workdir,
                    &mut index_changes,
                    &mut baseline_tree_updater,
                    &mut target_tree_updater,
                ) {
                    Ok(_) => true,
                    Err(err) => {
                        conflict_err = Some(err);
                        false
                    }
                }
            });
            git2_repo.checkout_tree(&target.as_object(), Some(&mut opts))?
        }
        eprintln!("after dry run");
        if let Some(err) = conflict_err {
            return Err(err);
        }
        eprintln!("after conflict_err is none {:?}", index_changes);
        if !index_changes.is_empty() {
            let mut index = match index {
                Some(index) => index,
                None => git2_repo.index()?,
            };
            for (path, oid) in index_changes {
                let mut index_entry = index.get_path(&path, 0).context(format!(
                    "could not get stage 0 index entry corresponding to {}",
                    path.display()
                ))?;
                index_entry.id = oid;
                index.add(&index_entry)?;
            }
            // Update trees, then checkout for real.
            let baseline = match baseline {
                Some(baseline) => baseline,
                None => git2_repo.head()?.peel_to_tree()?,
            };
            let baseline = git2_repo
                .find_tree(baseline_tree_updater.create_updated(&git2_repo, &baseline)?)?;
            let target =
                git2_repo.find_tree(target_tree_updater.create_updated(&git2_repo, &target)?)?;
            let mut opts = git2::build::CheckoutBuilder::new();
            opts.baseline(&baseline);
            git2_repo.checkout_tree(&target.as_object(), Some(&mut opts))?;
        } else {
            eprintln!("simple checkout");

            // Checkout for real.
            let mut opts = git2::build::CheckoutBuilder::new();
            if let Some(ref baseline) = baseline {
                opts.baseline(baseline);
            }
            git2_repo.checkout_tree(&target.as_object(), Some(&mut opts))?
        }
    }

    let mut head_update = None;
    if new_object.kind.is_commit() && !skip_head_update {
        let needs_update = repo
            .head()?
            .id()
            .is_none_or(|actual_head_id| actual_head_id != new_head_id);
        if needs_update {
            // We play it loose here, as we assume a repository lock so we won't interfere with ourselves.
            // Git itself enforces no lock either, so we rely on basic locking ref-locking here. Good enough.
            let edits = update_head_reference(
                repo,
                Target::Object(new_head_id),
                true,
                "safe checkout",
                "GitButler".into(),
                new_object.into_commit().parent_ids().count(),
            )?;
            head_update = Some(edits);
        }
    }

    Ok(Outcome { head_update })
}
