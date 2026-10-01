//! Spike: can the reads behind Lite's workspace page run on a bare repository that only holds
//! what a machine published, seen through a git ref namespace?
//!
//! A two-stack workspace with uncommitted edits is "published" by pushing its refs and a WIP
//! commit into a namespace of a bare mirror; every read then runs on both sides and is compared.

use std::path::Path;

use anyhow::{Context as _, Result};
use but_testsupport::git_at_dir;

use crate::support::{persist_default_target, writable_scenario, write_file};

const NAMESPACE: &str = "machine-a/checkout-1";
const WIP_REF: &str = but_core::diff::PUBLISHED_WORKTREE_REF;

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = git_at_dir(dir).args(args).output()?;
    anyhow::ensure!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

/// The source machine: `feature` and `sibling` applied as two stacks, plus edits nobody committed.
fn source_workspace() -> Result<(but_ctx::Context, tempfile::TempDir)> {
    let (repo, tmp) = writable_scenario("checkout-head-info");
    persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::branch::apply_only(&mut ctx, "refs/heads/feature".try_into()?)?;
    but_api::branch::apply_only(&mut ctx, "refs/heads/sibling".try_into()?)?;

    write_file(tmp.path(), "shared.txt", "main\nedited on machine a\n")?;
    write_file(tmp.path(), "untracked.txt", "new file\n")?;
    Ok((ctx, tmp))
}

/// `but publish --include-uncommitted`: capture worktree + index as a commit on top of HEAD
/// through a scratch index, then push every ref and the WIP commit atomically into the
/// mirror's namespace. The namespace is applied by the receiving side, as a server would.
fn publish(source: &Path, mirror: &Path) -> Result<()> {
    let scratch_index = source.join(".git/publish-index");
    let in_scratch = |args: &[&str]| -> Result<String> {
        let out = git_at_dir(source)
            .env("GIT_INDEX_FILE", &scratch_index)
            .args(args)
            .output()?;
        anyhow::ensure!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
    };
    in_scratch(&["read-tree", "HEAD"])?;
    in_scratch(&["add", "-A"])?;
    let tree = in_scratch(&["write-tree"])?;
    std::fs::remove_file(&scratch_index)?;
    let wip = git(source, &["commit-tree", &tree, "-p", "HEAD", "-m", "wip"])?;
    git(source, &["update-ref", WIP_REF, &wip])?;

    let receive_pack = format!("GIT_NAMESPACE={NAMESPACE} git-receive-pack");
    git(
        source,
        &[
            "push",
            "--atomic",
            "--receive-pack",
            &receive_pack,
            mirror.to_str().context("utf-8 path")?,
            "refs/heads/*:refs/heads/*",
            "refs/remotes/*:refs/remotes/*",
            &format!("{WIP_REF}:{WIP_REF}"),
        ],
    )?;
    Ok(())
}

/// The host's view of one published checkout: the shared bare repo with the checkout's namespace.
fn mirror_context(source: &gix::Repository, mirror: &Path) -> Result<but_ctx::Context> {
    let ns = gix::refs::namespace::expand(NAMESPACE)?;
    // Pushes can't carry a symbolic HEAD; the host points it at the workspace ref.
    git(
        mirror,
        &[
            "symbolic-ref",
            &format!("{}HEAD", ns.as_bstr()),
            &format!(
                "{}{}",
                ns.as_bstr(),
                source.head_name()?.context("HEAD is a ref")?
            ),
        ],
    )?;
    let mut repo = but_testsupport::open_repo(mirror)?;
    {
        // Each checkout gets its own GitButler storage for the host's caches; nothing in it
        // was published.
        let storage = format!("gitbutler/namespaces/{NAMESPACE}");
        repo.config_snapshot_mut().set_raw_value(
            but_project_handle::storage_path_config_key(),
            storage.as_str(),
        )?;
    }
    // Set last: committing an in-memory config edit resets the ref store and its namespace.
    repo.set_namespace(NAMESPACE)?;
    // Project metadata lives in repository-local git config, which a namespace doesn't scope.
    but_core::ref_metadata::ProjectMeta::resolve(source)?.persist(&repo)?;
    Ok(but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache())
}

fn json<T: serde::Serialize>(value: &T) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
}

/// Differences between the mirror and the source, reported together at the end.
#[derive(Default)]
struct Findings(Vec<String>);

impl Findings {
    fn compare(&mut self, what: &str, mirror: impl std::fmt::Debug, source: impl std::fmt::Debug) {
        let (mirror, source) = (format!("{mirror:#?}"), format!("{source:#?}"));
        if mirror != source {
            let dir = tempfile::tempdir().expect("temp dir");
            let (m, s) = (dir.path().join("mirror"), dir.path().join("source"));
            std::fs::write(&m, &mirror).expect("write");
            std::fs::write(&s, &source).expect("write");
            let out = std::process::Command::new("diff")
                .args(["-U2"])
                .arg(&s)
                .arg(&m)
                .output()
                .expect("diff runs");
            let diff = String::from_utf8_lossy(&out.stdout);
            let body: Vec<&str> = diff.lines().skip(2).collect();
            self.0.push(format!("{what}:\n{}", body.join("\n")));
        }
    }

    fn fail(&mut self, what: &str, err: anyhow::Error) {
        self.0.push(format!("{what}: mirror failed: {err:#}"));
    }
}

/// Run `read` on both sides, recording a difference or a mirror failure as a finding.
fn read_both<T: std::fmt::Debug>(
    findings: &mut Findings,
    what: &str,
    source: &but_ctx::Context,
    mirror: &but_ctx::Context,
    read: impl Fn(&but_ctx::Context) -> Result<T>,
) -> Result<()> {
    let source_value = read(source).with_context(|| format!("{what} on the source"))?;
    match read(mirror) {
        Ok(mirror_value) => findings.compare(what, mirror_value, source_value),
        Err(err) => findings.fail(what, err),
    }
    Ok(())
}

#[test]
fn workspace_reads_on_a_namespaced_bare_mirror() -> Result<()> {
    let (source_ctx, source_tmp) = source_workspace()?;
    let mirror_tmp = tempfile::tempdir()?;
    let mirror = mirror_tmp.path().join("project.git");
    git(
        mirror_tmp.path(),
        &["init", "--bare", "-q", mirror.to_str().unwrap()],
    )?;

    publish(source_tmp.path(), &mirror)?;
    // Another machine's checkout in the same repository, with a branch machine A doesn't have.
    let main = git(
        &mirror,
        &[
            "rev-parse",
            "refs/namespaces/machine-a/refs/namespaces/checkout-1/refs/heads/main",
        ],
    )?;
    git(
        &mirror,
        &[
            "update-ref",
            "refs/namespaces/machine-b/refs/namespaces/checkout-1/refs/heads/only-on-b",
            &main,
        ],
    )?;
    let source_repo = source_ctx.repo.get()?.clone();
    let mirror_ctx = mirror_context(&source_repo, &mirror)?;
    let mut findings = Findings::default();

    // Stack ids and branch metadata are the source's local bookkeeping, which isn't published.
    read_both(&mut findings, "headInfo", &source_ctx, &mirror_ctx, |ctx| {
        let mut info = json(&but_workspace::ui::RefInfo::try_from(
            but_api::legacy::workspace::head_info(ctx)?,
        )?)?;
        for stack in info["stacks"].as_array_mut().into_iter().flatten() {
            stack["id"] = serde_json::Value::Null;
            for segment in stack["segments"].as_array_mut().into_iter().flatten() {
                segment["metadata"] = serde_json::Value::Null;
            }
        }
        Ok(info)
    })?;
    read_both(
        &mut findings,
        "workspaceTargetCommits",
        &source_ctx,
        &mirror_ctx,
        |ctx| {
            json(&but_api::target_commits::workspace_target_commits(
                ctx, None, None,
            )?)
        },
    )?;
    read_both(
        &mut findings,
        "branchList",
        &source_ctx,
        &mirror_ctx,
        |ctx| Ok(format!("{:#?}", but_api::branch::branch_list(ctx)?)),
    )?;

    for branch in ["feature", "sibling"] {
        read_both(
            &mut findings,
            &format!("branchDetails({branch})"),
            &source_ctx,
            &mirror_ctx,
            |ctx| {
                json(&but_api::legacy::workspace::branch_details(
                    ctx,
                    branch.into(),
                    None,
                )?)
            },
        )?;
        read_both(
            &mut findings,
            &format!("branchDiff({branch})"),
            &source_ctx,
            &mirror_ctx,
            |ctx| json(&but_api::branch::branch_diff(ctx, branch.into())?),
        )?;
        let tip = source_repo.rev_parse_single(branch)?.detach();
        read_both(
            &mut findings,
            &format!("commitDetailsWithLineStats({branch})"),
            &source_ctx,
            &mirror_ctx,
            |ctx| {
                json(&but_api::diff::json::CommitDetails::from(
                    but_api::diff::commit_details_with_line_stats(ctx, tip)?,
                ))
            },
        )?;
    }

    // changesInWorktree as Lite calls it. Blob ids and the untracked flag are left out: a worktree
    // has no ids for unhashed files and the published commit can't tell untracked from added.
    read_both(
        &mut findings,
        "changesInWorktree",
        &source_ctx,
        &mirror_ctx,
        |ctx| {
            let changes = but_api::diff::changes_in_worktree(
                ctx,
                but_api::commit::json::ChangesSource::Head,
                true,
            )?;
            let files: Vec<String> = changes
                .worktree_changes
                .changes
                .iter()
                .map(|change| {
                    format!(
                        "{:?} {:?}",
                        change.path,
                        std::mem::discriminant(&change.status)
                    )
                })
                .collect();
            // Hunk assignments are deprecated and left out.
            Ok((files, changes.dependencies.is_some()))
        },
    )?;

    // treeChangeDiffs for each uncommitted change, as each side reports it.
    let changes_of = |ctx: &but_ctx::Context| -> Result<Vec<but_core::TreeChange>> {
        Ok(but_core::diff::worktree_changes(&*ctx.repo.get()?)?.changes)
    };
    for (mirror_change, source_change) in changes_of(&mirror_ctx)?
        .iter()
        .zip(changes_of(&source_ctx)?)
    {
        let source_patch =
            but_api::diff::tree_change_diffs(&source_ctx, source_change.clone().into())?;
        let what = format!("treeChangeDiffs({})", source_change.path);
        match but_api::diff::tree_change_diffs(&mirror_ctx, mirror_change.clone().into()) {
            Ok(mirror_patch) => findings.compare(&what, mirror_patch, source_patch),
            Err(err) => findings.fail(&what, err),
        }
    }

    // A bare repository answers every read the workspace page makes like the checkout it was
    // published from.
    assert!(findings.0.is_empty(), "{}", findings.0.join("\n\n"));
    Ok(())
}
