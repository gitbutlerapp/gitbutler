//! In-process Git transport backed by [`grit-lib`](https://github.com/gitbutlerapp/grit).
//!
//! When the `gitbutler.grit` configuration flag is set — or when no `git`
//! binary is available on the system, in which case this is the fallback —
//! fetch/push/clone are performed entirely in-process over grit-lib's transport
//! stack instead of shelling out to the `git` binary. The transport (and the
//! authentication) is inferred from the remote URL scheme, mirroring grit's own
//! `gritx-fetch`/`gritx-push` examples:
//!
//! * `http(s)://` — smart HTTP; credentials come from the configured
//!   `credential.helper` programs (filled on a `401` and retried as Basic).
//! * `ssh://` / `git@host:path` — SSH; auth is the `ssh` child process's job.
//! * `git://` — anonymous Git daemon; no auth.
//! * `file://` / local path — a local repository; no auth.
//!
//! Unlike the `git`-CLI path, this backend never opens a TTY/askpass prompt: a
//! missing credential surfaces as a typed error.
//!
//! # Limitations
//!
//! [`clone`] is a deliberately minimal reimplementation (init + remote config +
//! fetch + checkout). It does **not** handle submodules, sparse-checkout,
//! shallow clones, or `.gitattributes` smudge filters/CRLF conversion.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use grit_lib::config::ConfigSet;
use grit_lib::credentials::HelperCredentialProvider;
use grit_lib::fetch::{NoProgress, fetch_remote};
use grit_lib::objects::{ObjectId, ObjectKind, parse_tree};
use grit_lib::odb::Odb;
use grit_lib::push::{push_http, push_remote};
use grit_lib::push_report::PushRefStatus;
use grit_lib::transfer::{
    self, FetchOptions, FetchOutcome, PushOptions, PushOutcome, PushRefSpec, TagMode,
};
use grit_lib::transport::http::http_fetch;
use grit_lib::transport::http::ureq_client::UreqHttpClient;
use grit_lib::transport::{
    ConnectOptions, GitDaemonTransport, Service, SshTransport, Transport as _,
};

use crate::RefSpec;

/// Whether to use the in-process grit transport for `repo` instead of shelling
/// out to `git`. True when the `gitbutler.grit` flag is set in the repository
/// configuration, **or** when no `git` binary is available on this system (in
/// which case grit is the fallback, since the CLI path cannot run).
pub fn is_enabled(repo: &gix::Repository) -> bool {
    config_flag(repo) || !git_binary_available()
}

/// Like [`is_enabled`], but reads only the global/system Git configuration. Used
/// by [`clone`], which runs before the target repository exists.
pub fn is_enabled_global() -> bool {
    config_flag_global() || !git_binary_available()
}

/// The `gitbutler.grit` flag from `repo`'s configuration.
fn config_flag(repo: &gix::Repository) -> bool {
    repo.config_snapshot()
        .boolean("gitbutler.grit")
        .unwrap_or(false)
}

/// The `gitbutler.grit` flag from global/system Git configuration.
fn config_flag_global() -> bool {
    gix::config::File::from_globals()
        .ok()
        .and_then(|cfg| cfg.boolean("gitbutler.grit").and_then(Result::ok))
        .unwrap_or(false)
}

/// Whether the external `git` binary can be found and run on this system.
///
/// Probed once and cached: it shells out to `git --version` (using the same
/// executable resolution the executor uses) only when needed, i.e. only when the
/// `gitbutler.grit` flag is not already set.
fn git_binary_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let git = gix::path::env::exe_invocation();
        std::process::Command::new(git)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    })
}

/// How a remote URL is reached, and therefore what authentication it implies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RemoteKind {
    /// `http(s)://` — smart HTTP; auth via configured credential helpers.
    Http,
    /// `git://` — anonymous Git daemon; no auth.
    GitDaemon,
    /// `ssh://`, `git+ssh://`, or scp-style `host:path`; auth via SSH keys/agent.
    Ssh,
    /// `file://` or a local filesystem path; no auth.
    Local,
}

impl RemoteKind {
    fn classify(url: &str) -> Self {
        if url.starts_with("http://") || url.starts_with("https://") {
            Self::Http
        } else if url.starts_with("git://") {
            Self::GitDaemon
        } else if grit_lib::transport::is_ssh_url(url) {
            Self::Ssh
        } else {
            Self::Local
        }
    }
}

/// A resolved remote: the URL to use, the transport kind, and the fetch refspecs.
struct ResolvedRemote {
    url: String,
    kind: RemoteKind,
    fetch_refspecs: Vec<String>,
}

/// Fetch from `remote` into `repo` using its configured fetch refspecs, over the
/// transport implied by the remote URL. Writes refs and objects directly into
/// the local repository (grit-lib drives the negotiation and pack ingest).
pub fn fetch(repo: &gix::Repository, remote: &str) -> Result<()> {
    let git_dir = repo.git_dir().to_owned();
    let resolved = resolve_remote(repo, remote, false)?;
    let opts = FetchOptions {
        refspecs: resolved.fetch_refspecs.clone(),
        ..Default::default()
    };
    run_fetch(&git_dir, &resolved, &opts)?;
    Ok(())
}

/// Push `refspec` to `remote` from `repo`, over the transport implied by the
/// remote URL. Returns an empty string on success (the `git`-CLI path returns
/// the remote's stderr; grit-lib has none to surface).
pub fn push(
    repo: &gix::Repository,
    remote: &str,
    refspec: &RefSpec,
    force: bool,
    force_with_lease: bool,
) -> Result<String> {
    let git_dir = repo.git_dir().to_owned();
    let resolved = resolve_remote(repo, remote, true)?;
    let pushspec = build_push_refspec(repo, remote, refspec, force, force_with_lease)?;
    let outcome = run_push(&git_dir, &resolved, &[pushspec], &PushOptions::default())?;
    summarize_push(&outcome)
}

/// Minimal in-process clone of `url` into `target_dir`: init a repository,
/// configure `origin`, fetch, set up the default branch + `HEAD`, and check out
/// the working tree. See the module-level limitations.
pub fn clone(url: &str, target_dir: &Path) -> Result<()> {
    if dir_is_non_empty(target_dir) {
        bail!(
            "target directory '{}' already exists and is not an empty directory",
            target_dir.display()
        );
    }

    // 1. Initialise a fresh (non-bare) repository. The initial branch is a
    //    placeholder; the real default branch is taken from the remote below.
    let repo = grit_lib::repo::init_repository(target_dir, false, "main", None, "files")
        .map_err(|e| anyhow!("failed to initialise repository: {e}"))?;
    let git_dir = repo.git_dir.clone();

    let fetch_refspec = "+refs/heads/*:refs/remotes/origin/*".to_owned();
    let resolved = ResolvedRemote {
        url: url.to_owned(),
        kind: RemoteKind::classify(url),
        fetch_refspecs: vec![fetch_refspec.clone()],
    };

    // 2. Configure the `origin` remote so the clone is usable afterwards.
    append_config_sections(
        &git_dir,
        &[(
            "remote \"origin\"".to_owned(),
            vec![
                ("url".to_owned(), url.to_owned()),
                ("fetch".to_owned(), fetch_refspec),
            ],
        )],
    )?;

    // 3. Fetch all branches into `refs/remotes/origin/*`.
    let opts = FetchOptions {
        refspecs: resolved.fetch_refspecs.clone(),
        tags: TagMode::Following,
        ..Default::default()
    };
    let outcome = run_fetch(&git_dir, &resolved, &opts)?;

    // 4. Pick the default branch (remote `HEAD` symref, else `main`).
    let branch = outcome.default_branch.clone().unwrap_or_else(|| {
        // Fall back to the first branch we fetched, else "main".
        outcome
            .updates
            .iter()
            .find_map(|u| {
                u.local_ref
                    .as_deref()
                    .and_then(|r| r.strip_prefix("refs/remotes/origin/"))
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "main".to_owned())
    });

    // 5. Materialise the local branch + HEAD + tracking config.
    let head_oid = grit_lib::refs::resolve_ref(&git_dir, &format!("refs/remotes/origin/{branch}"))
        .ok();
    if let Some(oid) = &head_oid {
        grit_lib::refs::write_ref(&git_dir, &format!("refs/heads/{branch}"), oid)
            .map_err(|e| anyhow!("failed to write refs/heads/{branch}: {e}"))?;
    }
    grit_lib::refs::write_symbolic_ref(&git_dir, "HEAD", &format!("refs/heads/{branch}"))
        .map_err(|e| anyhow!("failed to write HEAD: {e}"))?;
    append_config_sections(
        &git_dir,
        &[(
            format!("branch \"{branch}\""),
            vec![
                ("remote".to_owned(), "origin".to_owned()),
                ("merge".to_owned(), format!("refs/heads/{branch}")),
            ],
        )],
    )?;

    // 6. Check out the working tree (best-effort; no filters/submodules).
    if let Some(oid) = head_oid {
        checkout_worktree(&git_dir, target_dir, &oid)?;
    }
    Ok(())
}

// --- transport dispatch -----------------------------------------------------

fn run_fetch(
    git_dir: &Path,
    resolved: &ResolvedRemote,
    opts: &FetchOptions,
) -> Result<FetchOutcome> {
    let v2 = ConnectOptions {
        protocol_version: 2,
        ..Default::default()
    };
    let outcome = match resolved.kind {
        RemoteKind::Http => {
            let client = http_client(git_dir, Some("version=2"));
            http_fetch(&client, git_dir, &resolved.url, opts, &mut NoProgress)?
        }
        RemoteKind::GitDaemon => {
            let mut conn =
                GitDaemonTransport::new().connect(&resolved.url, Service::UploadPack, &v2)?;
            fetch_remote(git_dir, &mut *conn, opts, &mut NoProgress)?
        }
        RemoteKind::Ssh => {
            let mut conn = SshTransport::new().connect(&resolved.url, Service::UploadPack, &v2)?;
            fetch_remote(git_dir, &mut *conn, opts, &mut NoProgress)?
        }
        RemoteKind::Local => {
            transfer::fetch_local(git_dir, &local_git_dir(&resolved.url), opts)?
        }
    };
    Ok(outcome)
}

fn run_push(
    git_dir: &Path,
    resolved: &ResolvedRemote,
    refs: &[PushRefSpec],
    opts: &PushOptions,
) -> Result<PushOutcome> {
    // Push uses protocol v0/v1 (there is no v2 receive-pack).
    let v0 = ConnectOptions::default();
    let outcome = match resolved.kind {
        RemoteKind::Http => {
            let client = http_client(git_dir, None);
            push_http(&client, git_dir, &resolved.url, refs, opts, &mut NoProgress)?
        }
        RemoteKind::GitDaemon => {
            let mut conn =
                GitDaemonTransport::new().connect(&resolved.url, Service::ReceivePack, &v0)?;
            push_remote(git_dir, &mut *conn, refs, opts, &mut NoProgress)?
        }
        RemoteKind::Ssh => {
            let mut conn = SshTransport::new().connect(&resolved.url, Service::ReceivePack, &v0)?;
            push_remote(git_dir, &mut *conn, refs, opts, &mut NoProgress)?
        }
        RemoteKind::Local => {
            transfer::push_local(git_dir, &local_git_dir(&resolved.url), refs, opts)?
        }
    };
    Ok(outcome)
}

/// Build an HTTP client honoring request-shaping config (`http.proxy`,
/// cookies, `http.extraHeader`) and wired with a config-driven
/// [`HelperCredentialProvider`] so `credential.helper` programs satisfy a `401`.
fn http_client(git_dir: &Path, git_protocol: Option<&str>) -> UreqHttpClient {
    let client = match ConfigSet::load(Some(git_dir), true) {
        Ok(config) => {
            let provider = HelperCredentialProvider::new(config.clone());
            match UreqHttpClient::from_config(&config) {
                Ok(c) => c.with_credential_provider(Box::new(provider)),
                Err(_) => UreqHttpClient::with_credentials(Box::new(provider)),
            }
        }
        Err(_) => UreqHttpClient::new(),
    };
    match git_protocol {
        Some(v) => client.with_git_protocol(v.to_owned()),
        None => client,
    }
}

// --- remote/refspec resolution ----------------------------------------------

fn resolve_remote(repo: &gix::Repository, remote: &str, for_push: bool) -> Result<ResolvedRemote> {
    let r = repo
        .find_remote(remote)
        .with_context(|| format!("remote '{remote}' not found"))?;
    let direction = if for_push {
        gix::remote::Direction::Push
    } else {
        gix::remote::Direction::Fetch
    };
    let url = r
        .url(direction)
        .map(ToString::to_string)
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| anyhow!("remote '{remote}' has no configured URL"))?;
    let fetch_refspecs = fetch_refspecs_for(&r, remote);
    let kind = RemoteKind::classify(&url);
    Ok(ResolvedRemote {
        url,
        kind,
        fetch_refspecs,
    })
}

fn fetch_refspecs_for(remote: &gix::Remote<'_>, name: &str) -> Vec<String> {
    let specs: Vec<String> = remote
        .refspecs(gix::remote::Direction::Fetch)
        .iter()
        .map(|spec| spec.to_ref().to_bstring().to_string())
        .collect();
    if specs.is_empty() {
        vec![format!("+refs/heads/*:refs/remotes/{name}/*")]
    } else {
        specs
    }
}

fn build_push_refspec(
    repo: &gix::Repository,
    remote: &str,
    refspec: &RefSpec,
    force: bool,
    force_with_lease: bool,
) -> Result<PushRefSpec> {
    let dst = refspec
        .destination
        .clone()
        .ok_or_else(|| anyhow!("push refspec has no destination"))?;
    let delete = refspec.source.is_none();
    let src = match &refspec.source {
        None => None,
        Some(s) => Some(resolve_to_oid(repo, s)?),
    };
    // Force-with-lease: expect the remote ref to still match our last-known value
    // (the remote-tracking ref). If we cannot resolve it, fall back to no lease.
    let expected_old = if force_with_lease && !delete {
        lease_oid(repo, remote, &dst)
    } else {
        None
    };
    Ok(PushRefSpec {
        src,
        dst,
        force: force || refspec.update_non_fastforward,
        delete,
        expected_old,
        expect_absent: false,
    })
}

/// Resolve a refspec source (an object id or a revision) to a grit [`ObjectId`].
fn resolve_to_oid(repo: &gix::Repository, spec: &str) -> Result<ObjectId> {
    if let Ok(oid) = ObjectId::from_hex(spec) {
        return Ok(oid);
    }
    let id = repo
        .rev_parse_single(spec)
        .with_context(|| format!("failed to resolve push source '{spec}'"))?;
    ObjectId::from_bytes(id.as_bytes()).map_err(|e| anyhow!("invalid object id for '{spec}': {e}"))
}

/// The remote-tracking ref value used as the force-with-lease expectation.
fn lease_oid(repo: &gix::Repository, remote: &str, dst: &str) -> Option<ObjectId> {
    let branch = dst.strip_prefix("refs/heads/").unwrap_or(dst);
    let name = format!("refs/remotes/{remote}/{branch}");
    let mut reference = repo.try_find_reference(&name).ok().flatten()?;
    let id = reference.peel_to_id().ok()?.detach();
    ObjectId::from_bytes(id.as_bytes()).ok()
}

fn summarize_push(outcome: &PushOutcome) -> Result<String> {
    let rejected: Vec<String> = outcome
        .results
        .iter()
        .filter(|r| !matches!(r.status, PushRefStatus::Ok))
        .map(|r| {
            let reason = r
                .message
                .clone()
                .unwrap_or_else(|| format!("{:?}", r.status));
            format!("{} ({reason})", r.remote_ref)
        })
        .collect();
    if rejected.is_empty() {
        Ok(String::new())
    } else {
        Err(anyhow!("push rejected: {}", rejected.join(", ")))
    }
}

// --- clone helpers ----------------------------------------------------------

fn dir_is_non_empty(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Append well-formed config sections to the (freshly created) `<git_dir>/config`.
/// Each section is `(header, [(key, value)])`, e.g. `("remote \"origin\"", ...)`.
fn append_config_sections(git_dir: &Path, sections: &[(String, Vec<(String, String)>)]) -> Result<()> {
    use std::fmt::Write as _;
    let path = git_dir.join("config");
    let mut content = std::fs::read_to_string(&path).unwrap_or_default();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for (header, entries) in sections {
        let _ = writeln!(content, "[{header}]");
        for (key, value) in entries {
            let _ = writeln!(content, "\t{key} = {value}");
        }
    }
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Recursively write the tree of `commit_oid` into `work_tree` and build a basic
/// index. No smudge filters, CRLF conversion, or submodule handling.
fn checkout_worktree(git_dir: &Path, work_tree: &Path, commit_oid: &ObjectId) -> Result<()> {
    let odb = Odb::new(&git_dir.join("objects"));

    let commit = odb
        .read(commit_oid)
        .map_err(|e| anyhow!("reading commit {}: {e}", commit_oid.to_hex()))?;
    if commit.kind != ObjectKind::Commit {
        bail!("HEAD does not point at a commit");
    }
    let commit_data =
        grit_lib::objects::parse_commit(&commit.data).map_err(|e| anyhow!("parsing commit: {e}"))?;

    let mut entries = Vec::new();
    write_tree_recursive(&odb, work_tree, &commit_data.tree, "", &mut entries)?;

    let mut index = grit_lib::index::Index::new();
    index.entries = entries;
    index.sort();
    index
        .write(&git_dir.join("index"))
        .map_err(|e| anyhow!("writing index: {e}"))?;
    Ok(())
}

fn write_tree_recursive(
    odb: &Odb,
    work_tree: &Path,
    tree_oid: &ObjectId,
    prefix: &str,
    entries: &mut Vec<grit_lib::index::IndexEntry>,
) -> Result<()> {
    let tree = odb
        .read(tree_oid)
        .map_err(|e| anyhow!("reading tree {}: {e}", tree_oid.to_hex()))?;
    if tree.kind != ObjectKind::Tree {
        bail!("expected a tree object at {}", tree_oid.to_hex());
    }
    for entry in parse_tree(&tree.data).map_err(|e| anyhow!("parsing tree: {e}"))? {
        let name = String::from_utf8_lossy(&entry.name);
        let rel = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        let abs = work_tree.join(&rel);

        match entry.mode {
            0o040000 => {
                std::fs::create_dir_all(&abs)
                    .with_context(|| format!("creating directory {}", abs.display()))?;
                write_tree_recursive(odb, work_tree, &entry.oid, &rel, entries)?;
                continue;
            }
            // Submodule (gitlink): no submodule support in the minimal clone.
            0o160000 => continue,
            _ => {}
        }

        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let blob = odb
            .read(&entry.oid)
            .map_err(|e| anyhow!("reading blob {}: {e}", entry.oid.to_hex()))?;

        if entry.mode == 0o120000 {
            write_symlink(&abs, &blob.data)?;
        } else {
            std::fs::write(&abs, &blob.data)
                .with_context(|| format!("writing {}", abs.display()))?;
            #[cfg(unix)]
            if entry.mode == 0o100755 {
                use std::os::unix::fs::PermissionsExt as _;
                let perms = std::fs::Permissions::from_mode(0o755);
                let _ = std::fs::set_permissions(&abs, perms);
            }
        }

        entries.push(index_entry(&abs, rel.as_bytes(), entry.mode, entry.oid));
    }
    Ok(())
}

#[cfg(unix)]
fn write_symlink(path: &Path, target: &[u8]) -> Result<()> {
    use std::os::unix::ffi::OsStrExt as _;
    let target = std::ffi::OsStr::from_bytes(target);
    std::os::unix::fs::symlink(target, path)
        .with_context(|| format!("creating symlink {}", path.display()))
}

#[cfg(not(unix))]
fn write_symlink(path: &Path, target: &[u8]) -> Result<()> {
    // No symlink support: write the link text as a regular file.
    std::fs::write(path, target).with_context(|| format!("writing {}", path.display()))
}

/// Build an index entry for a just-written working-tree file, filling the stat
/// cache from disk so `git status` is clean after the clone.
fn index_entry(abs: &Path, rel: &[u8], mode: u32, oid: ObjectId) -> grit_lib::index::IndexEntry {
    let flags = (rel.len().min(0xFFF)) as u16;
    let mut entry = grit_lib::index::IndexEntry {
        ctime_sec: 0,
        ctime_nsec: 0,
        mtime_sec: 0,
        mtime_nsec: 0,
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        size: 0,
        oid,
        flags,
        flags_extended: None,
        path: rel.to_vec(),
        base_index_pos: 0,
    };
    if let Ok(meta) = std::fs::symlink_metadata(abs) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            entry.ctime_sec = meta.ctime() as u32;
            entry.ctime_nsec = meta.ctime_nsec() as u32;
            entry.mtime_sec = meta.mtime() as u32;
            entry.mtime_nsec = meta.mtime_nsec() as u32;
            entry.dev = meta.dev() as u32;
            entry.ino = meta.ino() as u32;
            entry.uid = meta.uid();
            entry.gid = meta.gid();
            entry.size = meta.size() as u32;
        }
        #[cfg(not(unix))]
        {
            entry.size = meta.len() as u32;
        }
    }
    entry
}

fn local_git_dir(url: &str) -> PathBuf {
    let raw = url.strip_prefix("file://").unwrap_or(url);
    let path = PathBuf::from(raw);
    let dot_git = path.join(".git");
    if dot_git.is_dir() { dot_git } else { path }
}
