//! Helpers to keep virtual-branches metadata synchronized between TOML and the project database.

use std::{
    collections::HashMap,
    path::Path,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, anyhow, bail};
use but_core::ref_metadata::StackId;
use but_db::{VbBranchTarget, VbStack, VbStackHead, VbState, VirtualBranchesSnapshot};
use gitbutler_reference::{Refname, RemoteRefname};
use sha2::{Digest, Sha256};

use crate::virtual_branches_legacy_types::{Stack, StackBranch, Target, VirtualBranches};

/// Read virtual-branches state using DB as runtime source of truth while synchronizing with TOML.
///
/// The synchronization rules are:
/// * if DB is uninitialized, TOML is imported if valid; otherwise DB is initialized with empty state.
/// * if TOML is newer than DB metadata and valid, DB is overwritten with TOML.
/// * if newer TOML is invalid, TOML is rewritten from DB state.
/// * if TOML is missing, it is recreated from DB state.
pub fn read_synced_virtual_branches(path: &Path) -> anyhow::Result<VirtualBranches> {
    let mut db = db_handle_from_toml_path(path)?;
    let mut tx = db.immediate_transaction()?;
    let state = ensure_vb_storage_in_sync(path, &mut tx)?;
    tx.commit()?;
    Ok(state)
}

/// Write virtual-branches state to DB and TOML, keeping sync metadata up-to-date.
///
/// This always runs synchronization first to resolve any newer TOML changes before applying `data`.
pub fn write_virtual_branches_and_sync(path: &Path, data: &VirtualBranches) -> anyhow::Result<()> {
    let mut db = db_handle_from_toml_path(path)?;
    let mut tx = db.immediate_transaction()?;
    ensure_vb_storage_in_sync(path, &mut tx)?;
    let mut base_state = snapshot_state(path, &tx)?.unwrap_or_default().state;
    if !base_state.initialized {
        base_state.initialized = true;
    }
    let mut snapshot = legacy_to_snapshot(data, base_state)?;
    let info = write_toml(path, data)?;
    snapshot.state.toml_last_seen_mtime_ns = Some(info.mtime_ns);
    snapshot.state.toml_last_seen_sha256 = Some(info.sha256);
    snapshot.state.toml_mirror_dirty = false;
    persist_snapshot(&mut tx, &snapshot)?;
    tx.commit()?;
    Ok(())
}

/// Import TOML into DB if TOML is valid.
///
/// This is meant for oplog restore and similar flows where TOML was restored externally.
pub fn import_toml_into_db(path: &Path) -> anyhow::Result<()> {
    let info = read_toml_info(path)?;
    let TomlInfo::Parsed(parsed) = info else {
        bail!("Cannot import TOML into DB: TOML does not exist or is invalid");
    };

    let mut db = db_handle_from_toml_path(path)?;
    let mut tx = db.immediate_transaction()?;
    let mut state = snapshot_state(path, &tx)?.unwrap_or_default().state;
    state.initialized = true;
    state.toml_last_seen_mtime_ns = Some(parsed.mtime_ns);
    state.toml_last_seen_sha256 = Some(parsed.sha256);
    state.toml_mirror_dirty = false;
    let snapshot = legacy_to_snapshot(&parsed.data, state)?;
    persist_snapshot(&mut tx, &snapshot)?;
    tx.commit()?;
    Ok(())
}

fn ensure_vb_storage_in_sync(path: &Path, tx: &mut but_db::Transaction<'_>) -> anyhow::Result<VirtualBranches> {
    let snapshot = snapshot_state(path, tx)?.unwrap_or_default();
    let initialized = snapshot.state.initialized;
    let toml_info = read_toml_info(path)?;

    if !initialized {
        match toml_info {
            TomlInfo::Parsed(parsed) => {
                let mut state = snapshot.state;
                state.initialized = true;
                state.toml_last_seen_mtime_ns = Some(parsed.mtime_ns);
                state.toml_last_seen_sha256 = Some(parsed.sha256);
                state.toml_mirror_dirty = false;
                let db_snapshot = legacy_to_snapshot(&parsed.data, state)?;
                persist_snapshot(tx, &db_snapshot)?;
                return Ok(parsed.data);
            }
            TomlInfo::Missing | TomlInfo::Invalid(_) => {
                let mut state = snapshot.state;
                state.initialized = true;
                let empty = VirtualBranches::default();
                let info = write_toml(path, &empty)?;
                state.toml_last_seen_mtime_ns = Some(info.mtime_ns);
                state.toml_last_seen_sha256 = Some(info.sha256);
                state.toml_mirror_dirty = false;
                let db_snapshot = legacy_to_snapshot(&empty, state)?;
                persist_snapshot(tx, &db_snapshot)?;
                return Ok(empty);
            }
        }
    }

    let db_data = snapshot_to_legacy(&snapshot)?;
    match toml_info {
        TomlInfo::Missing => {
            let info = write_toml(path, &db_data)?;
            let mut updated = snapshot;
            updated.state.toml_last_seen_mtime_ns = Some(info.mtime_ns);
            updated.state.toml_last_seen_sha256 = Some(info.sha256);
            updated.state.toml_mirror_dirty = false;
            persist_snapshot(tx, &updated)?;
            Ok(db_data)
        }
        TomlInfo::Parsed(parsed) => {
            if toml_is_newer(&snapshot.state, parsed.mtime_ns, &parsed.sha256) {
                let mut state = snapshot.state;
                state.initialized = true;
                state.toml_last_seen_mtime_ns = Some(parsed.mtime_ns);
                state.toml_last_seen_sha256 = Some(parsed.sha256);
                state.toml_mirror_dirty = false;
                let db_snapshot = legacy_to_snapshot(&parsed.data, state)?;
                persist_snapshot(tx, &db_snapshot)?;
                Ok(parsed.data)
            } else if snapshot.state.toml_mirror_dirty {
                let info = write_toml(path, &db_data)?;
                let mut updated = snapshot;
                updated.state.toml_last_seen_mtime_ns = Some(info.mtime_ns);
                updated.state.toml_last_seen_sha256 = Some(info.sha256);
                updated.state.toml_mirror_dirty = false;
                persist_snapshot(tx, &updated)?;
                Ok(db_data)
            } else {
                Ok(db_data)
            }
        }
        TomlInfo::Invalid(invalid) => {
            if toml_is_newer(&snapshot.state, invalid.mtime_ns, &invalid.sha256) || snapshot.state.toml_mirror_dirty {
                let info = write_toml(path, &db_data)?;
                let mut updated = snapshot;
                updated.state.toml_last_seen_mtime_ns = Some(info.mtime_ns);
                updated.state.toml_last_seen_sha256 = Some(info.sha256);
                updated.state.toml_mirror_dirty = false;
                persist_snapshot(tx, &updated)?;
            }
            Ok(db_data)
        }
    }
}

fn toml_is_newer(state: &VbState, mtime_ns: i64, sha256: &str) -> bool {
    match state.toml_last_seen_mtime_ns {
        None => true,
        Some(last_seen) if mtime_ns > last_seen => true,
        Some(last_seen) if mtime_ns == last_seen => state.toml_last_seen_sha256.as_deref() != Some(sha256),
        Some(_) => false,
    }
}

fn db_handle_from_toml_path(path: &Path) -> anyhow::Result<but_db::DbHandle> {
    let Some(parent) = path.parent() else {
        bail!("Expected TOML path to have a parent directory: {}", path.display());
    };
    but_db::DbHandle::new_in_directory(parent)
}

fn snapshot_state(path: &Path, tx: &but_db::Transaction<'_>) -> anyhow::Result<Option<VirtualBranchesSnapshot>> {
    tx.virtual_branches()
        .get_snapshot()
        .with_context(|| format!("Failed to read VB snapshot from DB near {}", path.display()))
}

fn persist_snapshot(tx: &mut but_db::Transaction<'_>, snapshot: &VirtualBranchesSnapshot) -> anyhow::Result<()> {
    let mut handle = tx.virtual_branches_mut()?;
    handle.replace_snapshot(snapshot)?;
    handle.commit()?;
    Ok(())
}

fn write_toml(path: &Path, data: &VirtualBranches) -> anyhow::Result<WrittenToml> {
    let content = toml::to_string(data)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content.as_bytes())?;
    let bytes = std::fs::read(path)?;
    let mtime_ns = file_mtime_ns(path)?;
    Ok(WrittenToml {
        mtime_ns,
        sha256: sha256_hex(&bytes),
    })
}

fn read_toml_info(path: &Path) -> anyhow::Result<TomlInfo> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(TomlInfo::Missing),
        Err(err) => return Err(err.into()),
    };
    let mtime_ns = file_mtime_ns(path)?;
    let sha256 = sha256_hex(&bytes);
    let content = match String::from_utf8(bytes.clone()) {
        Ok(content) => content,
        Err(err) => {
            return Ok(TomlInfo::Invalid(InvalidToml {
                mtime_ns,
                sha256,
                error: anyhow!(err),
            }));
        }
    };
    match toml::from_str::<VirtualBranches>(&content) {
        Ok(data) => Ok(TomlInfo::Parsed(Box::new(ParsedToml { data, mtime_ns, sha256 }))),
        Err(err) => Ok(TomlInfo::Invalid(InvalidToml {
            mtime_ns,
            sha256,
            error: err.into(),
        })),
    }
}

fn file_mtime_ns(path: &Path) -> anyhow::Result<i64> {
    let modified = std::fs::metadata(path)?.modified()?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .or_else(|_| SystemTime::now().duration_since(UNIX_EPOCH))
        .context("Could not compute UNIX timestamp for TOML mtime")?;
    i64::try_from(duration.as_nanos()).context("mtime nanos exceed i64 range")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn legacy_to_snapshot(data: &VirtualBranches, mut state: VbState) -> anyhow::Result<VirtualBranchesSnapshot> {
    state.initialized = true;
    if let Some(target) = &data.default_target {
        state.default_target_remote_name = Some(target.branch.remote().to_owned());
        state.default_target_branch_name = Some(target.branch.branch().to_owned());
        state.default_target_remote_url = Some(target.remote_url.clone());
        state.default_target_sha = Some(target.sha.to_string());
        state.default_target_push_remote_name = target.push_remote_name.clone();
    } else {
        state.default_target_remote_name = None;
        state.default_target_branch_name = None;
        state.default_target_remote_url = None;
        state.default_target_sha = None;
        state.default_target_push_remote_name = None;
    }
    state.last_pushed_base_sha = data.last_pushed_base.map(|oid| oid.to_string());

    let mut stacks: Vec<_> = data.branches.iter().collect();
    stacks.sort_by_key(|(sid, stack)| (stack.order, sid.to_string()));

    let mut out_stacks = Vec::with_capacity(stacks.len());
    let mut out_heads = Vec::new();
    for (stack_id, stack) in stacks {
        out_stacks.push(VbStack {
            id: stack_id.to_string(),
            source_refname: stack.source_refname.as_ref().map(ToString::to_string),
            upstream_remote_name: stack.upstream.as_ref().map(|up| up.remote().to_owned()),
            upstream_branch_name: stack.upstream.as_ref().map(|up| up.branch().to_owned()),
            sort_order: i64::try_from(stack.order).context("Stack order exceeds i64")?,
            in_workspace: stack.in_workspace,
            #[expect(deprecated)]
            legacy_name: stack.name.clone(),
            #[expect(deprecated)]
            legacy_notes: stack.notes.clone(),
            #[expect(deprecated)]
            legacy_ownership: stack.ownership.to_string(),
            #[expect(deprecated)]
            legacy_allow_rebasing: stack.allow_rebasing,
            #[expect(deprecated)]
            legacy_post_commits: stack.post_commits,
            #[expect(deprecated)]
            legacy_tree_sha: stack.tree.to_string(),
            #[expect(deprecated)]
            legacy_head_sha: stack.head.to_string(),
            #[expect(deprecated)]
            legacy_created_timestamp_ms: stack.created_timestamp_ms.to_string(),
            #[expect(deprecated)]
            legacy_updated_timestamp_ms: stack.updated_timestamp_ms.to_string(),
        });

        for (position, head) in stack.heads.iter().enumerate() {
            out_heads.push(VbStackHead {
                stack_id: stack_id.to_string(),
                position: i64::try_from(position).context("Head position exceeds i64")?,
                name: head.name.clone(),
                head_sha: head.head.to_string(),
                pr_number: head.pr_number.map(|value| value as i64),
                archived: head.archived,
                review_id: head.review_id.clone(),
            });
        }
    }

    let mut out_targets: Vec<_> = data.branch_targets.iter().collect();
    out_targets.sort_by_key(|(sid, _)| sid.to_string());
    let branch_targets = out_targets
        .into_iter()
        .map(|(stack_id, target)| VbBranchTarget {
            stack_id: stack_id.to_string(),
            remote_name: target.branch.remote().to_owned(),
            branch_name: target.branch.branch().to_owned(),
            remote_url: target.remote_url.clone(),
            sha: target.sha.to_string(),
            push_remote_name: target.push_remote_name.clone(),
        })
        .collect();

    Ok(VirtualBranchesSnapshot {
        state,
        stacks: out_stacks,
        heads: out_heads,
        branch_targets,
    })
}

fn snapshot_to_legacy(snapshot: &VirtualBranchesSnapshot) -> anyhow::Result<VirtualBranches> {
    let default_target = match (
        snapshot.state.default_target_remote_name.as_ref(),
        snapshot.state.default_target_branch_name.as_ref(),
        snapshot.state.default_target_remote_url.as_ref(),
        snapshot.state.default_target_sha.as_ref(),
    ) {
        (Some(remote), Some(branch), Some(remote_url), Some(sha)) => Some(Target {
            branch: RemoteRefname::new(remote, branch),
            remote_url: remote_url.clone(),
            sha: gix::ObjectId::from_str(sha).with_context(|| format!("Invalid default target sha: {sha}"))?,
            push_remote_name: snapshot.state.default_target_push_remote_name.clone(),
        }),
        _ => None,
    };

    let last_pushed_base = snapshot
        .state
        .last_pushed_base_sha
        .as_ref()
        .map(|value| gix::ObjectId::from_str(value).with_context(|| format!("Invalid last_pushed_base sha: {value}")))
        .transpose()?;

    let mut branches = HashMap::new();
    for stack in &snapshot.stacks {
        let stack_id = StackId::from_str(&stack.id).with_context(|| format!("Invalid stack id '{}'", stack.id))?;
        let source_refname = stack
            .source_refname
            .as_ref()
            .map(|name| Refname::from_str(name).with_context(|| format!("Invalid source_refname '{name}'")))
            .transpose()?;
        let upstream = match (&stack.upstream_remote_name, &stack.upstream_branch_name) {
            (Some(remote), Some(branch)) => Some(RemoteRefname::new(remote, branch)),
            _ => None,
        };

        branches.insert(
            stack_id,
            Stack {
                id: stack_id,
                source_refname,
                upstream,
                order: usize::try_from(stack.sort_order)
                    .with_context(|| format!("Invalid stack sort order '{}'", stack.sort_order))?,
                in_workspace: stack.in_workspace,
                heads: Vec::new(),
                #[expect(deprecated)]
                notes: stack.legacy_notes.clone(),
                #[expect(deprecated)]
                ownership: stack
                    .legacy_ownership
                    .parse()
                    .with_context(|| format!("Invalid ownership claims for '{}'", stack.id))?,
                #[expect(deprecated)]
                allow_rebasing: stack.legacy_allow_rebasing,
                #[expect(deprecated)]
                post_commits: stack.legacy_post_commits,
                #[expect(deprecated)]
                tree: gix::ObjectId::from_str(&stack.legacy_tree_sha).with_context(|| {
                    format!("Invalid legacy tree sha '{}' for '{}'", stack.legacy_tree_sha, stack.id)
                })?,
                #[expect(deprecated)]
                created_timestamp_ms: stack
                    .legacy_created_timestamp_ms
                    .parse()
                    .with_context(|| format!("Invalid legacy created timestamp for '{}'", stack.id))?,
                #[expect(deprecated)]
                updated_timestamp_ms: stack
                    .legacy_updated_timestamp_ms
                    .parse()
                    .with_context(|| format!("Invalid legacy updated timestamp for '{}'", stack.id))?,
                #[expect(deprecated)]
                name: stack.legacy_name.clone(),
                #[expect(deprecated)]
                head: gix::ObjectId::from_str(&stack.legacy_head_sha).with_context(|| {
                    format!("Invalid legacy head sha '{}' for '{}'", stack.legacy_head_sha, stack.id)
                })?,
            },
        );
    }

    for head in &snapshot.heads {
        let stack_id =
            StackId::from_str(&head.stack_id).with_context(|| format!("Invalid stack id '{}'", head.stack_id))?;
        let stack = branches
            .get_mut(&stack_id)
            .ok_or_else(|| anyhow!("Missing stack '{}' for head '{}'", head.stack_id, head.name))?;
        stack.heads.push(StackBranch {
            head: gix::ObjectId::from_str(&head.head_sha)
                .with_context(|| format!("Invalid head sha '{}' on '{}'", head.head_sha, head.name))?,
            name: head.name.clone(),
            pr_number: head.pr_number.map(usize::try_from).transpose().with_context(|| {
                format!(
                    "Invalid pr_number '{}' on stack '{}'",
                    head.pr_number.unwrap_or_default(),
                    head.stack_id
                )
            })?,
            archived: head.archived,
            review_id: head.review_id.clone(),
        });
    }

    let mut branch_targets = HashMap::new();
    for target in &snapshot.branch_targets {
        let stack_id = StackId::from_str(&target.stack_id)
            .with_context(|| format!("Invalid branch_targets stack id '{}'", target.stack_id))?;
        branch_targets.insert(
            stack_id,
            Target {
                branch: RemoteRefname::new(&target.remote_name, &target.branch_name),
                remote_url: target.remote_url.clone(),
                sha: gix::ObjectId::from_str(&target.sha)
                    .with_context(|| format!("Invalid branch target sha '{}' for '{}'", target.sha, target.stack_id))?,
                push_remote_name: target.push_remote_name.clone(),
            },
        );
    }

    Ok(VirtualBranches {
        default_target,
        branch_targets,
        branches,
        last_pushed_base,
    })
}

enum TomlInfo {
    Missing,
    Parsed(Box<ParsedToml>),
    Invalid(InvalidToml),
}

struct ParsedToml {
    data: VirtualBranches,
    mtime_ns: i64,
    sha256: String,
}

struct InvalidToml {
    mtime_ns: i64,
    sha256: String,
    #[allow(dead_code)]
    error: anyhow::Error,
}

struct WrittenToml {
    mtime_ns: i64,
    sha256: String,
}
