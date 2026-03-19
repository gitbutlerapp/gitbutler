//! Helpers to keep virtual-branches metadata synchronized between TOML and the project database.

use std::{
    cmp::Ordering, collections::HashMap, io::Read as _, path::Path, str::FromStr, time::UNIX_EPOCH,
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
/// * if TOML is invalid, TOML is rewritten from DB state.
/// * if TOML is missing, it is recreated from DB state.
pub fn read_synced_virtual_branches(path: &Path) -> anyhow::Result<VirtualBranches> {
    let mut db = db_handle_from_toml_path(path)?;
    let mut tx = db.immediate_transaction()?;
    let state = ensure_vb_storage_in_sync(path, &mut tx)?;
    tx.commit()?;
    Ok(state)
}

/// Write virtual-branches `vb_state` to DB and TOML, keeping sync metadata up-to-date.
///
/// This always runs synchronization first to resolve any newer TOML changes before applying `data`.
pub fn write_virtual_branches_and_sync(path: &Path, vb: &VirtualBranches) -> anyhow::Result<()> {
    let mut db = db_handle_from_toml_path(path)?;
    let mut tx = db.immediate_transaction()?;
    ensure_vb_storage_in_sync(path, &mut tx)?;
    let mut db_state = snapshot_state(path, &tx)?.unwrap_or_default().state;
    if !db_state.initialized {
        db_state.initialized = true;
    }
    let mut snapshot = legacy_to_snapshot(vb, into_db_toml_file_info(db_state))?;
    let info = write_toml(path, vb)?;
    info.update_last_seen_metadata_on(&mut snapshot.state);
    persist_snapshot(&mut tx, snapshot)?;
    tx.commit()?;
    Ok(())
}

/// Import TOML into DB if TOML is valid, overwriting existing data forcefully.
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
    parsed.update_last_seen_metadata_on(&mut state);
    let snapshot = legacy_to_snapshot(&parsed.data, into_db_toml_file_info(state))?;
    persist_snapshot(&mut tx, snapshot)?;
    tx.commit()?;
    Ok(())
}

fn ensure_vb_storage_in_sync(
    path: &Path,
    tx: &mut but_db::Transaction<'_>,
) -> anyhow::Result<VirtualBranches> {
    let snapshot = snapshot_state(path, tx)?.unwrap_or_default();
    let toml_info = read_toml_info(path)?;

    if !snapshot.state.initialized {
        let mut state = snapshot.state;
        state.initialized = true;
        return match toml_info {
            TomlInfo::Parsed(parsed) => {
                parsed.update_last_seen_metadata_on(&mut state);
                let db_snapshot = legacy_to_snapshot(&parsed.data, into_db_toml_file_info(state))?;
                persist_snapshot(tx, db_snapshot)?;
                Ok(parsed.data)
            }
            TomlInfo::Missing | TomlInfo::Invalid => {
                let empty = VirtualBranches::default();
                let info = write_toml(path, &empty)?;
                info.update_last_seen_metadata_on(&mut state);
                let db_snapshot = legacy_to_snapshot(&empty, into_db_toml_file_info(state))?;
                persist_snapshot(tx, db_snapshot)?;
                Ok(empty)
            }
        };
    }

    match toml_info {
        TomlInfo::Missing => {
            let db_data = snapshot_to_legacy(&snapshot)?;
            let info = write_toml(path, &db_data)?;
            let mut updated = snapshot;
            info.update_last_seen_metadata_on(&mut updated.state);
            persist_snapshot(tx, updated)?;
            Ok(db_data)
        }
        TomlInfo::Parsed(parsed) => {
            match toml_compare(&snapshot.state, parsed.mtime_ns, &parsed.sha256_hexdigest) {
                // Toml is newer
                Ordering::Greater => {
                    let mut state = snapshot.state;
                    parsed.update_last_seen_metadata_on(&mut state);
                    let db_snapshot =
                        legacy_to_snapshot(&parsed.data, into_db_toml_file_info(state))?;
                    persist_snapshot(tx, db_snapshot)?;
                    Ok(parsed.data)
                }
                Ordering::Equal => Ok(snapshot_to_legacy(&snapshot)?),
                // Toml is older
                Ordering::Less => {
                    let db_data = snapshot_to_legacy(&snapshot)?;
                    let info = write_toml(path, &db_data)?;
                    let mut updated = snapshot;
                    info.update_last_seen_metadata_on(&mut updated.state);
                    persist_snapshot(tx, updated)?;
                    Ok(db_data)
                }
            }
        }
        TomlInfo::Invalid => {
            let db_data = snapshot_to_legacy(&snapshot)?;
            let info = write_toml(path, &db_data)?;
            let mut updated = snapshot;
            info.update_last_seen_metadata_on(&mut updated.state);
            persist_snapshot(tx, updated)?;
            Ok(db_data)
        }
    }
}

/// Compare observed TOML metadata against DB sync metadata.
///
/// Returns `Ordering::Greater` when TOML is newer than what DB has last seen.
/// Returns `Ordering::Equal` when TOML has the same mtime and sha256 as last seen.
/// Returns `Ordering::Less` when TOML is older than DB's last-seen metadata.
fn toml_compare(state: &VbState, mtime_ns: i64, sha256: &str) -> Ordering {
    match state.toml_last_seen_mtime_ns {
        None => Ordering::Greater,
        Some(last_seen) if mtime_ns > last_seen => Ordering::Greater,
        Some(last_seen) if mtime_ns < last_seen => Ordering::Less,
        Some(_last_seen_is_equal) => match state.toml_last_seen_sha256.as_deref() {
            Some(last_seen_sha256) if last_seen_sha256 == sha256 => Ordering::Equal,
            Some(_) | None => Ordering::Greater,
        },
    }
}

/// Fail loudly when failing to open a database as this really should work, it's just the normal
/// meta-database after all, but have a special case for when the directory doesn't exist which is assumed
/// to be a test.
fn db_handle_from_toml_path(path: &Path) -> anyhow::Result<but_db::DbHandle> {
    let Some(parent) = path.parent() else {
        bail!(
            "Expected TOML path to have a parent directory: {}",
            path.display()
        );
    };

    // Special case: in read-only tests, the TOML file is always authoritative, don't write
    // into read-only fixtures. It's bad we have to do this.
    if path
        .iter()
        .any(|component| component == "generated-do-not-edit")
    {
        return but_db::DbHandle::new_at_path(":memory:");
    }

    match but_db::DbHandle::new_in_directory(parent) {
        Ok(db) => Ok(db),
        Err(err) => {
            tracing::warn!(
                toml_path = %path.display(),
                db_dir = %parent.display(),
                error = ?err,
                "Failed to open virtual-branches DB on disk, using in-memory fallback"
            );
            but_db::DbHandle::new_at_path(":memory:").with_context(|| {
                format!(
                    "Failed to open in-memory fallback DB for TOML at {}",
                    path.display()
                )
            })
        }
    }
}

fn snapshot_state(
    path: &Path,
    tx: &but_db::Transaction<'_>,
) -> anyhow::Result<Option<VirtualBranchesSnapshot>> {
    tx.virtual_branches()
        .get_snapshot()
        .with_context(|| format!("Failed to read VB snapshot from DB near {}", path.display()))
}

/// We own `snapshot` to show it's not supposed to be used afterwards. When that changes, borrow it again.
fn persist_snapshot(
    tx: &mut but_db::Transaction<'_>,
    snapshot: VirtualBranchesSnapshot,
) -> anyhow::Result<()> {
    tx.virtual_branches_mut()?.replace_snapshot(&snapshot)?;
    Ok(())
}

fn write_toml(path: &Path, data: &VirtualBranches) -> anyhow::Result<TomlFileState> {
    let content = toml::to_string(data)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    but_fs::write(path, content.as_bytes())?;
    // Read from a single file descriptor so mtime/hash come from the same file instance.
    let (bytes, metadata) = read_file_bytes_and_metadata(path)?;
    let mtime_ns = file_mtime_ns(&metadata)?;
    Ok(TomlFileState {
        mtime_ns,
        sha256_hexdigest: sha256_hex_hash(&bytes),
    })
}

/// Parse [`VirtualBranches`] from `path` with a lot of error handling.
fn read_toml_info(path: &Path) -> anyhow::Result<TomlInfo> {
    let (bytes, metadata) = match read_file_bytes_and_metadata(path) {
        Ok(out) => out,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(TomlInfo::Missing),
        Err(err) => return Err(err.into()),
    };
    let mtime_ns = file_mtime_ns(&metadata)?;
    let content = match String::from_utf8(bytes.clone()) {
        Ok(content) => content,
        Err(_err) => {
            return Ok(TomlInfo::Invalid);
        }
    };
    match toml::from_str::<VirtualBranches>(&content) {
        Ok(data) => {
            let sha256 = sha256_hex_hash(&bytes);
            Ok(TomlInfo::Parsed(ParsedToml {
                data,
                mtime_ns,
                sha256_hexdigest: sha256,
            }))
        }
        Err(_err) => Ok(TomlInfo::Invalid),
    }
}

/// This should work for the case where the date is before the unix epoch,
/// but it's of course hypothetical.
fn file_mtime_ns(metadata: &std::fs::Metadata) -> anyhow::Result<i64> {
    let modified = metadata.modified()?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .or_else(|_| UNIX_EPOCH.duration_since(modified))
        .context("Could not compute UNIX timestamp for TOML mtime")?;
    i64::try_from(duration.as_nanos()).context("mtime nanos exceed i64 range")
}

fn read_file_bytes_and_metadata(path: &Path) -> std::io::Result<(Vec<u8>, std::fs::Metadata)> {
    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let metadata = file.metadata()?;
    Ok((bytes, metadata))
}

fn sha256_hex_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn legacy_to_snapshot(
    vb: &VirtualBranches,
    DbTomlFileState {
        toml_last_seen_mtime_ns,
        toml_last_seen_sha256,
    }: DbTomlFileState,
) -> anyhow::Result<VirtualBranchesSnapshot> {
    let initialized = true;
    let (
        default_target_remote_name,
        default_target_branch_name,
        default_target_remote_url,
        default_target_sha,
        default_target_push_remote_name,
    ) = vb
        .default_target
        .as_ref()
        .map(|target| {
            (
                Some(target.branch.remote().to_owned()),
                Some(target.branch.branch().to_owned()),
                Some(target.remote_url.clone()),
                Some(target.sha.to_string()),
                target.push_remote_name.clone(),
            )
        })
        .unwrap_or_default();
    let last_pushed_base_sha = vb.last_pushed_base.map(|oid| oid.to_string());

    let mut stacks: Vec<_> = vb.branches.iter().collect();
    stacks.sort_by_key(|(sid, stack)| (stack.order, sid.to_string()));

    let mut out_stacks = Vec::with_capacity(stacks.len());
    let mut out_heads = Vec::new();
    for (stack_id, stack) in stacks {
        let Stack {
            id: _, // should match with `stack_id`, but we effectively normalise on the unique key here
            source_refname,
            upstream,
            order,
            in_workspace,
            workspace_merge_from: _,
            heads,
            #[expect(deprecated)]
            notes,
            #[expect(deprecated)]
            ownership,
            #[expect(deprecated)]
            allow_rebasing,
            #[expect(deprecated)]
            post_commits,
            #[expect(deprecated)]
            tree,
            #[expect(deprecated)]
            created_timestamp_ms,
            #[expect(deprecated)]
            updated_timestamp_ms,
            #[expect(deprecated)]
            name,
            #[expect(deprecated)]
            head,
        } = stack;
        out_stacks.push(VbStack {
            id: stack_id.to_string(),
            source_refname: source_refname.as_ref().map(ToString::to_string),
            upstream_remote_name: upstream.as_ref().map(|up| up.remote().to_owned()),
            upstream_branch_name: upstream.as_ref().map(|up| up.branch().to_owned()),
            sort_order: i64::try_from(*order).context("Stack order exceeds i64")?,
            in_workspace: *in_workspace,
            legacy_name: name.clone(),
            legacy_notes: notes.clone(),
            legacy_ownership: ownership.to_string(),
            legacy_allow_rebasing: *allow_rebasing,
            legacy_post_commits: *post_commits,
            legacy_tree_sha: tree.to_string(),
            legacy_head_sha: head.to_string(),
            legacy_created_timestamp_ms: created_timestamp_ms.to_string(),
            legacy_updated_timestamp_ms: updated_timestamp_ms.to_string(),
        });

        for (position, head) in heads.iter().enumerate() {
            let StackBranch {
                head,
                name,
                pr_number,
                archived,
                review_id,
            } = head;
            out_heads.push(VbStackHead {
                stack_id: stack_id.to_string(),
                position: i64::try_from(position).context("Head position exceeds i64")?,
                name: name.clone(),
                head_sha: head.to_string(),
                pr_number: pr_number.map(|value| value as i64),
                archived: *archived,
                review_id: review_id.clone(),
            });
        }
    }

    let mut out_targets: Vec<_> = vb.branch_targets.iter().collect();
    out_targets.sort_by_key(|(sid, _)| sid.to_string());
    let branch_targets = out_targets
        .into_iter()
        .map(|(stack_id, target)| {
            let Target {
                branch,
                remote_url,
                sha,
                push_remote_name,
            } = target;
            VbBranchTarget {
                stack_id: stack_id.to_string(),
                remote_name: branch.remote().to_owned(),
                branch_name: branch.branch().to_owned(),
                remote_url: remote_url.clone(),
                sha: sha.to_string(),
                push_remote_name: push_remote_name.clone(),
            }
        })
        .collect();

    Ok(VirtualBranchesSnapshot {
        state: VbState {
            default_target_remote_name,
            default_target_branch_name,
            default_target_remote_url,
            default_target_sha,
            default_target_push_remote_name,
            last_pushed_base_sha,
            initialized,
            toml_last_seen_mtime_ns,
            toml_last_seen_sha256,
        },
        stacks: out_stacks,
        heads: out_heads,
        branch_targets,
    })
}

fn snapshot_to_legacy(snapshot: &VirtualBranchesSnapshot) -> anyhow::Result<VirtualBranches> {
    let VirtualBranchesSnapshot {
        state,
        stacks,
        heads,
        branch_targets: snapshot_branch_targets,
    } = snapshot;
    let default_target = match (
        state.default_target_remote_name.as_ref(),
        state.default_target_branch_name.as_ref(),
        state.default_target_remote_url.as_ref(),
        state.default_target_sha.as_ref(),
    ) {
        (Some(remote), Some(branch), Some(remote_url), Some(sha)) => Some(Target {
            branch: RemoteRefname::new(remote, branch),
            remote_url: remote_url.clone(),
            sha: gix::ObjectId::from_str(sha)
                .with_context(|| format!("Invalid default target sha: {sha}"))?,
            push_remote_name: state.default_target_push_remote_name.clone(),
        }),
        _ => None,
    };

    let last_pushed_base = state
        .last_pushed_base_sha
        .as_ref()
        .map(|value| {
            gix::ObjectId::from_str(value)
                .with_context(|| format!("Invalid last_pushed_base sha: {value}"))
        })
        .transpose()?;

    let mut branches = HashMap::new();
    for stack in stacks {
        let VbStack {
            id,
            source_refname,
            upstream_remote_name,
            upstream_branch_name,
            sort_order,
            in_workspace,
            legacy_name,
            legacy_notes,
            legacy_ownership,
            legacy_allow_rebasing,
            legacy_post_commits,
            legacy_tree_sha,
            legacy_head_sha,
            legacy_created_timestamp_ms,
            legacy_updated_timestamp_ms,
        } = stack;
        let stack_id = StackId::from_str(id).with_context(|| format!("Invalid stack id '{id}'"))?;
        let source_refname = source_refname
            .as_ref()
            .map(|name| {
                Refname::from_str(name).with_context(|| format!("Invalid source_refname '{name}'"))
            })
            .transpose()?;
        let upstream = match (upstream_remote_name, upstream_branch_name) {
            (Some(remote), Some(branch)) => Some(RemoteRefname::new(remote, branch)),
            _ => None,
        };

        branches.insert(
            stack_id,
            Stack {
                id: stack_id,
                source_refname,
                upstream,
                order: usize::try_from(*sort_order)
                    .with_context(|| format!("Invalid stack sort order '{sort_order}'"))?,
                in_workspace: *in_workspace,
                workspace_merge_from: None,
                heads: Vec::new(),
                #[expect(deprecated)]
                notes: legacy_notes.clone(),
                #[expect(deprecated)]
                ownership: legacy_ownership
                    .parse()
                    .with_context(|| format!("Invalid ownership claims for '{stack_id}'"))?,
                #[expect(deprecated)]
                allow_rebasing: *legacy_allow_rebasing,
                #[expect(deprecated)]
                post_commits: *legacy_post_commits,
                #[expect(deprecated)]
                tree: gix::ObjectId::from_str(legacy_tree_sha).with_context(|| {
                    format!("Invalid legacy tree sha '{legacy_tree_sha}' for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                created_timestamp_ms: legacy_created_timestamp_ms.parse().with_context(|| {
                    format!("Invalid legacy created timestamp for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                updated_timestamp_ms: legacy_updated_timestamp_ms.parse().with_context(|| {
                    format!("Invalid legacy updated timestamp for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                name: legacy_name.clone(),
                #[expect(deprecated)]
                head: gix::ObjectId::from_str(legacy_head_sha).with_context(|| {
                    format!("Invalid legacy head sha '{legacy_head_sha}' for '{stack_id}'")
                })?,
            },
        );
    }

    for head in heads {
        let VbStackHead {
            stack_id,
            position: _, // previously set based on vec position
            name,
            head_sha,
            pr_number,
            archived,
            review_id,
        } = head;
        let stack_id = StackId::from_str(stack_id)
            .with_context(|| format!("Invalid stack id '{stack_id}'"))?;
        let stack = branches
            .get_mut(&stack_id)
            .ok_or_else(|| anyhow!("Missing stack '{stack_id}' for head '{name}'"))?;
        stack.heads.push(StackBranch {
            head: gix::ObjectId::from_str(head_sha)
                .with_context(|| format!("Invalid head sha '{head_sha}' on '{name}'"))?,
            name: name.clone(),
            pr_number: pr_number
                .map(usize::try_from)
                .transpose()
                .with_context(|| {
                    format!(
                        "Invalid pr_number '{}' on stack '{stack_id}'",
                        pr_number.unwrap_or_default(),
                    )
                })?,
            archived: *archived,
            review_id: review_id.clone(),
        });
    }

    let mut branch_targets = HashMap::new();
    for target in snapshot_branch_targets {
        let VbBranchTarget {
            stack_id,
            remote_name,
            branch_name,
            remote_url,
            sha,
            push_remote_name,
        } = target;
        let stack_id = StackId::from_str(stack_id)
            .with_context(|| format!("Invalid branch_targets stack id '{stack_id}'"))?;
        branch_targets.insert(
            stack_id,
            Target {
                branch: RemoteRefname::new(remote_name, branch_name),
                remote_url: remote_url.clone(),
                sha: gix::ObjectId::from_str(sha).with_context(|| {
                    format!("Invalid branch target sha '{sha}' for '{stack_id}'")
                })?,
                push_remote_name: push_remote_name.clone(),
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

#[expect(clippy::large_enum_variant)]
enum TomlInfo {
    Missing,
    Parsed(ParsedToml),
    Invalid,
}

struct ParsedToml {
    data: VirtualBranches,
    mtime_ns: i64,
    sha256_hexdigest: String,
}

impl ParsedToml {
    fn update_last_seen_metadata_on(&self, state: &mut VbState) {
        state.toml_last_seen_mtime_ns = Some(self.mtime_ns);
        state.toml_last_seen_sha256 = Some(self.sha256_hexdigest.clone());
    }
}

struct TomlFileState {
    mtime_ns: i64,
    sha256_hexdigest: String,
}

impl TomlFileState {
    fn update_last_seen_metadata_on(&self, state: &mut VbState) {
        state.toml_last_seen_mtime_ns = Some(self.mtime_ns);
        state.toml_last_seen_sha256 = Some(self.sha256_hexdigest.clone());
    }
}

struct DbTomlFileState {
    toml_last_seen_mtime_ns: Option<i64>,
    toml_last_seen_sha256: Option<String>,
}

/// Extract just the parts that corelate the db state with the toml file from `db_state`.
fn into_db_toml_file_info(db_state: VbState) -> DbTomlFileState {
    DbTomlFileState {
        toml_last_seen_mtime_ns: db_state.toml_last_seen_mtime_ns,
        toml_last_seen_sha256: db_state.toml_last_seen_sha256,
    }
}
