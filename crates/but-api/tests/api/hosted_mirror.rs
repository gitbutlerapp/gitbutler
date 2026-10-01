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
const WIP_REF: &str = "refs/gitbutler/wip";
const META_REF: &str = "refs/gitbutler/meta";

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
    // Branch and stack metadata travel as a blob on a ref.
    let meta = git(
        source,
        &["hash-object", "-w", ".git/gitbutler/virtual_branches.toml"],
    )?;
    git(source, &["update-ref", META_REF, &meta])?;

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
            &format!("{META_REF}:{META_REF}"),
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
        // Each checkout gets its own GitButler storage inside the shared git dir, filled from
        // the published metadata blob.
        let storage = format!("gitbutler/namespaces/{NAMESPACE}");
        let meta = repo
            .find_reference(&format!("{}{META_REF}", ns.as_bstr()))?
            .peel_to_id()?
            .detach();
        let toml = repo.find_blob(meta)?.data.clone();
        std::fs::create_dir_all(mirror.join(&storage))?;
        std::fs::write(mirror.join(&storage).join("virtual_branches.toml"), toml)?;
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

    read_both(&mut findings, "headInfo", &source_ctx, &mirror_ctx, |ctx| {
        json(&but_workspace::ui::RefInfo::try_from(
            but_api::legacy::workspace::head_info(ctx)?,
        )?)
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

    // changesInWorktree as Lite calls it: expected to need a worktree.
    read_both(
        &mut findings,
        "changesInWorktree(as is)",
        &source_ctx,
        &mirror_ctx,
        |ctx| {
            let changes = but_api::diff::changes_in_worktree(
                ctx,
                but_api::commit::json::ChangesSource::Head,
                true,
            )?;
            Ok(format!("{:#?}", changes.worktree_changes.changes))
        },
    )?;

    // The seam: on the mirror, worktree changes are HEAD diffed against the WIP commit.
    let source_changes: Vec<but_core::TreeChange> =
        but_core::diff::worktree_changes(&source_repo)?.changes;
    let mirror_repo = mirror_ctx.repo.get()?.clone();
    let wip = mirror_repo.find_reference(WIP_REF)?.peel_to_id()?.detach();
    let head = mirror_repo.head_id()?.detach();
    let mirror_changes = but_core::diff::tree_changes(&mirror_repo, Some(head), wip)?;
    findings.compare(
        "worktree changes (HEAD..wip)",
        &mirror_changes,
        &source_changes,
    );

    // treeChangeDiffs for each uncommitted change, resolved in each side's own repository.
    for (mirror_change, source_change) in mirror_changes.iter().zip(&source_changes) {
        let source_patch = source_change.unified_patch(&source_repo, 3)?;
        match mirror_change.unified_patch(&mirror_repo, 3) {
            Ok(mirror_patch) => findings.compare(
                &format!("treeChangeDiffs({})", source_change.path),
                mirror_patch,
                source_patch,
            ),
            Err(err) => findings.fail(&format!("treeChangeDiffs({})", source_change.path), err),
        }
    }

    // Expected gaps, until the host reads worktree changes from the WIP commit: Lite's
    // `changesInWorktree` needs a worktree, and a tree diff has blob ids where the worktree
    // has none yet, and can't tell untracked from added.
    snapbox::assert_data_eq!(
        findings.0.join("\n\n"),
        snapbox::str![[r#"
changesInWorktree(as is): mirror failed: need non-bare repository

worktree changes (HEAD..wip):
@@ -8,5 +8,5 @@
             },
             state: ChangeState {
-                id: Sha1(0000000000000000000000000000000000000000),
+                id: Sha1(21fa33431f947eae6cabb2bc94445c0f3c11bc73),
                 kind: Blob,
             },
@@ -18,8 +18,8 @@
         status: Addition {
             state: ChangeState {
-                id: Sha1(0000000000000000000000000000000000000000),
+                id: Sha1(fa49b077972391ad58037050f2a75f74e3671e92),
                 kind: Blob,
             },
-            is_untracked: true,
+            is_untracked: false,
         },
     },
"#]]
    );
    Ok(())
}
