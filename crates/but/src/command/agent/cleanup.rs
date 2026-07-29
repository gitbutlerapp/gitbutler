//! Best-effort cleanup of retired command syntax inside the managed policy
//! block that `but agent setup` wrote before the syntax changed.
//!
//! The wizard's answers are not persisted, so the full block cannot be
//! re-rendered for existing installs. Instead, the exact machine-written
//! bullet that taught the retired syntax is swapped for its current wording,
//! and only inside the managed markers.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use super::{
    files::{find_line_anchored, managed_block_spans},
    plan::{AgentTarget, join_components},
    policy,
};
use crate::{theme, utils::detect_agent};

/// The commit fast-path bullet as `but agent setup` wrote it before the
/// `but commit` arguments changed. The replacement is derived from the
/// wizard's current wording, so the two can never drift apart.
const RETIRED_FAST_PATH_BULLET: &str = "- For commit just/only/specific changes on a new branch (selected-change requests), use the two-command fast path from the GitButler skill: `but diff`, then `but commit <branch> -c -m \"message\" --changes <id>,<id>`.";

/// The wizard's current wording for the same rule, prefixed the way
/// `write_bullets` renders it.
fn current_bullet() -> String {
    format!("- {}", policy::FAST_PATH_BULLET)
}

/// Rewrite the retired commit bullet in every global instruction file
/// `but agent setup` could have written, and return an agent-facing notice
/// when something changed. Returns `None` when no agent is driving or nothing
/// needed rewriting.
///
/// Infallible by construction: per-file errors are logged and skipped inside
/// `cleanup_files`, and a panic anywhere in the sweep is caught here, so this
/// maintenance pass can never take down the command that triggered it.
pub(crate) fn retired_policy_syntax_notice() -> Option<String> {
    detect_agent::detect()?;
    std::panic::catch_unwind(|| {
        let changed = cleanup_files(&candidate_files());
        (!changed.is_empty()).then(|| notice(&changed))
    })
    .unwrap_or_else(|_| {
        tracing::debug!("retired policy syntax cleanup panicked");
        None
    })
}

/// The per-agent global instruction files under `$HOME` the setup wizard
/// writes. Deliberately NOT the repo-local `AGENTS.md`/`CLAUDE.md` files: those
/// are usually git-tracked, and this sweep runs right before the invoked
/// command, so dirtying them here could sweep the edit into an unrelated
/// `but commit` or block a clean `but pull`. Stale repo files still get fixed
/// by re-running `but agent setup` in that repository.
fn candidate_files() -> Vec<PathBuf> {
    let Some(home) = but_path::home_dir() else {
        return Vec::new();
    };
    AgentTarget::ALL
        .into_iter()
        .filter_map(|agent| agent.global_instruction_components())
        .map(|components| join_components(&home, components))
        .collect()
}

/// Instruction files are kilobytes; anything larger is not a file the setup
/// wizard wrote, so leave it alone rather than rewriting something unexpected.
const MAX_INSTRUCTION_FILE_BYTES: u64 = 1024 * 1024;

fn cleanup_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut changed = Vec::new();
    for path in paths {
        match cleanup_file(path) {
            Ok(true) => changed.push(path.clone()),
            Ok(false) => {}
            Err(err) => {
                tracing::debug!(?err, ?path, "failed to rewrite retired policy syntax");
            }
        }
    }
    changed
}

fn cleanup_file(path: &Path) -> Result<bool> {
    // `metadata` follows symlinks, so size, type, and permissions describe the
    // real file even for a dotfile-managed link.
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    if !metadata.is_file() || metadata.len() > MAX_INSTRUCTION_FILE_BYTES {
        return Ok(false);
    }
    // Non-UTF-8 content errors here, so binary or otherwise unexpected files
    // are skipped instead of being rewritten lossily.
    let content = std::fs::read_to_string(path)?;
    let Some(updated) = replace_retired_bullet(&content) else {
        return Ok(false);
    };
    // Only now, on the rare rewrite path, resolve symlinks so the write below
    // edits the link's target; renaming over the path directly would swap the
    // symlink itself for a regular file.
    replace_file_contents(&path.canonicalize()?, &content, &updated, &metadata)?;
    Ok(true)
}

/// Write via a temp file in the same directory and rename it over `path`, so
/// a crash or full disk never leaves a truncated instruction file behind. The
/// rewrite is idempotent, so two concurrent `but` invocations converge on the
/// same corrected content.
fn replace_file_contents(
    path: &Path,
    read_content: &str,
    new_content: &str,
    original: &std::fs::Metadata,
) -> Result<()> {
    use std::io::Write as _;
    let dir = path
        .parent()
        .with_context(|| format!("No parent directory for {}", path.display()))?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(new_content.as_bytes())?;
    // Temp files default to owner-only permissions; keep the original mode.
    temp.as_file().set_permissions(original.permissions())?;
    temp.as_file().sync_all()?;
    // The rename below replaces the whole file, so an edit that landed after
    // our read would be silently thrown away with it. Back off if the content
    // moved under us — a later command re-runs the sweep on the fresh content.
    if std::fs::read_to_string(path)? != read_content {
        anyhow::bail!(
            "{} changed while preparing the rewrite; leaving it alone",
            path.display()
        );
    }
    temp.persist(path)
        .with_context(|| format!("Failed to replace {}", path.display()))?;
    Ok(())
}

/// Replace every line-anchored occurrence of the retired bullet that sits
/// inside a managed block. Returns `None` when nothing needs rewriting.
fn replace_retired_bullet(content: &str) -> Option<String> {
    // The cheap check first: on the steady-state clean file this skips the
    // marker scan and allocation entirely.
    if !content.contains(RETIRED_FAST_PATH_BULLET) {
        return None;
    }
    // A malformed block (unmatched marker) is not ours to interpret: skip.
    let spans = managed_block_spans(content).ok()?;
    let current = current_bullet();
    let mut updated = String::with_capacity(content.len());
    let mut copied = 0;
    let mut pos = 0;
    while let Some(idx) = find_line_anchored(content, RETIRED_FAST_PATH_BULLET, pos) {
        pos = idx + RETIRED_FAST_PATH_BULLET.len();
        if !spans.iter().any(|span| span.contains(&idx)) {
            continue;
        }
        updated.push_str(&content[copied..idx]);
        updated.push_str(&current);
        copied = pos;
    }
    (copied > 0).then(|| {
        updated.push_str(&content[copied..]);
        updated
    })
}

/// The agent that triggered the cleanup loaded the stale rules at session
/// start, so the notice restates the current syntax instead of only pointing
/// at the rewritten files.
fn notice(changed: &[PathBuf]) -> String {
    let t = theme::get();
    let paths = changed
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} Retired `but commit` syntax was removed from your GitButler workflow rules ({paths}).\n\
         The rule is now: {}\n\
         Follow the updated syntax; the rules text loaded into your context is outdated.",
        t.sym().success,
        policy::FAST_PATH_BULLET,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        super::{MANAGED_BLOCK_END, MANAGED_BLOCK_START},
        *,
    };

    fn managed(body: &str) -> String {
        format!("# Rules\n\n{MANAGED_BLOCK_START}\n{body}\n{MANAGED_BLOCK_END}\n")
    }

    #[test]
    fn rewrites_retired_bullet_inside_managed_block() {
        let content = managed(&format!("## Version control\n\n{RETIRED_FAST_PATH_BULLET}"));
        let updated = replace_retired_bullet(&content).unwrap();
        assert_eq!(
            updated,
            managed(&format!("## Version control\n\n{}", current_bullet())),
            "only the bullet changes; the rest of the file is byte-identical"
        );
    }

    #[test]
    fn leaves_retired_bullet_outside_managed_block_alone() {
        let content = format!("# Notes\n\n{RETIRED_FAST_PATH_BULLET}\n");
        assert!(
            replace_retired_bullet(&content).is_none(),
            "user-authored text outside the markers is not ours to edit"
        );
    }

    #[test]
    fn leaves_current_bullet_alone() {
        let content = managed(&current_bullet());
        assert!(replace_retired_bullet(&content).is_none());
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let content = managed(RETIRED_FAST_PATH_BULLET).replace('\n', "\r\n");
        let updated = replace_retired_bullet(&content).unwrap();
        assert_eq!(
            updated,
            managed(&current_bullet()).replace('\n', "\r\n"),
            "the bullet itself has no line ending, so CRLF endings survive"
        );
    }

    #[test]
    fn cleanup_files_rewrites_and_reports_only_changed_files() {
        let dir = tempfile::tempdir().unwrap();
        let stale = dir.path().join("stale.md");
        let current = dir.path().join("current.md");
        let missing = dir.path().join("missing.md");
        std::fs::write(&stale, managed(RETIRED_FAST_PATH_BULLET)).unwrap();
        std::fs::write(&current, managed(&current_bullet())).unwrap();

        let changed = cleanup_files(&[stale.clone(), current.clone(), missing]);

        assert_eq!(changed, vec![stale.clone()]);
        assert_eq!(
            std::fs::read_to_string(&stale).unwrap(),
            managed(&current_bullet())
        );
    }

    #[test]
    fn skips_oversized_and_binary_files() {
        let dir = tempfile::tempdir().unwrap();
        let oversized = dir.path().join("oversized.md");
        let mut content = managed(RETIRED_FAST_PATH_BULLET);
        content.push_str(&"x".repeat(MAX_INSTRUCTION_FILE_BYTES as usize));
        std::fs::write(&oversized, &content).unwrap();
        let binary = dir.path().join("binary.md");
        let mut bytes = managed(RETIRED_FAST_PATH_BULLET).into_bytes();
        bytes.push(0xff);
        std::fs::write(&binary, &bytes).unwrap();

        let changed = cleanup_files(&[oversized.clone(), binary.clone()]);

        assert_eq!(
            changed,
            Vec::<PathBuf>::new(),
            "files the wizard cannot have written stay untouched"
        );
        assert_eq!(std::fs::read_to_string(&oversized).unwrap(), content);
        assert_eq!(std::fs::read(&binary).unwrap(), bytes);
    }

    #[cfg(unix)]
    #[test]
    fn rewrites_symlink_target_and_keeps_the_link() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("dotfiles-repo.md");
        let link = dir.path().join("rules.md");
        std::fs::write(&target, managed(RETIRED_FAST_PATH_BULLET)).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let changed = cleanup_files(std::slice::from_ref(&link));

        assert_eq!(changed, vec![link.clone()]);
        assert!(
            std::fs::symlink_metadata(&link).unwrap().is_symlink(),
            "a dotfile-managed rules file must stay a symlink"
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            managed(&current_bullet())
        );
    }

    #[test]
    fn backs_off_when_the_file_changed_after_the_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.md");
        let concurrent_edit = format!(
            "{}\nA line saved by an editor.\n",
            managed(&current_bullet())
        );
        std::fs::write(&path, &concurrent_edit).unwrap();
        let metadata = std::fs::metadata(&path).unwrap();

        let stale_read = managed(RETIRED_FAST_PATH_BULLET);
        let result =
            replace_file_contents(&path, &stale_read, &managed(&current_bullet()), &metadata);

        assert!(result.is_err(), "a rewrite from a stale read must back off");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concurrent_edit,
            "the concurrent edit must survive untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn preserves_file_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.md");
        std::fs::write(&path, managed(RETIRED_FAST_PATH_BULLET)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let changed = cleanup_files(std::slice::from_ref(&path));

        assert_eq!(changed, vec![path.clone()]);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644,
            "the rewrite must not tighten the file to the temp file's owner-only mode"
        );
    }
}
