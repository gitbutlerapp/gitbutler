//! The only part of the app that talks to GitButler. Everything here is blocking and
//! meant to run on a background thread; results are plain [`model`] data.

pub mod accounts;
pub mod ai;
mod annotate;
pub mod askpass;
pub mod branches;
pub mod conflicts;
mod highlight;
pub mod history;
pub mod model;
mod ops;
pub mod reviews;
pub mod settings;
pub mod upstream;
pub mod watch;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

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
            // Kept when the folder is gone, so the app can say so and offer to find it.
            Some(Project {
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
    let uncommitted = worktree_files(&*ctx.repo.get()?)?;

    let mut stacks: Vec<Stack> = info
        .stacks
        .iter()
        .map(|stack| Stack {
            branches: stack.segments.iter().map(branch_from).collect(),
        })
        .collect();
    describe_reviews(&ctx, &mut stacks);
    let target = info.target_ref.as_ref().map(|target| Target {
        name: target.ref_name.shorten().to_string(),
        behind: target.commits_ahead,
        needs_update: !info.is_target_current || info.has_lanes_behind_target,
    });
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "loaded workspace");
    Ok(Workspace {
        target,
        forge: reviews::forge_name(&ctx),
        stacks,
        uncommitted,
    })
}

/// Just the uncommitted files, for when only the worktree changed.
pub fn load_uncommitted(path: &Path) -> Result<Vec<FileChange>> {
    let ctx = open(path)?;
    worktree_files(&*ctx.repo.get()?)
}

/// Fill in what the forge cache knows about each branch's pull request: its title, state,
/// link and CI. Reads only the cache; [`refresh_forge`] is what asks the forge.
fn describe_reviews(ctx: &Context, stacks: &mut [Stack]) {
    let reviews: HashMap<u64, but_forge::ForgeReview> = ctx
        .db
        .get_cache()
        .ok()
        .and_then(|db| but_forge::list_cached_forge_reviews(&db).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|review| (review.number as u64, review))
        .collect();
    let branches = stacks.iter_mut().flat_map(|stack| &mut stack.branches);
    for branch in branches {
        let Some(review) = &mut branch.review else {
            continue;
        };
        if let Some(cached) = reviews.get(&review.number) {
            review.title = cached.title.clone();
            review.url = Some(cached.html_url.clone());
            review.state = review_state(cached);
        }
        // A merged pull request drops out of the open ones the forge lists.
        if branch.push == PushState::Integrated {
            review.state = ReviewState::Merged;
        }
        let cache_only = Some(but_forge::CacheConfig::CacheOnly);
        let checks = but_api::legacy::forge::list_ci_checks_for_ref(ctx, &branch.name, cache_only)
            .unwrap_or_default();
        review.checks = (!checks.is_empty()).then(|| summarize_checks(&checks));
    }
}

/// Images bigger than this show as a binary file, rather than spend the memory.
const IMAGE_LIMIT: usize = 20_000_000;

/// Both sides of an image file, when `file` is one GPUI can draw.
fn image_body(repo: &gix::Repository, file: &FileChange) -> Option<DiffBody> {
    use gpui::ImageFormat as F;
    let extension = file.path.rsplit_once('.')?.1.to_ascii_lowercase();
    let format = match extension.as_str() {
        "png" => F::Png,
        "jpg" | "jpeg" => F::Jpeg,
        "gif" => F::Gif,
        "webp" => F::Webp,
        "bmp" => F::Bmp,
        "tif" | "tiff" => F::Tiff,
        _ => return None,
    };
    // A side that isn't in the object database is the file in the worktree.
    let bytes = |state: but_core::ChangeState, path: &str| -> Option<Vec<u8>> {
        let data = if state.id.is_null() {
            std::fs::read(repo.workdir()?.join(path)).ok()?
        } else {
            repo.find_blob(state.id).ok()?.data.clone()
        };
        (data.len() <= IMAGE_LIMIT).then_some(data)
    };
    let image = |data: Vec<u8>| std::sync::Arc::new(gpui::Image::from_bytes(format, data));
    let status = &file.change.status;
    let before = status
        .previous_state_and_path()
        .and_then(|(state, old_path)| {
            let path = old_path.map_or_else(|| file.path.clone(), |p| p.to_string());
            bytes(state, &path).map(image)
        });
    let after = status
        .state()
        .and_then(|state| bytes(state, &file.path).map(image));
    (before.is_some() || after.is_some()).then_some(DiffBody::Image { before, after })
}

fn review_state(review: &but_forge::ForgeReview) -> ReviewState {
    match (&review.merged_at, &review.closed_at, review.draft) {
        (Some(_), _, _) => ReviewState::Merged,
        (None, Some(_), _) => ReviewState::Closed,
        (None, None, true) => ReviewState::Draft,
        (None, None, false) => ReviewState::Open,
    }
}

fn summarize_checks(checks: &[but_forge::CiCheck]) -> Checks {
    use but_forge::{CiConclusion as C, CiStatus as S};
    checks.iter().fold(Checks::default(), |mut sum, check| {
        match &check.status {
            S::Complete { conclusion, .. } => match conclusion {
                C::Success | C::Neutral | C::Skipped => sum.passed += 1,
                C::Failure | C::TimedOut | C::Cancelled | C::ActionRequired | C::Unknown => {
                    sum.failed += 1
                }
            },
            S::InProgress | S::Queued | S::Unknown => sum.running += 1,
        }
        sum
    })
}

/// Ask the forge about the project's pull requests, refreshing the cache that
/// [`load_workspace`] reads. Fails when the project has no forge or no account for it.
pub fn refresh_reviews(path: &Path) -> Result<()> {
    let started = std::time::Instant::now();
    tracing::debug!(path = %path.display(), "refreshing reviews");
    let ctx = open(path)?;
    but_api::legacy::forge::list_reviews(&ctx, Some(but_forge::CacheConfig::NoCache))?;
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "refreshed reviews");
    Ok(())
}

/// Ask the forge about the CI of each branch with a pull request, a request per branch.
pub fn refresh_checks(path: &Path) -> Result<()> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    but_api::legacy::forge::warm_ci_checks_cache(&ctx)?;
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "refreshed checks");
    Ok(())
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
                title: String::new(),
                state: ReviewState::Open,
                url: None,
                checks: None,
            }),
    }
}

fn commit_from(local: &but_workspace::ref_info::LocalCommit) -> Commit {
    use but_workspace::ref_info::LocalCommitRelation as R;
    let inner = &local.inner;
    let message = inner.message.to_str_lossy().trim_end().to_string();
    Commit {
        id: CommitId(inner.id),
        key: inner
            .change_id
            .as_ref()
            .map_or_else(|| inner.id.to_string(), ToString::to_string),
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
        modified: None,
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
    /// What a branch outside the workspace changes: from where it forked off the target to
    /// its tip `tip`, as its pull request would show.
    Branch {
        tip: CommitId,
    },
    /// What the operation that snapshot `id` was taken for changed: up to the snapshot
    /// after it, `newer`, or to now.
    Snapshot {
        id: SnapshotId,
        newer: Option<SnapshotId>,
    },
}

/// Uncommitted files, each with when it was last written.
fn worktree_files(repo: &gix::Repository) -> Result<Vec<FileChange>> {
    let changes = but_core::diff::worktree_changes(repo)?;
    let ui_changes: but_core::ui::WorktreeChanges = changes.clone().into();
    let modified = but_core::diff::ui::modification_times(repo, &ui_changes);
    Ok(changes
        .changes
        .into_iter()
        .map(|change| {
            let mut file = file_change(change);
            file.modified = modified.get(&file.path).map(|ms| *ms as i64);
            file
        })
        .collect())
}

/// The diff of `source`. A worktree diff takes the `worktree` files the workspace just
/// listed, rather than looking at the worktree a second time. Files whose patch is the same
/// as in `previous` keep their highlighting, which is most of what loading costs.
pub fn load_diff(
    path: &Path,
    source: DiffSource,
    worktree: Option<Vec<FileChange>>,
    previous: Option<Arc<Vec<FileDiff>>>,
    annotate: Option<usize>,
) -> Result<Vec<FileDiff>> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let changes: Vec<FileChange> = match source {
        // With modification times, so review marks match the workspace's file list.
        DiffSource::Worktree => match worktree {
            Some(files) => files,
            None => worktree_files(&repo)?,
        },
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
        DiffSource::Branch { tip } => {
            let target = ctx
                .project_meta()?
                .target_ref
                .and_then(|name| repo.find_reference(name.as_ref()).ok())
                .and_then(|mut target| target.peel_to_id().ok())
                .map(|id| id.detach());
            let base = target
                .and_then(|target| repo.merge_base(tip.0, target).ok().flatten())
                .map(|id| id.detach());
            but_core::diff::tree_changes(&repo, base, tip.0)?
                .into_iter()
                .map(file_change)
                .collect()
        }
        DiffSource::Snapshot { id, newer } => {
            but_api::legacy::oplog::snapshot_diff(&ctx, id.0, newer.map(|newer| newer.0))?
                .into_iter()
                .map(|change| file_change(change.into()))
                .collect()
        }
    };
    let context_lines = ctx.settings.context_lines;
    let previous: HashMap<&str, &FileDiff> = previous
        .iter()
        .flat_map(|files| files.iter())
        .map(|file| (file.file.path.as_str(), file))
        .collect();
    // Only the first `annotate` files are highlighted and given scope names; the rest stay
    // plain unless reused.
    let first = annotate.unwrap_or(usize::MAX);
    let repo = repo.clone().into_sync();
    let diffs = parallel_map(
        changes,
        || repo.to_thread_local(),
        |repo, index, file| {
            let previous = previous.get(file.path.as_str()).copied();
            load_file(repo, file, context_lines, previous, index < first)
        },
    )
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    tracing::debug!(elapsed = ?started.elapsed(), ?source, "loaded diff");
    Ok(diffs)
}

/// One file's diff, reused from `previous` when its patch is the same, else parsed and,
/// when `annotate`, highlighted and given scope names.
fn load_file(
    repo: &gix::Repository,
    file: FileChange,
    context_lines: u32,
    previous: Option<&FileDiff>,
    annotate: bool,
) -> Result<FileDiff> {
    // An uncommitted file last written at the same moment, against the same base, has the
    // same patch, so it isn't computed again, much as git skips files whose stat is unchanged.
    if let Some(old) = previous.filter(|old| {
        file.modified.is_some()
            && old.file.modified == file.modified
            && old.file.kind == file.kind
            && old.file.previous_path == file.previous_path
            && old.file.base() == file.base()
            && (old.annotated || !annotate)
    }) {
        return Ok(FileDiff {
            file,
            body: old.body.clone(),
            fingerprint: old.fingerprint,
            annotated: old.annotated,
        });
    }
    let mut filter = but_core::unified_diff::filter_from_state(
        repo,
        file.change.status.state(),
        but_core::UnifiedPatch::CONVERSION_MODE,
    )?;
    let patch = file
        .change
        .unified_patch_with_filter(repo, context_lines, &mut filter)?;
    let fingerprint = fingerprint(&patch);
    if let Some(old) = previous.filter(|old| old.fingerprint == fingerprint && old.annotated) {
        return Ok(FileDiff {
            file,
            body: old.body.clone(),
            fingerprint,
            annotated: true,
        });
    }
    let mut body = match patch {
        None => DiffBody::Empty,
        Some(but_core::UnifiedPatch::Binary) => image_body(repo, &file).unwrap_or(DiffBody::Binary),
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
    if annotate
        && let DiffBody::Hunks {
            hunks,
            added,
            removed,
        } = &mut body
        && *added + *removed <= HIGHLIGHT_LIMIT
    {
        for hunk in hunks.iter_mut() {
            annotate::emphasize(hunk);
            let Some(mut highlighter) = highlight::Highlighter::for_path(&file.path) else {
                continue;
            };
            for line in &mut hunk.lines {
                line.spans = highlighter.line(&line.text);
            }
        }
        if let Some((old, new)) = scope_sides(&filter) {
            annotate::label_scopes(hunks, &old, &new, &file.path);
        }
    }
    Ok(FileDiff {
        file,
        body,
        fingerprint,
        annotated: annotate,
    })
}

/// The new side of `file` as diff lines, numbered on that side, for showing the unchanged
/// lines around its hunks. `None` when it has no new side to show, like a deletion.
pub fn file_lines(path: &Path, file: &FileChange) -> Result<Option<Vec<DiffLine>>> {
    let Some(state) = file.change.status.state() else {
        return Ok(None);
    };
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let bytes = if state.id.is_null() {
        let workdir = repo.workdir().context("the repository has no worktree")?;
        std::fs::read(workdir.join(&file.path))?
    } else {
        repo.find_blob(state.id)?.data.clone()
    };
    if bytes.contains(&0) {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut highlighter = (text.len() <= WHOLE_HIGHLIGHT_LIMIT)
        .then(|| highlight::Highlighter::for_path(&file.path))
        .flatten();
    Ok(Some(
        text.lines()
            .enumerate()
            .map(|(ix, raw)| {
                let text = display_text(raw);
                let spans = highlighter
                    .as_mut()
                    .map(|h| h.line(&text))
                    .unwrap_or_default();
                DiffLine {
                    kind: LineKind::Context,
                    old: None,
                    new: Some(ix as u32 + 1),
                    text: text.into(),
                    spans,
                    emph: Vec::new(),
                }
            })
            .collect(),
    ))
}

/// Files shown whole are highlighted up to this size in bytes.
const WHOLE_HIGHLIGHT_LIMIT: usize = 512 * 1024;

/// `f` over `items` on all cores, keeping their order. Each thread gets its own `state`.
fn parallel_map<T: Send, S, R: Send>(
    items: Vec<T>,
    state: impl Fn() -> S + Sync,
    f: impl Fn(&S, usize, T) -> R + Sync,
) -> Vec<R> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(items.len());
    let next = AtomicUsize::new(0);
    let items: Vec<Mutex<Option<T>>> = items
        .into_iter()
        .map(|item| Mutex::new(Some(item)))
        .collect();
    let results: Vec<Mutex<Option<R>>> = items.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let state = state();
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let item = item.lock().expect("each item is taken once").take();
                    let result = f(&state, index, item.expect("each item is taken once"));
                    *results[index].lock().expect("each result is set once") = Some(result);
                }
            });
        }
    });
    results
        .into_iter()
        .map(|result| {
            result
                .into_inner()
                .expect("each result is set once")
                .expect("every item is mapped")
        })
        .collect()
}

/// A hash of a file's patch, hunk headers included, so two equal ones have the same lines.
fn fingerprint(patch: &Option<but_core::UnifiedPatch>) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match patch {
        Some(but_core::UnifiedPatch::Patch { hunks, .. }) => {
            hunks.iter().for_each(|hunk| hunk.diff.hash(&mut hasher))
        }
        Some(but_core::UnifiedPatch::TooLarge { size_in_bytes }) => size_in_bytes.hash(&mut hasher),
        Some(but_core::UnifiedPatch::Binary) => "binary".hash(&mut hasher),
        None => "empty".hash(&mut hasher),
    }
    hasher.finish()
}

/// Files bigger than this, both sides together, aren't parsed for scope names: that
/// parses all of both sides, however small the change.
const SCOPE_LIMIT: usize = 1 << 20;

/// Both sides of a text change the patch was computed from, if small enough to parse.
fn scope_sides(filter: &gix::diff::blob::Platform) -> Option<(String, String)> {
    let (old, new) = filter.resources()?;
    let (old, new) = (side_bytes(old.data)?, side_bytes(new.data)?);
    (old.len() + new.len() <= SCOPE_LIMIT).then(|| {
        let text = |bytes| String::from_utf8_lossy(bytes).into_owned();
        (text(old), text(new))
    })
}

/// One side of a change: empty when it doesn't exist, `None` when binary.
fn side_bytes(data: gix::diff::blob::platform::resource::Data<'_>) -> Option<&[u8]> {
    use gix::diff::blob::platform::resource::Data;
    match data {
        Data::Missing => Some(&[]),
        Data::Buffer { buf, .. } => Some(buf),
        Data::Binary { .. } => None,
    }
}

/// Files with more changed lines than this aren't highlighted, to keep big diffs fast.
const HIGHLIGHT_LIMIT: u32 = 4000;

/// Lines longer than this, as in minified files, are cut short: shaping and highlighting
/// them costs far more than anyone reads.
const LINE_LIMIT: usize = 2000;

/// A diff line as shown: tabs as spaces, no carriage return, and cut at [`LINE_LIMIT`].
fn display_text(raw: &str) -> String {
    let mut text = raw.trim_end_matches('\r').replace('\t', "    ");
    if text.len() > LINE_LIMIT {
        let cut = (0..=LINE_LIMIT)
            .rev()
            .find(|ix| text.is_char_boundary(*ix))
            .unwrap_or(0);
        text.truncate(cut);
        text.push('…');
    }
    text
}

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
            text: display_text(rest).into(),
            spans: Vec::new(),
            emph: Vec::new(),
        });
    }
    Hunk {
        key: HunkKey(hunk.into()),
        header,
        lines,
        scope: Vec::new(),
    }
}
