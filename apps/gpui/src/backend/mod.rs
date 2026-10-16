//! The only part of the app that talks to GitButler. Everything here is blocking and
//! meant to run on a background thread; results are plain [`model`] data.

pub mod askpass;
mod highlight;
pub mod model;
mod ops;
pub mod watch;

use std::path::Path;

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_ctx::Context;

pub use model::*;
pub use ops::*;

/// Process-wide setup, before any other call.
pub fn init() {
    // Forge tokens live in the keychain under the app's identifier.
    but_api::platform::init_application_namespace(None).ok();
}

pub fn list_projects() -> Result<Vec<Project>> {
    let mut projects: Vec<Project> = but_api::legacy::projects::list_projects_stateless()?
        .into_iter()
        .filter_map(|entry| {
            let project: gitbutler_project::Project = entry.inner.into();
            let git_dir = project.git_dir();
            let path = if git_dir.ends_with(".git") {
                git_dir.parent()?.to_owned()
            } else {
                git_dir.to_owned()
            };
            path.exists().then(|| Project {
                id: project.id.to_string(),
                name: project.title.clone(),
                path,
            })
        })
        .collect();
    projects.sort_by_key(|project| project.name.to_lowercase());
    Ok(projects)
}

/// Register the repository at `path` with GitButler, so it shows up in [`list_projects()`].
pub fn add_project(path: &Path) -> Result<()> {
    use gitbutler_project::AddProjectOutcome;
    match but_api::legacy::projects::add_project_best_effort(path.to_owned())? {
        AddProjectOutcome::Added(_) | AddProjectOutcome::AlreadyExists(_) => Ok(()),
        other => anyhow::bail!("Can't add {}: {other}", path.display()),
    }
}

/// Get a project ready to show: the first time, GitButler sets up its workspace
/// metadata, like which branch is the target.
pub fn activate(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::legacy::projects::prepare_project_for_activation(&mut ctx)
}

pub(crate) fn open(path: &Path) -> Result<Context> {
    Context::open(path).with_context(|| format!("Couldn't open {}", path.display()))
}

pub fn load_workspace(path: &Path) -> Result<Workspace> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let uncommitted = {
        let repo = ctx.repo.get()?;
        but_core::diff::worktree_changes(&repo)?
            .changes
            .into_iter()
            .map(file_change)
            .collect()
    };

    let stacks = info
        .stacks
        .iter()
        .map(|stack| Stack {
            branches: stack.segments.iter().map(branch_from).collect(),
        })
        .collect();
    let target = info.target_ref.as_ref().map(|target| Target {
        name: target.ref_name.shorten().to_string(),
        behind: target.commits_ahead,
        needs_update: !info.is_target_current || info.has_lanes_behind_target,
    });
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "loaded workspace");
    Ok(Workspace {
        target,
        stacks,
        uncommitted,
    })
}

fn branch_from(segment: &but_workspace::ref_info::Segment) -> Branch {
    let (name, ref_name) = match segment.ref_name() {
        Some(name) => (name.shorten().to_string(), name.to_string()),
        None => ("Unnamed branch".into(), String::new()),
    };
    use but_core::ui::PushStatus as P;
    let push = match segment.push_status {
        P::NothingToPush => PushState::NothingToPush,
        P::UnpushedCommits => PushState::Unpushed,
        P::UnpushedCommitsRequiringForce => PushState::NeedsForce,
        P::CompletelyUnpushed => PushState::NeverPushed,
        P::Integrated => PushState::Integrated,
    };
    Branch {
        name,
        ref_name,
        commits: segment.commits.iter().map(commit_from).collect(),
        upstream_only: segment.commits_on_remote.len(),
        push,
        review: segment
            .metadata
            .as_ref()
            .and_then(|meta| meta.review.pull_request)
            .map(|number| Review {
                number: number as u64,
            }),
    }
}

fn commit_from(local: &but_workspace::ref_info::LocalCommit) -> Commit {
    use but_workspace::ref_info::LocalCommitRelation as R;
    let inner = &local.inner;
    let message = inner.message.to_str_lossy().trim_end().to_string();
    Commit {
        id: CommitId(inner.id),
        title: message.lines().next().unwrap_or_default().to_string(),
        message,
        author: inner.author.name.to_str_lossy().into_owned(),
        time: inner.author.time.seconds,
        state: match local.relation {
            R::LocalOnly => CommitState::Local,
            R::LocalAndRemote(_) => CommitState::Pushed,
            R::Integrated(_) => CommitState::Integrated,
        },
        conflicted: inner.has_conflicts,
    }
}

fn file_change(change: but_core::TreeChange) -> FileChange {
    use but_core::TreeStatus as S;
    let (kind, previous_path) = match &change.status {
        S::Addition { .. } => (ChangeKind::Added, None),
        S::Deletion { .. } => (ChangeKind::Deleted, None),
        S::Modification { .. } => (ChangeKind::Modified, None),
        S::Rename { previous_path, .. } => (
            ChangeKind::Renamed,
            Some(previous_path.to_str_lossy().into_owned()),
        ),
    };
    FileChange {
        path: change.path.to_str_lossy().into_owned(),
        previous_path,
        kind,
        change,
    }
}

/// What a diff is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffSource {
    Worktree,
    Commit(CommitId),
    /// Everything from `oldest` (inclusive) up to `newest`, like a whole branch.
    Range {
        oldest: CommitId,
        newest: CommitId,
    },
}

pub fn load_diff(path: &Path, source: DiffSource) -> Result<Vec<FileDiff>> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let changes: Vec<FileChange> = match source {
        DiffSource::Worktree => but_core::diff::worktree_changes(&repo)?
            .changes
            .into_iter()
            .map(file_change)
            .collect(),
        DiffSource::Commit(id) => {
            let details =
                but_api::diff::commit_details(&ctx, id.0, but_api::diff::ComputeLineStats::No)?;
            details
                .diff_with_first_parent
                .into_iter()
                .map(file_change)
                .collect()
        }
        DiffSource::Range { oldest, newest } => {
            let base = repo
                .find_commit(oldest.0)?
                .parent_ids()
                .next()
                .map(|id| id.detach());
            but_core::diff::tree_changes(&repo, base, newest.0)?
                .into_iter()
                .map(file_change)
                .collect()
        }
    };
    let context_lines = ctx.settings.context_lines;
    let mut diffs = changes
        .into_iter()
        .map(|file| {
            let body = match file.change.unified_patch(&repo, context_lines)? {
                None => DiffBody::Empty,
                Some(but_core::UnifiedPatch::Binary) => DiffBody::Binary,
                Some(but_core::UnifiedPatch::TooLarge { size_in_bytes }) => DiffBody::TooLarge {
                    bytes: size_in_bytes,
                },
                Some(but_core::UnifiedPatch::Patch {
                    hunks,
                    lines_added,
                    lines_removed,
                    ..
                }) => DiffBody::Hunks {
                    hunks: hunks.iter().map(parse_hunk).collect(),
                    added: lines_added,
                    removed: lines_removed,
                },
            };
            Ok(FileDiff { file, body })
        })
        .collect::<Result<Vec<_>>>()?;
    highlight_all(&mut diffs);
    tracing::debug!(elapsed = ?started.elapsed(), ?source, "loaded diff");
    Ok(diffs)
}

/// Files with more changed lines than this aren't highlighted, to keep big diffs fast.
const HIGHLIGHT_LIMIT: u32 = 4000;

/// Highlight every file, spread over all cores: it costs far more than the diff itself.
fn highlight_all(diffs: &mut [FileDiff]) {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let files: Vec<std::sync::Mutex<&mut FileDiff>> =
        diffs.iter_mut().map(std::sync::Mutex::new).collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(files.len());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(file) =
                    files.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    let mut file = file.lock().expect("each file is taken once");
                    let path = file.file.path.clone();
                    let DiffBody::Hunks {
                        hunks,
                        added,
                        removed,
                    } = &mut file.body
                    else {
                        continue;
                    };
                    if *added + *removed > HIGHLIGHT_LIMIT {
                        continue;
                    }
                    for hunk in hunks {
                        let Some(mut highlighter) = highlight::Highlighter::for_path(&path) else {
                            break;
                        };
                        for line in &mut hunk.lines {
                            line.spans = highlighter.line(&line.text);
                        }
                    }
                }
            });
        }
    });
}

/// Files with more changed lines than this aren't highlighted, to keep big diffs fast.
fn parse_hunk(hunk: &but_core::unified_diff::DiffHunk) -> Hunk {
    let mut old = hunk.old_start;
    let mut new = hunk.new_start;
    let mut lines = Vec::new();
    let mut header = String::new();
    for raw in hunk.diff.lines() {
        let text = raw.to_str_lossy();
        let (kind, rest) = match raw.first() {
            Some(b'@') => {
                header = text.into_owned();
                continue;
            }
            Some(b'+') => (LineKind::Added, &text[1..]),
            Some(b'-') => (LineKind::Removed, &text[1..]),
            Some(b' ') => (LineKind::Context, &text[1..]),
            // "\ No newline at end of file"
            Some(b'\\') => continue,
            _ => (LineKind::Context, &*text),
        };
        let (old_no, new_no) = match kind {
            LineKind::Context => (Some(old), Some(new)),
            LineKind::Added => (None, Some(new)),
            LineKind::Removed => (Some(old), None),
        };
        match kind {
            LineKind::Context => {
                old += 1;
                new += 1;
            }
            LineKind::Added => new += 1,
            LineKind::Removed => old += 1,
        }
        lines.push(DiffLine {
            kind,
            old: old_no,
            new: new_no,
            text: rest.trim_end_matches('\r').replace('\t', "    "),
            spans: Vec::new(),
        });
    }
    Hunk {
        key: HunkKey(hunk.into()),
        header,
        lines,
    }
}
