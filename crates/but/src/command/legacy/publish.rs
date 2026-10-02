//! Implementation of the `but _publish` command.

use std::{collections::BTreeMap, io::Write as _, path::Path, process::Stdio};

use anyhow::{Context as _, bail};
use but_ctx::Context;
use serde::Serialize;

use crate::{
    CliResult,
    args::publish::Platform,
    error::bad_input,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

struct PublishOperation {
    server: String,
    token: String,
    machine: String,
    include_uncommitted: bool,
}

pub fn publish(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<PublishOutcome> {
    let operation = resolve(args)?;
    Ok(run(ctx, operation)?)
}

/// Names travel in the server's URLs, so they keep to a safe alphabet.
fn url_name(name: &str) -> String {
    name.replace(
        |c: char| !c.is_ascii_alphanumeric() && !"_-".contains(c),
        "-",
    )
}

fn resolve(args: Platform) -> CliResult<PublishOperation> {
    let Platform {
        to,
        token,
        machine,
        include_uncommitted,
    } = args;
    let machine = match machine {
        Some(machine) => machine,
        None => std::process::Command::new("hostname")
            .arg("-s")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .unwrap_or_default(),
    };
    if machine.is_empty() {
        return Err(bad_input("Name this machine with --machine").into());
    }
    Ok(PublishOperation {
        server: to.trim_end_matches('/').to_owned(),
        token,
        machine: url_name(&machine),
        include_uncommitted,
    })
}

fn run(ctx: &mut Context, operation: PublishOperation) -> anyhow::Result<PublishOutcome> {
    let PublishOperation {
        server,
        token,
        machine,
        include_uncommitted,
    } = operation;
    let repo = ctx.repo.get()?.clone();
    let dir = repo.workdir().context("Publishing needs a worktree")?;
    let checkout = url_name(&dir.file_name().unwrap_or_default().to_string_lossy());
    // The project is its earliest root commit, the same on every machine.
    let roots = git(dir, &[], &["rev-list", "--max-parents=0", "HEAD"], None)?;
    let project = roots.lines().last().context("HEAD has no root commit")?;

    let meta = but_core::ref_metadata::ProjectMeta::resolve(&repo)?;
    let announcement = serde_json::json!({
        "version": 1,
        "head": repo.head_name()?.context("HEAD is detached")?.as_bstr().to_string(),
        "targetRef": meta.target_ref.map(|name| name.to_string()),
        "targetCommitId": meta.target_commit_id.map(|id| id.to_string()),
        "pushRemote": meta.push_remote,
        "remotes": repo.remote_names().iter().filter_map(|name| {
            let url = repo.find_remote(&**name).ok()?.url(gix::remote::Direction::Fetch)?.to_bstring();
            Some((name.to_string(), url.to_string()))
        }).collect::<BTreeMap<_, _>>(),
        "includesUncommitted": include_uncommitted,
    });
    let blob = git(
        dir,
        &[],
        &["hash-object", "-w", "--stdin"],
        Some(&announcement.to_string()),
    )?;
    let tree = git(
        dir,
        &[],
        &["mktree"],
        Some(&format!("100644 blob {blob}\tpublished.json\n")),
    )?;

    let mut refspecs = vec![
        "+refs/heads/*:refs/heads/*".to_owned(),
        "+refs/remotes/*:refs/remotes/*".to_owned(),
        format!("+{tree}:refs/gitbutler/meta"),
    ];
    if include_uncommitted {
        // Worktree and index as a commit on top of HEAD, through a scratch index.
        let index = repo.git_dir().join("but-publish-index");
        let env = [("GIT_INDEX_FILE", index.as_os_str())];
        let snapshot = git(dir, &env, &["read-tree", "HEAD"], None)
            .and_then(|_| git(dir, &env, &["add", "-A"], None))
            .and_then(|_| git(dir, &env, &["write-tree"], None));
        std::fs::remove_file(&index).ok();
        let commit = [
            "commit-tree",
            &snapshot?,
            "-p",
            "HEAD",
            "-m",
            "Uncommitted changes",
        ];
        let wip = git(dir, &[], &commit, None)?;
        refspecs.push(format!("+{wip}:{}", but_core::diff::PUBLISHED_WORKTREE_REF));
    }

    let url = format!("{server}/git/{project}/{machine}/{checkout}");
    let auth = format!("http.extraHeader=Authorization: Bearer {token}");
    let mut push = vec!["-c", &auth, "push", "--atomic", "--prune", "--quiet", &url];
    push.extend(refspecs.iter().map(String::as_str));
    git(dir, &[], &push, None)?;
    Ok(PublishOutcome { url })
}

fn git(
    dir: &Path,
    env: &[(&str, &std::ffi::OsStr)],
    args: &[&str],
    stdin: Option<&str>,
) -> anyhow::Result<String> {
    let mut child = std::process::Command::new(gix::path::env::exe_invocation())
        .current_dir(dir)
        .envs(env.iter().copied())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut pipe = child.stdin.take().context("stdin")?;
    pipe.write_all(stdin.unwrap_or_default().as_bytes())?;
    drop(pipe);
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[must_use]
pub struct PublishOutcome {
    url: String,
}

impl CliOutputHuman for PublishOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        writeln!(out, "Published to {}", self.url)?;
        Ok(())
    }
}

impl CliOutput for PublishOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        struct Output {
            url: String,
        }
        Output { url: self.url }
    }
}
