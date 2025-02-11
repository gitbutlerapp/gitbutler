use but_core::TreeStatus;
use but_workspace::commit_engine::{Destination, DiffSpec};
use gix::prelude::ObjectIdExt;
use gix_testtools::Creation;

pub const CONTEXT_LINES: u32 = 0;

/// Sets and environment that assures commits are reproducible.
/// This needs the `testing` feature enabled in `but-core` as well to work.
/// This changes the process environment, be aware.
pub fn assure_stable_env() {
    let env = gix_testtools::Env::new()
        .set("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000")
        .set("GIT_AUTHOR_EMAIL", "author@example.com")
        .set("GIT_AUTHOR_NAME", "author")
        .set("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000")
        .set("GIT_COMMITTER_EMAIL", "committer@example.com")
        .set("GIT_COMMITTER_NAME", "committer")
        .set("CHANGE_ID", "committer");
    // assure it doesn't get racy.
    std::mem::forget(env);
}

fn writable_scenario_inner(
    name: &str,
    creation: Creation,
) -> anyhow::Result<(gix::Repository, tempfile::TempDir)> {
    let tmp = gix_testtools::scripted_fixture_writable_with_args(
        format!("scenario/{name}.sh"),
        None::<String>,
        creation,
    )
    .map_err(anyhow::Error::from_boxed)?;
    let mut options = gix::open::Options::isolated();
    options.permissions.env = gix::open::permissions::Environment::all();
    let repo = gix::open_opts(tmp.path(), options)?;
    Ok((repo, tmp))
}

/// Provide a scenario but assure the returned repository will write objects to memory.
pub fn read_only_in_memory_scenario(name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only(format!("scenario/{name}.sh"))
        .map_err(anyhow::Error::from_boxed)?;
    let mut options = gix::open::Options::isolated();
    options.permissions.env = gix::open::permissions::Environment::all();
    let repo = gix::open_opts(root, options)?.with_object_memory();
    Ok(repo)
}

pub fn writable_scenario(name: &str) -> (gix::Repository, tempfile::TempDir) {
    writable_scenario_inner(name, Creation::CopyFromReadOnly)
        .expect("fixtures will yield valid repositories")
}

pub fn writable_scenario_execute(name: &str) -> (gix::Repository, tempfile::TempDir) {
    writable_scenario_inner(name, Creation::ExecuteScript)
        .expect("fixtures will yield valid repositories")
}

/// Always use all the hunks.
pub fn to_change_specs_whole_file(changes: but_core::WorktreeChanges) -> Vec<DiffSpec> {
    let out: Vec<_> = changes
        .changes
        .into_iter()
        .map(|change| DiffSpec {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path,
            hunk_headers: Vec::new(),
        })
        .collect();
    assert!(
        !out.is_empty(),
        "fixture should contain actual changes to turn into requests"
    );
    out
}

/// Always use all the hunks.
pub fn to_change_specs_all_hunks(
    repo: &gix::Repository,
    changes: but_core::WorktreeChanges,
) -> anyhow::Result<Vec<DiffSpec>> {
    to_change_specs_all_hunks_with_context_lines(repo, changes, CONTEXT_LINES)
}

/// Always use all the hunks.
pub fn to_change_specs_all_hunks_with_context_lines(
    repo: &gix::Repository,
    changes: but_core::WorktreeChanges,
    context_lines: u32,
) -> anyhow::Result<Vec<DiffSpec>> {
    let mut out = Vec::with_capacity(changes.changes.len());
    for change in changes.changes {
        let spec = match change.status {
            // Untracked files must always be taken from disk (they don't have a counterpart in a tree yet)
            TreeStatus::Addition { is_untracked, .. } if is_untracked => DiffSpec {
                path: change.path,
                ..Default::default()
            },
            _ => {
                match change.unified_diff(repo, context_lines) {
                    Ok(but_core::UnifiedDiff::Patch { hunks }) => DiffSpec {
                        previous_path: change.previous_path().map(ToOwned::to_owned),
                        path: change.path,
                        hunk_headers: hunks.into_iter().map(Into::into).collect(),
                    },
                    Ok(_) => unreachable!("tests won't be binary or too large"),
                    Err(_err) => {
                        // Assume it's a submodule or something without content, don't do hunks then.
                        DiffSpec {
                            path: change.path,
                            ..Default::default()
                        }
                    }
                }
            }
        };
        out.push(spec);
    }
    Ok(out)
}

pub fn write_sequence(
    repo: &gix::Repository,
    filename: &str,
    sequences: impl IntoIterator<Item = (impl Into<Option<usize>>, impl Into<Option<usize>>)>,
) -> anyhow::Result<()> {
    use std::fmt::Write;
    let mut out = String::new();
    for (start, end) in sequences {
        let (start, end) = match (start.into(), end.into()) {
            (Some(start), Some(end)) => (start, end),
            (Some(start), None) => (1, start),
            invalid => panic!("invalid sequence: {invalid:?}"),
        };
        for num in start..=end {
            writeln!(&mut out, "{}", num)?;
        }
    }
    std::fs::write(
        repo.work_dir().expect("non-bare").join(filename),
        out.as_bytes(),
    )?;
    Ok(())
}

pub fn visualize_tree(
    repo: &gix::Repository,
    outcome: &but_workspace::commit_engine::CreateCommitOutcome,
) -> anyhow::Result<String> {
    Ok(gitbutler_testsupport::visualize_gix_tree(
        outcome
            .new_commit
            .expect("no rejected changes")
            .attach(repo)
            .object()?
            .peel_to_commit()?
            .tree_id()?,
    )
    .to_string())
}

/// Create a commit with the entire file as change, and another time with a whole hunk.
/// Both should be equal or it will panic.
pub fn commit_whole_files_and_all_hunks_from_workspace(
    repo: &gix::Repository,
    destination: Destination,
) -> anyhow::Result<but_workspace::commit_engine::CreateCommitOutcome> {
    let worktree_changes = but_core::diff::worktree_changes(repo)?;
    let whole_file_output = but_workspace::commit_engine::create_commit(
        repo,
        destination.clone(),
        None,
        to_change_specs_whole_file(worktree_changes.clone()),
        CONTEXT_LINES,
    )?;
    let all_hunks_output = but_workspace::commit_engine::create_commit(
        repo,
        destination,
        None,
        to_change_specs_all_hunks(repo, worktree_changes)?,
        CONTEXT_LINES,
    )?;

    if whole_file_output.new_commit.is_some() && all_hunks_output.new_commit.is_some() {
        assert_eq!(
            visualize_tree(repo, &all_hunks_output)?,
            visualize_tree(repo, &whole_file_output)?,
        );
    }
    assert_eq!(
        all_hunks_output.new_commit, whole_file_output.new_commit,
        "Adding the whole file is the same as adding all patches (but whole files are faster)"
    );
    // NOTE: cannot compare rejections as whole-file rejections don't have hunks
    assert_eq!(
        all_hunks_output
            .rejected_specs
            .iter()
            .cloned()
            .map(|mut spec| {
                spec.hunk_headers.clear();
                spec
            })
            .collect::<Vec<_>>(),
        whole_file_output.rejected_specs,
        "rejections are the same as well"
    );
    Ok(all_hunks_output)
}

pub fn commit_from_outcome(
    repo: &gix::Repository,
    outcome: &but_workspace::commit_engine::CreateCommitOutcome,
) -> anyhow::Result<gix::objs::Commit> {
    Ok(outcome
        .new_commit
        .expect("the amended commit was created")
        .attach(repo)
        .object()?
        .peel_to_commit()?
        .decode()?
        .into())
}
