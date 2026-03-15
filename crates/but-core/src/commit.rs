use std::{
    borrow::Cow, collections::HashSet, io::Write, path::Path, path::PathBuf, process::Stdio,
};

use anyhow::{Context as _, anyhow, bail};
use bstr::{BStr, BString, ByteSlice};
use but_error::Code;
use gix::objs::WriteTo;
use gix::prelude::ObjectIdExt;
use serde::{Deserialize, Serialize};

use crate::{
    ChangeId, Commit, CommitOwned, GitConfigSettings, RepositoryExt,
    cmd::prepare_with_shell_on_windows,
};

/// A collection of all the extra information we keep in the headers of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headers {
    /// A property we can use to determine if two different commits are
    /// actually the same "patch" at different points in time. We carry it
    /// forwards when you rebase a commit in GitButler.
    /// Note that these don't have to be unique within a branch even,
    /// and it's possible that different commits with the same change-id
    /// have different content.
    pub change_id: Option<ChangeId>,
    /// A property used to indicate that we've written a conflicted tree to a
    /// commit, and `Some(num_files)` is the amount of conflicted files.
    ///
    /// Conflicted commits should never make it into the main trunk.
    /// If `None`, the commit is a normal commit without a special tree.
    pub conflicted: Option<u64>,
}

/// Lifecycle
impl Headers {
    /// Creates a new set of headers with a randomly generated change_id.
    ///
    /// # Note - use [Self::from_config()] instead
    #[cfg(feature = "legacy")]
    pub fn new_with_random_change_id() -> Self {
        Self {
            change_id: Some(ChangeId::generate()),
            conflicted: None,
        }
    }

    /// Create a new instance, with the following rules for setting the change id:
    /// 1. Read `gitbutler.testing.changeId` from `config` and if it's a valid u128 integer, use it as change-id.
    /// 2. generate a new change-id
    pub fn from_config(config: &gix::config::Snapshot) -> Self {
        Headers {
            change_id: Some(
                config
                    .integer("gitbutler.testing.changeId")
                    .and_then(|id| {
                        u128::try_from(id)
                            .ok()
                            .map(ChangeId::from_number_for_testing)
                    })
                    .unwrap_or_else(ChangeId::generate),
            ),
            conflicted: None,
        }
    }

    /// Extract header information from the given `commit`, or return `None` if not present.
    pub fn try_from_commit(commit: &gix::objs::Commit) -> Option<Self> {
        Self::try_from_commit_headers(|| commit.extra_headers())
    }

    /// Extract header information from the given [`extra_headers`](gix::objs::Commit::extra_headers()) function,
    /// or return `None` if not present.
    pub fn try_from_commit_headers<'a, I>(
        extra_headers: impl Fn() -> gix::objs::commit::ExtraHeaders<I>,
    ) -> Option<Self>
    where
        I: Iterator<Item = (&'a BStr, &'a BStr)>,
    {
        let change_id = extra_headers()
            .find(HEADERS_NEW_CHANGE_ID_FIELD)
            .or_else(|| extra_headers().find(HEADERS_CHANGE_ID_FIELD))
            .map(ChangeId::from);

        let conflicted = extra_headers()
            .find(HEADERS_CONFLICTED_FIELD)
            .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

        if change_id.is_none() && conflicted.is_none() {
            return None;
        }

        Some(Headers {
            change_id,
            conflicted,
        })
    }

    /// Remove all header fields from `commit`.
    pub fn remove_in_commit(commit: &mut gix::objs::Commit) {
        for field in [
            HEADERS_VERSION_FIELD,
            HEADERS_CHANGE_ID_FIELD,
            HEADERS_CONFLICTED_FIELD,
            HEADERS_NEW_CHANGE_ID_FIELD,
        ] {
            if let Some(pos) = commit.extra_headers().find_pos(field) {
                commit.extra_headers.remove(pos);
            }
        }
    }

    /// Write the values from this instance to the given `commit`,  fully replacing any header
    /// that might have been there before.
    pub fn set_in_commit(&self, commit: &mut gix::objs::Commit) {
        Self::remove_in_commit(commit);
        commit
            .extra_headers
            .extend(Vec::<(BString, BString)>::from(self));
    }
}

const HEADERS_VERSION_FIELD: &str = "gitbutler-headers-version";
const HEADERS_CHANGE_ID_FIELD: &str = "gitbutler-change-id";
const HEADERS_NEW_CHANGE_ID_FIELD: &str = "change-id";
/// The name of the header field that stores the amount of conflicted files.
pub const HEADERS_CONFLICTED_FIELD: &str = "gitbutler-conflicted";
const HEADERS_VERSION: &str = "2";

impl From<&Headers> for Vec<(BString, BString)> {
    fn from(hdr: &Headers) -> Self {
        let mut out = vec![(
            BString::from(HEADERS_VERSION_FIELD),
            BString::from(HEADERS_VERSION),
        )];

        if let Some(change_id) = &hdr.change_id {
            out.push((HEADERS_NEW_CHANGE_ID_FIELD.into(), (**change_id).clone()));
        }

        if let Some(conflicted) = hdr.conflicted {
            out.push((
                HEADERS_CONFLICTED_FIELD.into(),
                conflicted.to_string().into(),
            ));
        }
        out
    }
}

/// Write `commit` into `repo`, removing any existing commit signature first, optionally creating a
/// new one based on repository configuration, and optionally updating `update_ref` to the new ID.
///
/// Apply any desired message/header mutations, such as Gerrit trailers, before calling this helper.
pub fn create(
    repo: &gix::Repository,
    mut commit: gix::objs::Commit,
    update_ref: Option<&gix::refs::FullNameRef>,
    sign_if_configured: bool,
) -> anyhow::Result<gix::ObjectId> {
    if let Some(pos) = commit
        .extra_headers()
        .find_pos(gix::objs::commit::SIGNATURE_FIELD_NAME)
    {
        commit.extra_headers.remove(pos);
    }

    if sign_if_configured && repo.git_settings()?.gitbutler_sign_commits.unwrap_or(false) {
        let mut buf = Vec::new();
        commit.write_to(&mut buf)?;
        match sign_buffer(repo, &buf) {
            Ok(signature) => {
                commit
                    .extra_headers
                    .push((gix::objs::commit::SIGNATURE_FIELD_NAME.into(), signature));
            }
            Err(err) => {
                if repo
                    .config_snapshot()
                    .boolean_filter("gitbutler.signCommits", |md| {
                        md.source != gix::config::Source::Local
                    })
                    .is_none()
                {
                    repo.set_git_settings(&GitConfigSettings {
                        gitbutler_sign_commits: Some(false),
                        ..GitConfigSettings::default()
                    })?;
                    return Err(
                        anyhow!("Failed to sign commit: {err}").context(Code::CommitSigningFailed)
                    );
                } else {
                    tracing::warn!(
                        "Commit signing failed but remains enabled as gitbutler.signCommits is explicitly enabled globally"
                    );
                    return Err(err);
                }
            }
        }
    }

    let oid = repo.write_object(&commit)?.detach();
    if let Some(update_ref) = update_ref {
        repo.reference(
            update_ref,
            oid,
            gix::refs::transaction::PreviousValue::Any,
            commit.message.as_bstr(),
        )?;
    }
    Ok(oid)
}

/// Sign `buffer` using repository configuration as obtained through `repo`,
/// similarly to Git's commit signing behavior.
pub fn sign_buffer(repo: &gix::Repository, buffer: &[u8]) -> anyhow::Result<BString> {
    fn into_command(prepare: gix::command::Prepare) -> std::process::Command {
        let cmd: std::process::Command = prepare.into();
        tracing::debug!(?cmd, "command to produce commit signature");
        cmd
    }

    fn as_literal_key(maybe_key: &BStr) -> Option<&BStr> {
        if let Some(key) = maybe_key.strip_prefix(b"key::") {
            return Some(key.into());
        }
        if maybe_key.starts_with(b"ssh-") {
            return Some(maybe_key);
        }
        None
    }

    fn signing_key(repo: &gix::Repository) -> anyhow::Result<BString> {
        if let Some(key) = repo.config_snapshot().string("user.signingkey") {
            return Ok(key.into_owned());
        }
        tracing::info!("Falling back to committer identity as user.signingKey isn't configured.");
        let mut buf = Vec::<u8>::new();
        repo.committer()
            .transpose()?
            .context("user.signingKey isn't configured and no committer is available either")?
            .actor()
            .trim()
            .write_to(&mut buf)?;
        Ok(buf.into())
    }

    let config = repo.config_snapshot();
    let signing_key = signing_key(repo)?;
    let sign_format = config.string("gpg.format");
    let is_ssh = sign_format.is_some_and(|value| value.as_ref() == "ssh");

    if is_ssh {
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

        let mut signing_cmd = prepare_with_shell_on_windows(gpg_program.into_owned())
            .args(["-Y", "sign", "-n", "git", "-f"]);

        let _key_storage;
        signing_cmd = if let Some(signing_key) = as_literal_key(signing_key.as_bstr()) {
            let mut keyfile = tempfile::NamedTempFile::new()?;
            keyfile.write_all(signing_key.as_bytes())?;

            #[cfg(unix)]
            {
                use std::os::unix::prelude::PermissionsExt;

                let mut permissions = keyfile.as_file().metadata()?.permissions();
                permissions.set_mode(0o600);
                keyfile.as_file().set_permissions(permissions)?;
            }

            let keyfile_path = keyfile.path().to_owned();
            _key_storage = keyfile.into_temp_path();
            signing_cmd
                .arg(keyfile_path)
                .arg("-U")
                .arg(buffer_file_to_sign_path.to_path_buf())
        } else {
            let signing_key = config
                .trusted_path("user.signingkey")
                .transpose()?
                .with_context(|| format!("Didn't trust 'ssh.signingKey': {signing_key}"))?;
            signing_cmd
                .arg(signing_key.into_owned())
                .arg(buffer_file_to_sign_path.to_path_buf())
        };
        let output = into_command(signing_cmd)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .output()?;

        if output.status.success() {
            let signature_path = buffer_file_to_sign_path.with_extension("sig");
            let sig_data = std::fs::read(signature_path)?;
            Ok(BString::new(sig_data))
        } else {
            let stderr = BString::new(output.stderr);
            let stdout = BString::new(output.stdout);
            bail!("Failed to sign SSH: {stdout} {stderr}");
        }
    } else {
        let gpg_program = config
            .trusted_program("gpg.program")
            .filter(|program| !program.is_empty())
            .map_or_else(
                || Path::new("gpg").into(),
                |program| Cow::Owned(program.into_owned().into()),
            );

        let mut cmd = into_command(
            prepare_with_shell_on_windows(gpg_program.as_ref())
                .args(["--status-fd=2", "-bsau"])
                .arg(gix::path::from_bstring(signing_key))
                .arg("-"),
        );
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                bail!(
                    "Could not find '{}'. Please make sure it is in your `PATH` or configure the full path using `gpg.program` in the Git configuration",
                    gpg_program.display()
                )
            }
            Err(err) => {
                return Err(err).context(format!("Could not execute GPG program using {cmd:?}"));
            }
        };
        child.stdin.take().expect("configured").write_all(buffer)?;

        let output = child.wait_with_output()?;
        if output.status.success() {
            Ok(BString::new(output.stdout))
        } else {
            let stderr = BString::new(output.stderr);
            let stdout = BString::new(output.stdout);
            bail!("Failed to sign GPG: {stdout} {stderr}");
        }
    }
}

/// When commits are in conflicting state, they store various trees which to help deal with the conflict.
///
/// This also includes variant that represents the blob which contains the
/// conflicted information.
#[derive(Debug, Copy, Clone)]
pub enum TreeKind {
    /// Our tree that caused a conflict during the merge.
    Ours,
    /// Their tree that caused a conflict during the merge.
    Theirs,
    /// The base of the conflicting mereg.
    Base,
    /// The tree that resulted from the merge with auto-resolution enabled.
    AutoResolution,
    /// The information about what is conflicted.
    ConflictFiles,
}

impl TreeKind {
    /// Return then name of the entry this tree would take in the 'meta' tree that captures cherry-pick conflicts.
    pub fn as_tree_entry_name(&self) -> &'static str {
        match self {
            TreeKind::Ours => ".conflict-side-0",
            TreeKind::Theirs => ".conflict-side-1",
            TreeKind::Base => ".conflict-base-0",
            TreeKind::AutoResolution => ".auto-resolution",
            TreeKind::ConflictFiles => ".conflict-files",
        }
    }
}

/// Instantiation
impl<'repo> Commit<'repo> {
    /// Decode the object at `commit_id` and keep its data for later query.
    pub fn from_id(commit_id: gix::Id<'repo>) -> anyhow::Result<Self> {
        commit_id.object()?.try_into_commit()?.try_into()
    }
}

impl<'repo> TryFrom<gix::Commit<'repo>> for Commit<'repo> {
    type Error = anyhow::Error;

    fn try_from(value: gix::Commit<'repo>) -> Result<Self, Self::Error> {
        let id = value.id();
        let commit = value.decode()?.try_into()?;
        Ok(Commit { id, inner: commit })
    }
}

impl From<Commit<'_>> for CommitOwned {
    fn from(Commit { id, inner }: Commit<'_>) -> Self {
        CommitOwned {
            id: id.detach(),
            inner,
        }
    }
}

impl Commit<'_> {
    /// Set this commit to use the given `headers`, completely replacing the ones it might currently have.
    pub fn set_headers(&mut self, header: &Headers) {
        header.set_in_commit(self)
    }
}

impl std::ops::Deref for Commit<'_> {
    type Target = gix::objs::Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Commit<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl std::ops::Deref for CommitOwned {
    type Target = gix::objs::Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for CommitOwned {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Headers {
    /// Return `true` if this commit contains a tree that is conflicted.
    pub fn is_conflicted(&self) -> bool {
        self.conflicted.is_some()
    }
}

impl CommitOwned {
    /// Attach `repo` to this instance to be able to do way more with it.
    pub fn attach(self, repo: &gix::Repository) -> Commit<'_> {
        let CommitOwned { id, inner } = self;
        Commit {
            id: id.attach(repo),
            inner,
        }
    }
}

/// Access
impl<'repo> Commit<'repo> {
    /// Remove the `repo` reference to become a fully owned instance.
    pub fn detach(self) -> CommitOwned {
        self.into()
    }

    /// Return `true` if this commit contains a tree that is conflicted.
    pub fn is_conflicted(&self) -> bool {
        self.headers().is_some_and(|hdr| hdr.is_conflicted())
    }

    /// If the commit is conflicted, then it returns the auto-resolution tree,
    /// otherwise it returns the commit's tree.
    ///
    /// Most of the time this is what you want to use when diffing or
    /// displaying the commit to the user.
    pub fn tree_id_or_auto_resolution(&self) -> anyhow::Result<gix::Id<'repo>> {
        self.tree_id_or_kind(TreeKind::AutoResolution)
    }

    /// If the commit is conflicted, then return the particular conflict-tree
    /// specified by `kind`, otherwise return the commit's tree.
    ///
    /// Most of the time, you will probably want to use [`Self::tree_id_or_auto_resolution()`]
    /// instead.
    pub fn tree_id_or_kind(&self, kind: TreeKind) -> anyhow::Result<gix::Id<'repo>> {
        Ok(if self.is_conflicted() {
            self.inner
                .tree
                .attach(self.id.repo)
                .object()?
                .into_tree()
                .find_entry(kind.as_tree_entry_name())
                .with_context(|| format!("Unexpected tree in conflicting commit {}", self.id))?
                .id()
        } else {
            self.inner.tree.attach(self.id.repo)
        })
    }

    /// If the commit is conflicted, returns the base, ours, and theirs tree IDs.
    pub fn conflicted_tree_ids(
        &self,
    ) -> anyhow::Result<Option<(gix::Id<'repo>, gix::Id<'repo>, gix::Id<'repo>)>> {
        if !self.is_conflicted() {
            return Ok(None);
        }
        let tree = self.inner.tree.attach(self.id.repo).object()?.into_tree();
        Ok(Some((
            tree.find_entry(TreeKind::Base.as_tree_entry_name())
                .with_context(|| format!("No base tree in conflicting commit {}", self.id))?
                .id(),
            tree.find_entry(TreeKind::Ours.as_tree_entry_name())
                .with_context(|| format!("No ours tree in conflicting commit {}", self.id))?
                .id(),
            tree.find_entry(TreeKind::Theirs.as_tree_entry_name())
                .with_context(|| format!("No theirs tree in conflicting commit {}", self.id))?
                .id(),
        )))
    }

    /// Return our custom headers, of present.
    pub fn headers(&self) -> Option<Headers> {
        Headers::try_from_commit(&self.inner)
    }
}

/// Conflict specific details
impl Commit<'_> {
    /// Obtains the conflict entries of a conflicted commit if the commit is
    /// conflicted, otherwise returns None.
    pub fn conflict_entries(&self) -> anyhow::Result<Option<ConflictEntries>> {
        let repo = self.id.repo;

        if !self.is_conflicted() {
            return Ok(None);
        }

        let tree = repo.find_tree(self.tree)?;
        let Some(conflicted_entries_blob) =
            tree.find_entry(TreeKind::ConflictFiles.as_tree_entry_name())
        else {
            bail!(
                "There has been a malformed conflicted commit, unable to find the conflicted files"
            );
        };
        let conflicted_entries_blob = conflicted_entries_blob.object()?.into_blob();
        let conflicted_entries: ConflictEntries =
            toml::from_str(&conflicted_entries_blob.data.as_bstr().to_str_lossy())?;

        Ok(Some(conflicted_entries))
    }
}

/// Represents what was causing a particular commit to conflict when rebased.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntries {
    /// The ancestors that were conflicted
    pub ancestor_entries: Vec<PathBuf>,
    /// The ours side entries that were conflicted
    pub our_entries: Vec<PathBuf>,
    /// The theirs side entries that were conflicted
    pub their_entries: Vec<PathBuf>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ConflictEntries);

impl ConflictEntries {
    /// If there are any conflict entries
    pub fn has_entries(&self) -> bool {
        !self.ancestor_entries.is_empty()
            || !self.our_entries.is_empty()
            || !self.their_entries.is_empty()
    }

    /// The total count of conflicted entries
    pub fn total_entries(&self) -> usize {
        let set = self
            .ancestor_entries
            .iter()
            .chain(self.our_entries.iter())
            .chain(self.their_entries.iter())
            .collect::<HashSet<_>>();

        set.len()
    }
}
