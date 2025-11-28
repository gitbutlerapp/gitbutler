#[cfg(target_family = "unix")]
use std::os::unix::prelude::PermissionsExt;
use std::{borrow::Borrow, fs, path::PathBuf};

use anyhow::{Context as _, Result, anyhow};
use bstr::{BString, ByteSlice, ByteVec};
use but_ctx::Context;
use diffy::{Line, Patch, apply_bytes as diffy_apply};
use gitbutler_cherry_pick::RepositoryExt as _;
use hex::ToHex;

use crate::GitHunk;

// this function takes a list of file ownership,
// constructs a tree from those changes on top of the target
// and writes it as a new tree for storage
pub fn hunks_onto_oid<T>(
    ctx: &Context,
    target: git2::Oid,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<T>>)>,
) -> Result<git2::Oid>
where
    T: Into<GitHunk> + Clone,
{
    hunks_onto_commit(ctx, target, files)
}

pub fn hunks_onto_commit<T>(
    ctx: &Context,
    commit_oid: git2::Oid,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<T>>)>,
) -> Result<git2::Oid>
where
    T: Into<GitHunk> + Clone,
{
    // read the base sha into an index
    let git_repository = &*ctx.git2_repo.get()?;

    let head_commit = git_repository.find_commit(commit_oid)?;
    let base_tree = git_repository.find_real_tree(&head_commit, Default::default())?;

    hunks_onto_tree(ctx, &base_tree, files, false)
}

pub fn hunks_onto_tree<T>(
    ctx: &Context,
    base_tree: &git2::Tree,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<T>>)>,
    allow_new_file: bool,
) -> Result<git2::Oid>
where
    T: Into<GitHunk> + Clone,
{
    let repo = &*ctx.git2_repo.get()?;
    let workdir = repo
        .workdir()
        .context("BUG: this codepath can only deal with non-bare repositories")?;
    let mut builder = git2::build::TreeUpdateBuilder::new();
    // now update the index with content in the working directory for each file
    for (rel_path, hunks) in files {
        let rel_path = rel_path.borrow();
        let hunks: Vec<GitHunk> = hunks.borrow().iter().map(|h| h.clone().into()).collect();
        let full_path = workdir.join(rel_path);

        let is_submodule = full_path.is_dir()
            && hunks.len() == 1
            && hunks[0].diff_lines.contains_str(b"Subproject commit");

        // if file exists
        let full_path_exists = full_path.exists();
        let discard_hunk = (hunks.len() == 1).then(|| &hunks[0]);
        if full_path_exists || allow_new_file {
            if discard_hunk.is_some_and(|hunk| hunk.change_type == crate::ChangeType::Deleted) {
                // File was created but now that hunk is being discarded with an inversed hunk
                builder.remove(rel_path);
                fs::remove_file(full_path.clone()).or_else(|err| {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(err)
                    }
                })?;
                continue;
            }
            // if file is executable, use 755, otherwise 644
            let mut filemode = git2::FileMode::Blob;
            // check if full_path file is executable
            if let Ok(metadata) = std::fs::symlink_metadata(&full_path) {
                #[cfg(target_family = "unix")]
                {
                    if metadata.permissions().mode() & 0o111 != 0 {
                        filemode = git2::FileMode::BlobExecutable;
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    // NOTE: *Keep* the existing executable bit if it was present
                    //       in the tree already, don't try to derive something from
                    //       the FS that doesn't exist.
                    filemode = base_tree
                        .get_path(rel_path)
                        .ok()
                        .and_then(|entry| {
                            (entry.filemode() & 0o100000 == 0o100000
                                && entry.filemode() & 0o111 != 0)
                                .then_some(git2::FileMode::BlobExecutable)
                        })
                        .unwrap_or(filemode);
                }

                if metadata.file_type().is_symlink() {
                    filemode = git2::FileMode::Link;
                }
            }

            // get the blob
            if filemode == git2::FileMode::Link {
                // it's a symlink, make the content the path of the link
                let link_target = std::fs::read_link(&full_path)?;

                // if the link target is inside the project repository, make it relative
                let link_target = link_target.strip_prefix(workdir).unwrap_or(&link_target);

                let blob_oid = repo.blob(
                    link_target
                        .to_str()
                        .ok_or_else(|| {
                            anyhow!("path contains invalid utf-8 characters: {link_target:?}")
                        })?
                        .as_bytes(),
                )?;
                builder.upsert(rel_path, blob_oid, filemode);
            } else if let Ok(tree_entry) = base_tree.get_path(rel_path) {
                if discard_hunk.is_some_and(|hunk| hunk.binary) {
                    let new_blob_oid = &hunks[0].diff_lines;
                    // convert string to Oid
                    let new_blob_oid = new_blob_oid
                        .to_str()
                        .expect("hex-string")
                        .parse()
                        .context("failed to diff as oid")?;
                    builder.upsert(rel_path, new_blob_oid, filemode);
                } else {
                    // blob from tree_entry
                    let blob = tree_entry
                        .to_object(repo)
                        .unwrap()
                        .peel_to_blob()
                        .context("failed to get blob")?;

                    let blob_contents = blob.content();

                    let mut hunks = hunks.iter().collect::<Vec<_>>();
                    hunks.sort_by_key(|hunk| hunk.new_start);
                    let mut all_diffs = BString::default();
                    for hunk in hunks {
                        all_diffs.push_str(&hunk.diff_lines);
                    }

                    let patch = Patch::from_bytes(&all_diffs)?;
                    let blob_contents = apply(blob_contents, &patch).context(format!(
                        "failed to apply\n{}\nonto:\n{}",
                        all_diffs.as_bstr(),
                        blob_contents.as_bstr()
                    ));

                    match blob_contents {
                        Ok(blob_contents) => {
                            // create a blob
                            let new_blob_oid = repo.blob(blob_contents.as_bytes())?;
                            // upsert into the builder
                            builder.upsert(rel_path, new_blob_oid, filemode);
                        }
                        Err(_) => {
                            // If the patch failed to apply, do nothing, this is handled elsewhere
                            continue;
                        }
                    }
                }
            } else if is_submodule {
                let mut blob_contents = BString::default();

                let mut hunks = hunks.iter().collect::<Vec<_>>();
                hunks.sort_by_key(|hunk| hunk.new_start);
                let mut all_diffs = BString::default();
                for hunk in hunks {
                    all_diffs.push_str(&hunk.diff_lines);
                }
                let patch = Patch::from_bytes(&all_diffs)?;
                blob_contents = apply(&blob_contents, &patch)
                    .context(format!("failed to apply {all_diffs}"))?;

                // create a blob
                let new_blob_oid = repo.blob(blob_contents.as_bytes())?;
                // upsert into the builder
                builder.upsert(rel_path, new_blob_oid, filemode);
            } else if !full_path_exists
                && discard_hunk.is_some_and(|hunk| {
                    hunk.change_type == crate::ChangeType::Added
                        || hunk.change_type == crate::ChangeType::Untracked
                })
            {
                // File was deleted but now that hunk is being discarded with an inversed hunk
                let mut all_diffs = BString::default();
                for hunk in hunks {
                    all_diffs.push_str(&hunk.diff_lines);
                }
                let patch = Patch::from_bytes(&all_diffs)?;
                let blob_contents =
                    apply([], &patch).context(format!("failed to apply {all_diffs}"))?;

                let new_blob_oid = repo.blob(&blob_contents)?;
                builder.upsert(rel_path, new_blob_oid, filemode);
            } else {
                // create a git blob from a file on disk
                let blob_oid = repo
                    .blob_path(&full_path)
                    .context(format!("failed to create blob from path {:?}", &full_path))?;
                builder.upsert(rel_path, blob_oid, filemode);
            }
        } else if base_tree.get_path(rel_path).is_ok() {
            // remove file from index if it exists in the base tree
            builder.remove(rel_path);
        }
    }

    // now write out the tree
    let tree_oid = builder
        .create_updated(&*ctx.git2_repo.get()?, base_tree)
        .context("failed to write updated tree")?;

    Ok(tree_oid)
}

/// Just like [`diffy::apply()`], but on error it will attach hashes of the input `base_image` and `patch`.
pub fn apply<S: AsRef<[u8]>>(base_image: S, patch: &Patch<'_, [u8]>) -> Result<BString> {
    fn md5_hash_hex(b: impl AsRef<[u8]>) -> String {
        md5::compute(b).encode_hex()
    }

    #[derive(Debug)]
    #[expect(dead_code)] // Read by Debug auto-impl, which doesn't count
    pub enum DebugLine {
        // Note that each of these strings is a hash only
        Context(String),
        Delete(String),
        Insert(String),
    }

    impl<'a> From<&diffy::Line<'a, [u8]>> for DebugLine {
        fn from(line: &Line<'a, [u8]>) -> Self {
            match line {
                Line::Context(s) => DebugLine::Context(md5_hash_hex(s)),
                Line::Delete(s) => DebugLine::Delete(md5_hash_hex(s)),
                Line::Insert(s) => DebugLine::Insert(md5_hash_hex(s)),
            }
        }
    }

    #[derive(Debug)]
    #[expect(dead_code)] // Read by Debug auto-impl, which doesn't count
    struct DebugHunk {
        old_range: diffy::HunkRange,
        new_range: diffy::HunkRange,
        lines: Vec<DebugLine>,
    }

    impl<'a> From<&diffy::Hunk<'a, [u8]>> for DebugHunk {
        fn from(hunk: &diffy::Hunk<'a, [u8]>) -> Self {
            Self {
                old_range: hunk.old_range(),
                new_range: hunk.new_range(),
                lines: hunk.lines().iter().map(Into::into).collect(),
            }
        }
    }

    #[derive(Debug)]
    #[expect(dead_code)] // Read by Debug auto-impl, which doesn't count
    struct DebugContext {
        base_image_hash: String,
        hunks: Vec<DebugHunk>,
    }

    impl std::fmt::Display for DebugContext {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(self, f)
        }
    }

    diffy_apply(base_image.as_ref(), patch)
        .with_context(|| DebugContext {
            base_image_hash: md5_hash_hex(base_image),
            hunks: patch.hunks().iter().map(Into::into).collect(),
        })
        .map(Into::into)
}
