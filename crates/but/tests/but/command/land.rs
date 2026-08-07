use crate::utils::{CommandExt, Sandbox};

/// Headline real-remote path: landing a branch that is ahead of `origin/main` fast-forwards the
/// remote target (no merge commit), leaves the local `main` untouched, and rebases the sibling
/// branch onto the moved target.
#[test]
fn land_first_branch_into_origin() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");

    let remote = env.projects_root().with_extension("origin.git");
    env.invoke_git(&format!("init --bare {}", remote.display()));
    env.invoke_git(&format!(
        "--git-dir={} symbolic-ref HEAD refs/heads/main",
        remote.display()
    ));
    env.invoke_git(&format!("remote set-url origin {}", remote.display()));
    env.invoke_git("push origin main:main");
    env.invoke_git("fetch origin");
    env.but("setup").assert().success();

    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref HEAD"),
        "gitbutler/workspace"
    );

    env.but("branch new first-branch").assert().success();
    env.file("file1.txt", "content1");
    env.but("commit -b first-branch -m 'first commit on branch A'")
        .assert()
        .success();
    let first_branch_tip = env.invoke_git("rev-parse first-branch");

    env.but("branch new second-branch").assert().success();
    env.file("file2.txt", "content2");
    env.but("commit -b second-branch -m 'second commit on branch B'")
        .assert()
        .success();

    let main_before = env.invoke_git("rev-parse main");

    let output = env
        .but("land first-branch --yes")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("Landed first-branch onto origin/main"),
        "land should report it landed the branch onto the target; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Run `but undo`"),
        "land must not promise a clean `but undo` on the real-remote push path; got:\n{stdout}"
    );

    // Only the remote target advanced, and as a fast-forward (no merge commit).
    assert_eq!(
        main_before,
        env.invoke_git("rev-parse main"),
        "land must not move the local main ref"
    );
    assert_eq!(
        env.invoke_git("rev-parse origin/main"),
        first_branch_tip,
        "origin/main should fast-forward to the branch tip"
    );
    let parents = env.invoke_git("rev-list --parents -n 1 origin/main");
    let parent_count = parents.split_whitespace().count() - 1;
    assert_eq!(
        parent_count, 1,
        "fast-forward must not create a merge commit"
    );

    // The remaining sibling rebased onto the new target and the landed branch was removed.
    let status = status_json(&env);
    assert_eq!(
        status["stacks"].as_array().unwrap().len(),
        1,
        "only second-branch should remain after landing first-branch"
    );
    assert_eq!(
        env.invoke_git("merge-base origin/main second-branch"),
        env.invoke_git("rev-parse origin/main"),
        "second-branch should be rebased on top of the moved target"
    );
}

/// Self-remote (`gb-local`) path: landing a branch that is ahead of the local target fast-forwards
/// both `refs/heads/main` and the `gb-local/main` tracking ref, advances `behind` to 0, and removes
/// the integrated branch.
#[test]
fn land_fast_forwards_self_remote() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");

    env.but("setup").assert().success();
    env.but("branch new first-branch").assert().success();
    env.file("file1.txt", "content1");
    env.but("commit -b first-branch -m 'first commit on branch A'")
        .assert()
        .success();
    let first_branch_tip = env.invoke_git("rev-parse first-branch");

    let main_before = env.invoke_git("rev-parse main");
    assert_eq!(main_before, env.invoke_git("rev-parse gb-local/main"));

    let output = env
        .but("land first-branch --yes")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    // The self-remote move touches BOTH refs, so the recovery hint must name both — and must not
    // fall back to the misleading bare "Run `but undo`" (which can't reset the local target).
    assert!(
        stdout.contains("git update-ref refs/heads/main"),
        "self-remote recovery hint must restore refs/heads/main; got:\n{stdout}"
    );
    assert!(
        stdout.contains("git update-ref refs/remotes/gb-local/main"),
        "self-remote recovery hint must also restore the gb-local tracking ref; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Run `but undo`"),
        "land must not promise a clean `but undo` for the target move; got:\n{stdout}"
    );

    let main_after = env.invoke_git("rev-parse main");
    assert_eq!(
        main_after, first_branch_tip,
        "local main should fast-forward to the branch tip"
    );
    assert_eq!(
        main_after,
        env.invoke_git("rev-parse gb-local/main"),
        "gb-local/main should advance in lockstep with the local target"
    );
    let parents = env.invoke_git("rev-list --parents -n 1 main");
    assert_eq!(
        parents.split_whitespace().count() - 1,
        1,
        "fast-forward must not create a merge commit"
    );

    let status = status_json(&env);
    assert_eq!(
        status["stacks"].as_array().unwrap().len(),
        0,
        "the landed branch should be removed from the workspace"
    );
    assert_eq!(
        status["upstreamState"]["behind"].as_u64(),
        Some(0),
        "the stored base should be advanced to the updated target"
    );
}

/// `--no-ff` forces a merge commit even when a fast-forward is possible, and with signing enabled
/// the merge commit is signed and carries a GitButler change-id header. This is the regression
/// guard for the silent-unsigned-commit and missing-change-id bugs.
#[test]
fn land_no_ff_creates_signed_merge_commit() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");

    env.but("setup").assert().success();

    // Configure SSH commit signing, keeping the key outside the worktree so it can't pollute status.
    let key_path = env
        .projects_root()
        .parent()
        .expect("sandbox root")
        .join("land-signing.key");
    // ssh-keygen refuses to overwrite a key non-interactively, so clear any stale key from a
    // previous run of this test in a reused scenario directory.
    let _ = std::fs::remove_file(&key_path);
    let _ = std::fs::remove_file(key_path.with_file_name("land-signing.key.pub"));
    let keygen = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-C", "land-test", "-f"])
        .arg(&key_path)
        .status();
    match keygen {
        Ok(status) => assert!(status.success(), "ssh-keygen should produce a signing key"),
        // Skip rather than fail where OpenSSH isn't installed (e.g. minimal CI containers).
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping: ssh-keygen not available");
            return;
        }
        Err(err) => panic!("failed to run ssh-keygen: {err}"),
    }
    env.invoke_git("config gpg.format ssh");
    env.invoke_git(&format!("config user.signingKey {}", key_path.display()));
    env.invoke_git("config gitbutler.signCommits true");

    env.but("branch new first-branch").assert().success();
    env.file("file1.txt", "content1");
    env.but("commit -b first-branch -m 'first commit on branch A'")
        .assert()
        .success();

    let target_before = env.invoke_git("rev-parse main");
    let feature_tip = env.invoke_git("rev-parse first-branch");

    env.but("land first-branch --no-ff --yes")
        .assert()
        .success();

    // A 2-parent merge commit landed on the target (not a fast-forward), with parent order
    // [target, feature] so `git log --first-parent` stays on the target's history.
    let parents = env.invoke_git("rev-list --parents -n 1 main");
    let parent_oids: Vec<&str> = parents.split_whitespace().collect();
    assert_eq!(
        parent_oids.len() - 1,
        2,
        "--no-ff should create a 2-parent merge commit"
    );
    assert_eq!(
        parent_oids[1], target_before,
        "first parent must be the target"
    );
    assert_eq!(
        parent_oids[2], feature_tip,
        "second parent must be the landed branch"
    );

    let raw_commit = env.invoke_git("cat-file commit main");
    assert!(
        raw_commit.contains("Merge branch 'first-branch'"),
        "merge commit should carry the land message; got:\n{raw_commit}"
    );
    assert!(
        raw_commit.contains("BEGIN SSH SIGNATURE"),
        "the landed merge commit must be signed; got:\n{raw_commit}"
    );
    assert!(
        raw_commit.contains("change-id"),
        "the landed merge commit must carry a change-id header; got:\n{raw_commit}"
    );
}

/// An unreachable remote that is not the target's must not block landing: `but land` fetches only
/// the target's fetch remote, so a dead unrelated remote (old fork, deleted mirror) is ignored.
#[test]
fn land_ignores_unreachable_unrelated_remote() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");
    env.but("setup").assert().success();
    env.invoke_git("remote add broken /nonexistent/path/broken.git");

    env.but("branch new first-branch").assert().success();
    env.file("file1.txt", "content1");
    env.but("commit -b first-branch -m 'first commit on branch A'")
        .assert()
        .success();
    let branch_tip = env.invoke_git("rev-parse first-branch");

    env.but("land first-branch --yes").assert().success();

    assert_eq!(
        env.invoke_git("rev-parse gb-local/main"),
        branch_tip,
        "the land must succeed and advance the target despite the unreachable unrelated remote"
    );
}

/// Landing a non-bottom segment of a stack would silently publish the lower segments' commits, so
/// it must be refused before anything is mutated, naming the lower segment.
#[test]
fn land_refuses_non_bottom_stack_segment() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");
    env.but("setup").assert().success();

    // Build a 2-segment stack: bottom-seg, then top-seg stacked on top of it.
    env.but("branch new bottom-seg").assert().success();
    env.file("bottom.txt", "bottom");
    env.but("commit -b bottom-seg -m 'bottom commit'")
        .assert()
        .success();
    env.but("branch new top-seg --anchor bottom-seg")
        .assert()
        .success();
    env.file("top.txt", "top");
    env.but("commit -b top-seg -m 'top commit'")
        .assert()
        .success();

    let target_before = env.invoke_git("rev-parse gb-local/main");

    let output = env
        .but("land top-seg --yes")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains("Refusing to land"),
        "landing a non-bottom segment must be refused; got:\n{stderr}"
    );
    assert!(
        stderr.contains("bottom-seg"),
        "the refusal must name the lower segment that would be carried along; got:\n{stderr}"
    );
    assert!(
        stderr.contains("--whole-stack"),
        "the refusal must advertise the explicit whole-stack opt-in; got:\n{stderr}"
    );
    assert_eq!(
        target_before,
        env.invoke_git("rev-parse gb-local/main"),
        "a refused land must not move the target"
    );
}

/// `--whole-stack` is the explicit opt-in: naming the top segment lands the entire stack, and the
/// pre-flight warning names the lower segments being published so `--yes` runs stay honest.
#[test]
fn land_whole_stack_lands_top_segment() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");
    env.but("setup").assert().success();

    env.but("branch new bottom-seg").assert().success();
    env.file("bottom.txt", "bottom");
    env.but("commit -b bottom-seg -m 'bottom commit'")
        .assert()
        .success();
    env.but("branch new top-seg --anchor bottom-seg")
        .assert()
        .success();
    env.file("top.txt", "top");
    env.but("commit -b top-seg -m 'top commit'")
        .assert()
        .success();
    let top_tip = env.invoke_git("rev-parse top-seg");

    let output = env
        .but("land top-seg --whole-stack --yes")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("bottom-seg"),
        "the warning must name the lower segment being published along; got:\n{stdout}"
    );
    assert!(
        stdout.contains("Landed top-seg onto gb-local/main"),
        "the land should report success; got:\n{stdout}"
    );

    assert_eq!(
        env.invoke_git("rev-parse gb-local/main"),
        top_tip,
        "the target should advance to the top segment's tip, publishing the whole stack"
    );
    assert_eq!(
        env.invoke_git("rev-parse main"),
        top_tip,
        "the self-remote land should move the local target too"
    );
    assert_eq!(
        status_json(&env)["stacks"].as_array().unwrap().len(),
        0,
        "both landed segments should be removed from the workspace"
    );
}

/// The conflicted-commit guard must cover every segment a whole-stack land would publish: a
/// conflicted commit in a LOWER segment refuses `--whole-stack` on a clean top segment.
#[test]
fn land_whole_stack_refuses_conflicted_lower_segment() {
    let env = super::util::sandbox_with_conflicted_commit();

    // Stack a clean segment on top of branch A, which carries the conflicted commit.
    env.but("branch new top-seg --anchor A").assert().success();
    env.file("top.txt", "top");
    env.but("commit -b top-seg -m 'clean top commit'")
        .assert()
        .success();

    let output = env
        .but("land top-seg --whole-stack --yes")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains("conflicted commit"),
        "a conflicted commit below the named segment must refuse the whole-stack land; got:\n{stderr}"
    );
    assert!(
        stderr.contains("but resolve"),
        "the refusal must point at the resolution command; got:\n{stderr}"
    );
}

/// `--whole-stack` always means "the entire stack lands": naming anything but the top segment is
/// refused, pointing at the actual top, so a partial land can't strand the segments above.
#[test]
fn land_whole_stack_refuses_non_top_segment() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");
    env.but("setup").assert().success();

    env.but("branch new bottom-seg").assert().success();
    env.file("bottom.txt", "bottom");
    env.but("commit -b bottom-seg -m 'bottom commit'")
        .assert()
        .success();
    env.but("branch new top-seg --anchor bottom-seg")
        .assert()
        .success();
    env.file("top.txt", "top");
    env.but("commit -b top-seg -m 'top commit'")
        .assert()
        .success();

    let target_before = env.invoke_git("rev-parse gb-local/main");

    let output = env
        .but("land bottom-seg --whole-stack --yes")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains("not the top of its stack"),
        "--whole-stack on a non-top segment must be refused; got:\n{stderr}"
    );
    assert!(
        stderr.contains("top-seg"),
        "the refusal must point at the actual top segment to name; got:\n{stderr}"
    );
    assert_eq!(
        target_before,
        env.invoke_git("rev-parse gb-local/main"),
        "a refused land must not move the target"
    );
}

/// Rename tracking must be ON so a rename on the branch and a conflicting rename on the target
/// produce a conflict bail, NOT a silent mismerge that publishes a subtly-wrong tree. Uses a
/// rename/rename scenario: conflicts with rename tracking on (the shipped fix), but would merge
/// cleanly — silently — with it off, so this test fails if the fix is ever reverted.
#[test]
fn land_rename_no_silent_mismerge() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");
    env.but("setup").assert().success();

    // Seed a shared base file on the target via a clean fast-forward land.
    let base = "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\n";
    env.but("branch new seed").assert().success();
    env.file("foo.txt", base);
    env.but("commit -b seed -m 'add foo'").assert().success();
    env.but("land seed --yes").assert().success();

    // Branch renames foo.txt -> bar.txt (keeping it similar so the rename is detectable) and edits line 5.
    env.but("branch new rename-branch").assert().success();
    std::fs::remove_file(env.projects_root().join("foo.txt")).unwrap();
    env.file("bar.txt", "l1\nl2\nl3\nl4\nBRANCH\nl6\nl7\nl8\n");
    env.but("commit -b rename-branch -m 'rename foo to bar'")
        .assert()
        .success();

    // Out-of-band: advance the gb-local target to a commit that renames foo.txt -> baz.txt and edits
    // line 5 differently — a conflicting rename — WITHOUT going through the workspace reconcile.
    let target_tip = env.invoke_git("rev-parse gb-local/main");
    let wt = env
        .projects_root()
        .parent()
        .expect("sandbox root")
        .join("land-divergence-wt");
    let _ = std::fs::remove_dir_all(&wt);
    env.invoke_git(&format!(
        "worktree add --detach {} {}",
        wt.display(),
        target_tip
    ));
    std::fs::remove_file(wt.join("foo.txt")).unwrap();
    std::fs::write(wt.join("baz.txt"), "l1\nl2\nl3\nl4\nTARGET\nl6\nl7\nl8\n").unwrap();
    env.invoke_git(&format!("-C {} add -A", wt.display()));
    env.invoke_git(&format!(
        "-C {} commit -m 'target renames foo to baz'",
        wt.display()
    ));
    let diverged = env.invoke_git(&format!("-C {} rev-parse HEAD", wt.display()));
    env.invoke_git(&format!("worktree remove --force {}", wt.display()));
    env.invoke_git(&format!("update-ref refs/heads/main {diverged}"));
    env.invoke_git(&format!("update-ref refs/remotes/gb-local/main {diverged}"));

    // The conflicting rename must be detected and the land must bail before mutating the target.
    let output = env
        .but("land rename-branch --yes")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.to_lowercase().contains("conflict"),
        "a conflicting rename must bail (rename tracking ON), not silently mismerge; got:\n{stderr}"
    );
    assert_eq!(
        diverged,
        env.invoke_git("rev-parse gb-local/main"),
        "a conflicting land must not move the target"
    );
}

/// The load-bearing safety gate: without `--yes`, a non-interactive `but land` (a script or agent)
/// must refuse before mutating anything, rather than silently publishing to the target.
#[test]
fn land_without_yes_refuses_non_interactively() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");
    env.but("setup").assert().success();
    env.but("branch new first-branch").assert().success();
    env.file("file1.txt", "content1");
    env.but("commit -b first-branch -m 'first commit on branch A'")
        .assert()
        .success();

    let target_before = env.invoke_git("rev-parse gb-local/main");

    let output = env
        .but("land first-branch")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains("Re-run with --yes"),
        "a non-interactive land without --yes must refuse and point at --yes; got:\n{stderr}"
    );
    assert_eq!(
        target_before,
        env.invoke_git("rev-parse gb-local/main"),
        "a refused land must not move the target"
    );
    assert_eq!(
        status_json(&env)["stacks"].as_array().unwrap().len(),
        1,
        "the branch must remain applied after a refused land"
    );
}

/// Landing a previously pushed branch deletes its copy on the remote and the local tracking ref —
/// the direct-push counterpart of the forge's "delete branch after merge". Leaving it behind makes
/// a later branch with the same name show up as merged upstream and refuse commits.
#[test]
fn land_deletes_remote_copy_of_landed_branch() {
    let (env, remote) = sandbox_with_bare_origin();

    env.file("file1.txt", "content1");
    env.but("commit -b first-branch -m 'first commit'")
        .assert()
        .success();
    env.but("push first-branch").assert().success();
    assert_eq!(
        env.invoke_git("rev-parse origin/first-branch"),
        env.invoke_git("rev-parse first-branch"),
        "the branch must be on the remote before landing for this test to mean anything"
    );

    let output = env
        .but("land first-branch --yes")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("Deleted origin/first-branch"),
        "land should report the remote copy cleanup; got:\n{stdout}"
    );

    let remote_refs = env.invoke_git(&format!("--git-dir={} show-ref", remote.display()));
    assert!(
        !remote_refs.contains("refs/heads/first-branch"),
        "the landed branch's copy on the remote must be deleted; got:\n{remote_refs}"
    );
    let local_refs = env.invoke_git("show-ref");
    assert!(
        !local_refs.contains("refs/remotes/origin/first-branch"),
        "the stale remote-tracking ref must be deleted too; got:\n{local_refs}"
    );
}

/// A remote copy holding commits that did not land must be left alone: the containment guard only
/// deletes remote branches whose tip is reachable from the landed target.
#[test]
fn land_keeps_remote_branch_with_unlanded_commits() {
    let (env, remote) = sandbox_with_bare_origin();

    env.file("file1.txt", "content1");
    env.but("commit -b first-branch -m 'first commit'")
        .assert()
        .success();
    env.file("file2.txt", "content2");
    env.but("commit -b second-branch -m 'second commit'")
        .assert()
        .success();

    // Point the remote's first-branch at a commit that will NOT land (second-branch's tip).
    let unlanded = env.invoke_git("rev-parse second-branch");
    env.invoke_git(&format!("push origin {unlanded}:refs/heads/first-branch"));
    env.invoke_git("fetch origin");

    let output = env
        .but("land first-branch --yes")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("Landed first-branch onto origin/main"),
        "the land itself must succeed and report, or the negative check below proves nothing; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Deleted origin/first-branch"),
        "a remote copy with unlanded commits must not be reported deleted; got:\n{stdout}"
    );

    let remote_refs = env.invoke_git(&format!("--git-dir={} show-ref", remote.display()));
    assert!(
        remote_refs.contains("refs/heads/first-branch"),
        "a remote copy with unlanded commits must survive the land; got:\n{remote_refs}"
    );
}

/// A branch whose configured upstream is the target itself — the `git checkout -b topic
/// origin/main` shape — must never have that upstream deleted as its "remote copy": the upstream
/// is a fork point, not a copy, and deleting it would delete the target branch on the remote.
#[test]
fn land_never_deletes_a_differently_named_upstream() {
    let (env, remote) = sandbox_with_bare_origin();

    env.file("file1.txt", "content1");
    env.but("commit -b topic -m 'topic commit'")
        .assert()
        .success();
    env.invoke_git("branch --set-upstream-to=origin/main topic");

    let output = env
        .but("land topic --yes")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("Landed topic onto origin/main"),
        "the land itself must succeed; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Deleted origin/main"),
        "the target must never be reported as a deleted branch copy; got:\n{stdout}"
    );

    let remote_refs = env.invoke_git(&format!("--git-dir={} show-ref", remote.display()));
    assert!(
        remote_refs.contains("refs/heads/main"),
        "the target branch must survive on the remote; got:\n{remote_refs}"
    );
}

/// A sandbox whose `origin` is a real bare repository holding `main`, with the workspace set up —
/// the shape the remote-copy cleanup tests need to observe deletions on the remote side.
fn sandbox_with_bare_origin() -> (Sandbox, std::path::PathBuf) {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    let remote = env.projects_root().with_extension("origin.git");
    env.invoke_git(&format!("init --bare {}", remote.display()));
    env.invoke_git(&format!(
        "--git-dir={} symbolic-ref HEAD refs/heads/main",
        remote.display()
    ));
    env.invoke_git(&format!("remote set-url origin {}", remote.display()));
    env.invoke_git("push origin main:main");
    env.invoke_git("fetch origin");
    env.but("setup").assert().success();
    (env, remote)
}

fn status_json(env: &Sandbox) -> serde_json::Value {
    let stdout = env
        .but("status --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_str(&String::from_utf8_lossy(&stdout)).unwrap()
}
