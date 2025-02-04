//! The machinery used to alter and mutate commits in various ways.

use anyhow::{bail, Context};
use bstr::{BString, ByteSlice};
use but_core::RepositoryExt;
use gix::prelude::ObjectIdExt;
use serde::{Deserialize, Serialize};

/// A reference frame, literally and figuratively, to tell the engine which commits to look at.
///
/// This is important when rewriting references to point to altered commits, and to know which commits to update
/// after an ancestor was changed.
#[derive(Default, Debug, Clone)]
pub struct RefFrame {
    /// The reference pointing to the merge commit of the *Workspace*, integrating two or more [stacks](crate::StackEntry),
    /// or the reference to the top-most commit of the stack.
    ///
    /// The difference between the two is really if the ref points to a commit that has *one* or *more* parents.
    ///
    /// If `None`, we will use the reference that `HEAD` points to or fail it is detached.
    pub topmost_ref: Option<gix::refs::FullName>,

    /// If known, the ref of the local **Target** branch into which `topmost_ref` would eventually be integrated into.
    ///
    /// It's used to compute the *merge-base* with `topmost_ref` if available, which serves as lower bound for commit-graph traversals.
    // TODO: actually use this (or remove it if it's really not needed)
    pub target_ref: Option<gix::refs::FullName>,
}

/// The place to apply the [change-specifications](DiffSpec) to.
///
/// Note that any commit this instance points to will be the basis to apply all changes to.
#[derive(Debug, Clone, Copy)]
pub enum Destination {
    /// Create a new commit on top of the given `Some(commit)`, so it will be the sole parent
    /// of the newly created commit, making it its ancestor.
    /// To create a commit at the position of the first commit of a branch, the parent has to be the merge-base with the *target branch*.
    ///
    /// If the commit is `None`, the base-state for the new commit will be an empty tree and the new commit will be the first one
    /// (i.e. have no parent). This is the case when `HEAD` is unborn. If `HEAD` is detached, this is a failure.
    AncestorForNewCommit(Option<gix::ObjectId>),
    /// Amend the given commit.
    AmendCommit(gix::ObjectId),
}

/// A change that should be used to create a new commit or alter an existing one, along with enough information to know where to find it.
#[derive(Debug)]
pub struct DiffSpec {
    /// The worktree-relative path to the worktree file with the content to commit.
    ///
    /// If `hunks` is empty, this means the current content of the file should be committed.
    pub path: BString,
    /// If one or more hunks are specified, match them with actual changes currently in the worktree.
    /// Failure to match them will lead to the change being dropped.
    /// If empty, the whole file is taken as is.
    pub hunk_headers: Vec<HunkHeader>,
}

/// The header of a hunk that represents a change to a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkHeader {
    /// The 1-based line number at which the previous version of the file started.
    pub old_start: u32,
    /// The non-zero amount of lines included in the previous version of the file.
    pub old_lines: u32,
    /// The 1-based line number at which the new version of the file started.
    pub new_start: u32,
    /// The non-zero amount of lines included in the new version of the file.
    pub new_lines: u32,
}

/// Additional information about the outcome of a [`mutate()`] call.
#[derive(Debug)]
pub struct MutateOutcome {
    /// Changes that were removed from a commit because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_requests: Vec<DiffSpec>,

    /// The newly created commit, or `None` if no commit could be created as all changes-requests were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// A mapping between `(old_commit, new_commit)`, where `old_commit` was used to produce `new_commit` during rebasing.
    /// The pairs are ordered in order of the rebase, with the last pair possibly being the workspace merge commit which
    /// [`RefFrame::topmost_ref`] pointed to.
    pub rewritten_commits: Vec<(gix::ObjectId, gix::ObjectId)>,
    /// All references that were edited to point to new or rewritten commits.
    /// The reference-edits also inform about the last seen value.
    pub ref_edits: Vec<gix::refs::transaction::RefEdit>,
}

/// Alter the single `destination` in a given `frame` with as many `changes` as possible and write new objects into `repo`,
/// but only if the commit succeeds.
/// If `origin_commit` is `Some(commit)`, all changes are considered to originate from the given commit, otherwise they originate from the worktree.
/// Use `message` as commit message.
///
/// Return additional information that helps to understand to what extent the commit was created.
pub fn mutate(
    repo: &gix::Repository,
    frame: RefFrame,
    destination: Destination,
    origin_commit: Option<gix::ObjectId>,
    changes: Vec<DiffSpec>,
    message: &str,
) -> anyhow::Result<MutateOutcome> {
    if changes.is_empty() {
        bail!("Have to provide at least one change in order to mutate a commit");
    }

    let (parents, target_tree) = match destination {
        Destination::AncestorForNewCommit(None) => {
            (Vec::new(), gix::ObjectId::empty_tree(repo.object_hash()))
        }
        Destination::AncestorForNewCommit(Some(parent)) => (
            vec![parent],
            parent
                .attach(repo)
                .object()?
                .peel_to_commit()?
                .tree_id()?
                .detach(),
        ),
        Destination::AmendCommit(_) => {
            todo!("get parents of the given commit ")
        }
    };

    let ref_to_update = match frame.topmost_ref {
        Some(name) => name,
        None => repo
            .head_name()?
            .context("Refusing to commit into a detached HEAD")?,
    };

    assert!(
        parents.len() < 2,
        "cannot currently handle more than 1 parent"
    );
    let is_on_top = if let &[parent] = &parents[..] {
        new_commit_is_on_top_of_tip(repo, ref_to_update.as_ref(), parent)?
    } else {
        // It's zero parents
        true
    };

    let changes_base_tree = if let Some(_origin_commit) = origin_commit {
        todo!("get base tree and apply changes by cherry-picking, probably can all be done by one function, but optimizations are different")
    } else {
        repo.head()?.id().and_then(|id| {
            id.object()
                .ok()
                .and_then(|obj| obj.peel_to_commit().ok())
                .and_then(|commit| commit.tree_id().ok().map(|id| id.detach()))
        })
    };

    let (maybe_new_tree, rejected_requests) =
        apply_worktree_changes(changes_base_tree, target_tree, repo, changes)?;
    let mut ref_edits = Vec::new();
    let new_commit = if let Some(tree) = maybe_new_tree {
        let (author, committer) = repo.commit_signatures()?;
        let (new_commit, ref_edit) = create_commit(
            &repo,
            Some(ref_to_update),
            author,
            committer,
            message,
            tree.detach(),
            parents,
            None,
        )?;
        ref_edits.push(ref_edit.expect("ref_to_update must have been updated"));
        Some(new_commit)
    } else {
        None
    };
    Ok(MutateOutcome {
        rejected_requests,
        new_commit,
        rewritten_commits: vec![],
        ref_edits,
    })
}

fn new_commit_is_on_top_of_tip(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
    parent: gix::ObjectId,
) -> anyhow::Result<bool> {
    let Some(head_ref) = repo.try_find_reference(name)? else {
        return Ok(true);
    };
    Ok(head_ref.id() == *parent)
}

/// Apply `changes` to `target_tree` and return the newly written tree.
/// All `changes` are expected to originate from the worktree, and will be cherry-picked onto `target_tree`
/// after applying them to the tree which they originate in, `changes_base_tree` in this case.
fn apply_worktree_changes(
    changes_base_tree: Option<gix::ObjectId>,
    target_tree: gix::ObjectId,
    repo: &gix::Repository,
    changes: Vec<DiffSpec>,
) -> anyhow::Result<(Option<gix::Id<'_>>, Vec<DiffSpec>)> {
    let base_tree = changes_base_tree
        .unwrap_or_else(|| gix::ObjectId::empty_tree(repo.object_hash()))
        .attach(repo)
        .object()?
        .peel_to_tree()?;
    let mut base_tree_editor = base_tree.edit()?;
    let rejected = Vec::new();

    let workdir = repo.work_dir().expect("bare repos aren't supported");
    for change in changes {
        assert!(
            change.hunk_headers.is_empty(),
            "TODO: cannot yet handle hunks"
        );
        // TODO: worktree filters, maybe via diff-filter to easily do file conversion? File types
        let blob_id = repo.write_blob_stream(std::fs::File::open(
            workdir.join(gix::path::from_bstr(change.path.as_bstr())),
        )?)?;
        base_tree_editor.upsert(&change.path, gix::object::tree::EntryKind::Blob, blob_id)?;
    }

    let altered_base_tree_id = base_tree_editor.write()?;
    if changes_base_tree.map_or(true, |base_tree| base_tree == target_tree) {
        return Ok((Some(altered_base_tree_id), rejected));
    }

    todo!("cherry pick the tree with just the right changes on the other")
}

mod create_commit {
    use anyhow::{anyhow, bail, Context};
    use bstr::{BString, ByteSlice};
    use but_core::cmd::prepare_with_shell_on_windows;
    use but_core::{GitConfigSettings, RepositoryExt};
    use gitbutler_error::error::Code;
    use gix::objs::WriteTo;
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefLog};
    use gix::refs::Target;
    use std::borrow::Cow;
    use std::io::Write;
    use std::os::unix::prelude::PermissionsExt;
    use std::path::Path;
    use std::process::Stdio;

    #[allow(clippy::too_many_arguments)]
    pub fn create_commit(
        repo: &gix::Repository,
        update_ref: Option<gix::refs::FullName>,
        author: gix::actor::Signature,
        committer: gix::actor::Signature,
        message: &str,
        tree: gix::ObjectId,
        parents: impl IntoIterator<Item = gix::ObjectId>,
        commit_headers: Option<but_core::commit::HeadersV2>,
    ) -> anyhow::Result<(gix::ObjectId, Option<gix::refs::transaction::RefEdit>)> {
        let mut commit = gix::objs::Commit {
            message: message.into(),
            tree,
            author,
            committer,
            encoding: None,
            parents: parents.into_iter().collect(),
            extra_headers: commit_headers.unwrap_or_default().into(),
        };

        if repo.git_settings()?.gitbutler_sign_commits.unwrap_or(false) {
            let mut buf = Vec::new();
            commit.write_to(&mut buf)?;
            let signature = sign_buffer(repo, &buf);
            match signature {
                Ok(signature) => {
                    commit.extra_headers.push(("gpgsig".into(), signature));
                }
                Err(e) => {
                    // If signing fails, turn off signing automatically and let everyone know,
                    repo.set_git_settings(&GitConfigSettings {
                        gitbutler_sign_commits: Some(false),
                        ..GitConfigSettings::default()
                    })?;
                    return Err(
                        anyhow!("Failed to sign commit: {}", e).context(Code::CommitSigningFailed)
                    );
                }
            }
        }

        let new_commit_id = repo.write_object(&commit)?.detach();
        let refedit = if let Some(refname) = update_ref {
            // TODO: should this be something more like what Git does?
            //       Probably should support making a commit in full with `gix`.
            let log_message = message;
            let edit = update_reference(repo, refname, new_commit_id, log_message.into())?;
            Some(edit)
        } else {
            None
        };
        Ok((new_commit_id, refedit))
    }

    fn update_reference(
        repo: &gix::Repository,
        name: gix::refs::FullName,
        target: gix::ObjectId,
        log_message: BString,
    ) -> anyhow::Result<gix::refs::transaction::RefEdit> {
        let mut edits = repo.edit_reference(gix::refs::transaction::RefEdit {
            change: Change::Update {
                log: LogChange {
                    mode: RefLog::AndReference,
                    force_create_reflog: false,
                    message: log_message,
                },
                expected: PreviousValue::Any,
                new: Target::Object(target),
            },
            name,
            deref: false,
        })?;
        assert_eq!(
            edits.len(),
            1,
            "only one reference can be created, splits aren't possible"
        );
        Ok(edits.pop().expect("exactly one edit"))
    }

    fn sign_buffer(repo: &gix::Repository, buffer: &[u8]) -> anyhow::Result<BString> {
        // check git config for gpg.signingkey
        // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
        let config = repo.config_snapshot();
        let signing_key = config.string("user.signingkey");
        let Some(signing_key) = signing_key else {
            bail!("No signing key found");
        };
        let signing_key = signing_key.to_str().context("non-utf8 signing key")?;
        let sign_format = config.string("gpg.format");
        let is_ssh = if let Some(sign_format) = sign_format {
            sign_format.as_ref() == "ssh"
        } else {
            false
        };

        if is_ssh {
            // write commit data to a temp file so we can sign it
            let mut signature_storage = tempfile::NamedTempFile::new()?;
            signature_storage.write_all(buffer)?;
            let buffer_file_to_sign_path = signature_storage.into_temp_path();

            let gpg_program = config
                .trusted_program("gpg.ssh.program")
                .filter(|program| !program.is_empty())
                .map_or_else(
                    || Path::new("ssh-keygen").into(),
                    |program| Cow::Owned(program.into_owned().into()),
                );

            let cmd = prepare_with_shell_on_windows(gpg_program.into_owned())
                .args(["-Y", "sign", "-n", "git", "-f"]);

            // Write the key to a temp file. This is needs to be created in the
            // same scope where its used; IE: in the command, otherwise the
            // tmpfile will get garbage collected
            let mut key_storage = tempfile::NamedTempFile::new()?;
            // support literal ssh key
            let signing_cmd = if let Some(signing_key) = as_literal_key(signing_key) {
                key_storage.write_all(signing_key.as_bytes())?;

                // if on unix
                #[cfg(unix)]
                {
                    // make sure the tempfile permissions are acceptable for a private ssh key
                    let mut permissions = key_storage.as_file().metadata()?.permissions();
                    permissions.set_mode(0o600);
                    key_storage.as_file().set_permissions(permissions)?;
                }

                cmd.arg(key_storage.path())
                    .arg("-U")
                    .arg(buffer_file_to_sign_path.to_path_buf())
            } else {
                cmd.arg(signing_key)
                    .arg(buffer_file_to_sign_path.to_path_buf())
            };
            let output = into_command(signing_cmd)
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .stdin(Stdio::null())
                .output()?;

            if output.status.success() {
                // read signed_storage path plus .sig
                let signature_path = buffer_file_to_sign_path.with_extension("sig");
                let sig_data = std::fs::read(signature_path)?;
                let signature = BString::new(sig_data);
                Ok(signature)
            } else {
                let stderr = BString::new(output.stderr);
                let stdout = BString::new(output.stdout);
                let std_both = format!("{} {}", stdout, stderr);
                bail!("Failed to sign SSH: {}", std_both);
            }
        } else {
            let gpg_program = config
                .trusted_program("gpg.program")
                .filter(|program| !program.is_empty())
                .map_or_else(
                    || Path::new("gpg").into(),
                    |program| Cow::Owned(program.into_owned().into()),
                );

            let mut cmd = into_command(prepare_with_shell_on_windows(gpg_program.as_ref()).args([
                "--status-fd=2",
                "-bsau",
                signing_key,
                "-",
            ]));
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    bail!("Could not find '{}'. Please make sure it is in your `PATH` or configure the full path using `gpg.program` in the Git configuration", gpg_program.display())
                }
                Err(err) => {
                    return Err(err)
                        .context(format!("Could not execute GPG program using {:?}", cmd))
                }
            };
            child.stdin.take().expect("configured").write_all(buffer)?;

            let output = child.wait_with_output()?;
            if output.status.success() {
                // read stdout
                let signature = BString::new(output.stdout);
                Ok(signature)
            } else {
                let stderr = BString::new(output.stderr);
                let stdout = BString::new(output.stdout);
                let std_both = format!("{} {}", stdout, stderr);
                bail!("Failed to sign GPG: {}", std_both);
            }
        }
    }

    fn into_command(prepare: gix::command::Prepare) -> std::process::Command {
        let cmd: std::process::Command = prepare.into();
        tracing::debug!(?cmd, "command to produce commit signature");
        cmd
    }

    fn as_literal_key(maybe_key: &str) -> Option<&str> {
        if let Some(key) = maybe_key.strip_prefix("key::") {
            return Some(key);
        }
        if maybe_key.starts_with("ssh-") {
            return Some(maybe_key);
        }
        None
    }
}
use create_commit::create_commit;
