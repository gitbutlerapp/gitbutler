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

/// Where the checkout's announcement goes, as a tree: what `HEAD` is, its target and remotes.
const META_REF: &str = "refs/gitbutler/meta";

struct PublishOperation {
    server: String,
    token: String,
    machine: String,
    checkout: String,
    include_uncommitted: bool,
}

pub fn publish(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<PublishOutcome> {
    let operation = resolve(ctx, args)?;
    Ok(run(ctx, operation)?)
}

/// Names travel in the server's URLs, so they keep to a safe alphabet.
fn path_component(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_start_matches('.')
        .to_owned()
}

fn resolve(ctx: &mut Context, args: Platform) -> CliResult<PublishOperation> {
    let Platform {
        to,
        token,
        machine,
        include_uncommitted,
    } = args;

    let server = url::Url::parse(&to)
        .map_err(|err| bad_input(format!("Not a server URL: {err}")).arg_value(&to))?;
    if !matches!(server.scheme(), "http" | "https") {
        return Err(bad_input("The server URL must use http or https")
            .arg_value(&to)
            .into());
    }

    let machine = match machine {
        Some(machine) => path_component(&machine),
        None => path_component(
            &git_output(None, &["var", "GIT_COMMITTER_IDENT"]).map_or_else(
                |_| "machine".to_owned(),
                |_| host_name().unwrap_or_else(|| "machine".to_owned()),
            ),
        ),
    };
    if machine.is_empty() {
        return Err(bad_input("The machine name is empty").into());
    }

    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("Publishing needs a checkout with a worktree")?;
    let checkout = path_component(&workdir.file_name().unwrap_or_default().to_string_lossy());
    if repo.head_name()?.is_none() {
        return Err(
            bad_input("HEAD is detached; check out a branch or the workspace first").into(),
        );
    }

    Ok(PublishOperation {
        server: to.trim_end_matches('/').to_owned(),
        token,
        machine,
        checkout,
        include_uncommitted,
    })
}

fn host_name() -> Option<String> {
    let name = git_output(None, &["config", "--get", "but.publish.machine"])
        .ok()
        .filter(|name| !name.is_empty());
    name.or_else(|| {
        let out = std::process::Command::new("hostname")
            .arg("-s")
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    })
}

fn run(ctx: &mut Context, operation: PublishOperation) -> anyhow::Result<PublishOutcome> {
    let PublishOperation {
        server,
        token,
        machine,
        checkout,
        include_uncommitted,
    } = operation;
    let repo = ctx.repo.get()?.clone();
    let workdir = repo.workdir().context("a worktree")?.to_owned();
    let git_dir = repo.git_dir().to_owned();

    // The project is known by its earliest root commit, on every machine.
    let roots = git_output(Some(&workdir), &["rev-list", "--max-parents=0", "HEAD"])?;
    let project = roots
        .lines()
        .last()
        .context("HEAD has no root commit")?
        .to_owned();

    let head = repo
        .head_name()?
        .context("HEAD is a branch")?
        .as_bstr()
        .to_string();
    let project_meta = but_core::ref_metadata::ProjectMeta::resolve(&repo)?;
    let remotes: BTreeMap<String, String> = repo
        .remote_names()
        .into_iter()
        .filter_map(|name| {
            let remote = repo.find_remote(&*name).ok()?;
            let url = remote.url(gix::remote::Direction::Fetch)?;
            Some((name.to_string(), url.to_bstring().to_string()))
        })
        .collect();

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Announcement {
        version: u32,
        title: String,
        head: String,
        target_ref: Option<String>,
        target_commit_id: Option<String>,
        push_remote: Option<String>,
        remotes: BTreeMap<String, String>,
        includes_uncommitted: bool,
    }
    let announcement = serde_json::to_vec(&Announcement {
        version: 1,
        title: checkout.clone(),
        head,
        target_ref: project_meta.target_ref.map(|name| name.to_string()),
        target_commit_id: project_meta.target_commit_id.map(|id| id.to_string()),
        push_remote: project_meta.push_remote,
        remotes,
        includes_uncommitted: include_uncommitted,
    })?;

    let announcement = git_input(&workdir, &["hash-object", "-w", "--stdin"], &announcement)?;
    let tree_listing = format!("100644 blob {announcement}\tpublished.json\n");
    let meta_tree = git_input(&workdir, &["mktree"], tree_listing.as_bytes())?;

    let mut refspecs = vec![
        "+refs/heads/*:refs/heads/*".to_owned(),
        "+refs/remotes/*:refs/remotes/*".to_owned(),
        format!("+{meta_tree}:{META_REF}"),
    ];
    if include_uncommitted {
        let wip = uncommitted_commit(&workdir, &git_dir)?;
        refspecs.push(format!("+{wip}:{}", but_core::diff::PUBLISHED_WORKTREE_REF));
    }

    let url = format!("{server}/git/{project}/{machine}/{checkout}");
    let mut push = git_command(Some(&workdir));
    push.arg("-c")
        .arg(format!("http.extraHeader=Authorization: Bearer {token}"))
        .args(["push", "--atomic", "--prune", "--quiet"])
        .arg(&url)
        .args(&refspecs);
    let out = push.output().context("failed to run git push")?;
    if !out.status.success() {
        bail!(
            "Publishing to {server} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    Ok(PublishOutcome {
        server,
        machine,
        checkout,
        include_uncommitted,
    })
}

/// The worktree and index as a commit on top of `HEAD`, through a scratch index so the
/// real one is untouched.
fn uncommitted_commit(workdir: &Path, git_dir: &Path) -> anyhow::Result<String> {
    let scratch_index = git_dir.join("but-publish-index");
    let in_scratch = |args: &[&str]| -> anyhow::Result<String> {
        let out = git_command(Some(workdir))
            .env("GIT_INDEX_FILE", &scratch_index)
            .args(args)
            .output()?;
        if !out.status.success() {
            bail!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let tree = in_scratch(&["read-tree", "HEAD"])
        .and_then(|_| in_scratch(&["add", "-A"]))
        .and_then(|_| in_scratch(&["write-tree"]));
    std::fs::remove_file(&scratch_index).ok();
    let tree = tree?;
    git_output(
        Some(workdir),
        &[
            "commit-tree",
            &tree,
            "-p",
            "HEAD",
            "-m",
            "Uncommitted changes",
        ],
    )
}

fn git_command(dir: Option<&Path>) -> std::process::Command {
    let mut cmd = std::process::Command::new(gix::path::env::exe_invocation());
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    cmd
}

fn git_output(dir: Option<&Path>, args: &[&str]) -> anyhow::Result<String> {
    let out = git_command(dir).args(args).output()?;
    if !out.status.success() {
        bail!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn git_input(dir: &Path, args: &[&str], input: &[u8]) -> anyhow::Result<String> {
    let mut child = git_command(Some(dir))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child.stdin.take().context("stdin")?.write_all(input)?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[must_use]
pub struct PublishOutcome {
    server: String,
    machine: String,
    checkout: String,
    include_uncommitted: bool,
}

impl CliOutputHuman for PublishOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self {
            server,
            machine,
            checkout,
            include_uncommitted,
        } = self;
        let what = if include_uncommitted {
            "with uncommitted changes"
        } else {
            "committed changes only"
        };
        writeln!(out, "Published {machine}/{checkout} to {server} ({what})")?;
        Ok(())
    }
}

impl CliOutput for PublishOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            server: String,
            machine: String,
            checkout: String,
            include_uncommitted: bool,
        }

        let Self {
            server,
            machine,
            checkout,
            include_uncommitted,
        } = self;

        Output {
            server,
            machine,
            checkout,
            include_uncommitted,
        }
    }
}
