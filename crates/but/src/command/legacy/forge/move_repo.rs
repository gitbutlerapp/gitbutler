//! `but config forge _move`: make GitButler Code a repository's home.
//!
//! Every step checks whether it's done already, so running the command again finishes a move
//! that stopped partway, and changes nothing after a complete one.

use anyhow::{Context as _, Result};
use but_ctx::Context;
use but_gitbutler_code::{GitbutlerCodeAccountIdentifier, HttpStatusError};
use serde::Serialize;

use crate::{
    CliResult, bad_input,
    theme::{Paint, Theme},
    utils::{CliOutput, CliOutputHuman, OutputChannel, WriteWithUtils},
};

/// The remote the GitHub `origin` is renamed to.
const GITHUB_REMOTE: &str = "github";
/// The remote that points at the organization's project once the move is done.
const HOME_REMOTE: &str = "origin";
/// The branch the organization's project integrates into.
const MAIN: &str = "main";

pub(crate) async fn move_repo(
    ctx: &mut Context,
    out: &mut OutputChannel,
    destination: Option<String>,
) -> CliResult<()> {
    let account = signed_in_account(out).await?;
    let you = but_api::gitbutler_code::user(account.clone())
        .await
        .map_err(|err| match err.downcast_ref::<HttpStatusError>() {
            Some(status) if status.status.as_u16() == 401 => bad_input(format!(
                "GitButler Code no longer accepts the token of {account}"
            ))
            .hint("Sign in again with `but config forge auth` in a terminal")
            .into(),
            _ => crate::CliError::from(err),
        })?;
    let operation = resolve(ctx, you, destination)?;
    let outcome = run(ctx, &account, operation).await?;
    out.print_cli_output(outcome)
}

/// The signed-in GitButler Code account, signing in first in a terminal when there is none.
async fn signed_in_account(out: &mut OutputChannel) -> CliResult<GitbutlerCodeAccountIdentifier> {
    match but_api::gitbutler_code::signed_in_account() {
        Ok(account) => Ok(account),
        Err(err) if err.is::<but_api::gitbutler_code::NotSignedIn>() => {
            let Some(mut inout) = out.prepare_for_terminal_input() else {
                return Err(not_signed_in().into());
            };
            let username = crate::command::config::gitbutler_code_login(&mut inout).await?;
            Ok(GitbutlerCodeAccountIdentifier::device_login(&username))
        }
        Err(err) => Err(err.into()),
    }
}

/// The error for a run that needs a GitButler Code account and has none.
pub(crate) fn not_signed_in() -> crate::error::BadInput {
    bad_input("Not signed in to GitButler Code").hint(
        "Run `but config forge auth` in a terminal and pick GitButler Code (an agent has to ask the user to run it)",
    )
}

/// What to move, and where.
struct MoveOperation {
    /// The signed-in user, who gets the copy.
    you: String,
    /// The organization, like `gitbutler.com`.
    domain: String,
    /// The project's name in the organization and in `you`'s space.
    name: String,
    /// The remote the target branch tracks now.
    target_remote: String,
    /// Whether `target_remote` is on GitButler Code already, as after a complete move.
    moved: bool,
    /// The target branch's name on `target_remote`, like `master`.
    target_branch: String,
}

fn resolve(ctx: &Context, you: String, destination: Option<String>) -> CliResult<MoveOperation> {
    let repo = ctx.repo.get()?;
    let meta = ctx.project_meta()?;
    let Some(target_ref) = meta.target_ref.as_ref() else {
        return Err(bad_input("No target branch is configured")
            .hint("Set the target with `but config target origin/<default-branch>` first")
            .into());
    };
    let target_remote = repo
        .branch_remote_name(target_ref.shorten(), gix::remote::Direction::Fetch)
        .map(|name| name.as_bstr().to_string())
        .with_context(|| {
            format!(
                "The target branch {} tracks no remote",
                target_ref.shorten()
            )
        })?;
    let remote_tracking = meta.remote_target_ref(&repo)?.with_context(|| {
        format!(
            "The target branch {} tracks no remote",
            target_ref.shorten()
        )
    })?;
    let target_branch = remote_tracking
        .shorten()
        .to_string()
        .strip_prefix(&format!("{target_remote}/"))
        .map(str::to_owned)
        .with_context(|| format!("Can't tell the branch {remote_tracking} names"))?;
    let url = meta.target_remote_url(&repo)?;
    let info = but_forge::derive_forge_repo_info(&url, None);
    let (moved, owner, repo_name) =
        match info {
            Some(info) if info.forge == but_forge::ForgeName::GitButler => {
                (true, Some(info.owner), info.repo)
            }
            Some(info) if info.forge == but_forge::ForgeName::GitHub => (false, None, info.repo),
            _ => {
                return Err(bad_input(format!("The target's remote, {url}, isn't on GitHub"))
                .hint("Only repositories whose target is on GitHub can move to GitButler Code")
                .into());
            }
        };
    let (domain, name) = match destination.as_deref().map(|d| d.trim().trim_matches('/')) {
        Some(destination) if !destination.is_empty() => match destination.split_once('/') {
            Some((domain, name)) => (domain.to_lowercase(), name.to_owned()),
            None => (destination.to_lowercase(), repo_name.clone()),
        },
        _ => (
            owner
                .clone()
                .unwrap_or_else(|| format!("{you}.gitbutler.dev")),
            repo_name.clone(),
        ),
    };
    if let Some(owner) = owner
        && (owner != domain || repo_name != name)
    {
        return Err(bad_input(format!(
            "This repository moved to {owner}/{repo_name} already"
        ))
        .into());
    }
    Ok(MoveOperation {
        you,
        domain,
        name,
        target_remote,
        moved,
        target_branch,
    })
}

async fn run(
    ctx: &mut Context,
    account: &GitbutlerCodeAccountIdentifier,
    op: MoveOperation,
) -> CliResult<MoveOutcome> {
    let MoveOperation {
        you,
        domain,
        name,
        target_remote,
        moved,
        target_branch,
    } = op;
    let canonical = format!("{domain}/{name}");
    let canonical_url = format!("{}/{canonical}.git", but_gitbutler_code::base_url());

    // Remotes first: pushing `main` to a new project goes through `origin`.
    let mut renamed = false;
    let github_remote = if moved {
        None
    } else if target_remote == HOME_REMOTE {
        if remote_url(ctx, GITHUB_REMOTE)?.is_some() {
            return Err(bad_input(format!(
                "Can't rename {HOME_REMOTE} to {GITHUB_REMOTE}: a remote named {GITHUB_REMOTE} exists already"
            ))
            .into());
        }
        but_api::gitbutler_code::rename_remote(ctx, HOME_REMOTE, GITHUB_REMOTE)?;
        renamed = true;
        reload(ctx)?;
        Some(GITHUB_REMOTE.to_owned())
    } else {
        Some(target_remote)
    };
    ensure_remote(ctx, HOME_REMOTE, &canonical_url)?;

    // The organization's project: pushing GitHub's target as `main` creates it when it's missing.
    let exists = but_api::gitbutler_code::space(account.clone(), domain.clone(), name.clone())
        .await?
        .is_some();
    let created = !exists;
    if created {
        let github_remote = github_remote.with_context(|| {
            format!("{canonical} doesn't exist on GitButler Code, or you can't read it")
        })?;
        but_api::gitbutler_code::fetch(ctx, &github_remote)?;
        let commit = {
            let repo = ctx.repo.get()?;
            repo.find_reference(format!("refs/remotes/{github_remote}/{target_branch}").as_str())?
                .peel_to_id()?
                .detach()
        };
        if let Err(err) = but_api::gitbutler_code::push_main(ctx, HOME_REMOTE, commit) {
            // Leave the remotes as they were, so a failed move changes nothing.
            if renamed {
                but_api::gitbutler_code::remove_remote(ctx, HOME_REMOTE)?;
                but_api::gitbutler_code::rename_remote(ctx, GITHUB_REMOTE, HOME_REMOTE)?;
                reload(ctx)?;
            }
            let hint = if domain == format!("{you}.gitbutler.dev") {
                format!(
                    "Create your organization {domain} at {}/orgs, then run this again",
                    but_gitbutler_code::base_url()
                )
            } else {
                format!("Check that {domain} exists on GitButler Code and that you're a member")
            };
            return Err(bad_input(format!(
                "Couldn't create {canonical}: {}",
                server_reason(&err)
            ))
            .hint(hint)
            .into());
        }
    }

    but_api::gitbutler_code::fetch(ctx, HOME_REMOTE)?;
    // A local `main` that tracked GitHub's would stay the target's branch on GitHub.
    {
        let repo = ctx.repo.get()?;
        if repo
            .try_find_reference(format!("refs/heads/{MAIN}").as_str())?
            .is_some()
        {
            but_api::gitbutler_code::track(&repo, MAIN, HOME_REMOTE)?;
        }
    }
    reload(ctx)?;
    crate::command::config::set_target(ctx, &format!("{HOME_REMOTE}/{MAIN}"), None)?;
    let copy = but_api::gitbutler_code::setup_copy(ctx, account, &domain, &name).await?;

    Ok(MoveOutcome {
        canonical,
        canonical_url,
        created,
        already: moved,
        copy,
    })
}

/// What the server said about a refused push (its `remote:` lines), else the whole error.
fn server_reason(err: &anyhow::Error) -> String {
    let full = format!("{err:#}");
    let mut said: Vec<&str> = Vec::new();
    for line in full.lines() {
        if let Some(message) = line.trim().strip_prefix("remote:") {
            let message = message.trim();
            if !message.is_empty() && !said.contains(&message) {
                said.push(message);
            }
        }
    }
    if said.is_empty() {
        full
    } else {
        format!("GitButler Code says {}", said.join("; "))
    }
}

fn remote_url(ctx: &Context, name: &str) -> Result<Option<String>> {
    let repo = ctx.repo.get()?;
    let Ok(remote) = repo.find_remote(name) else {
        return Ok(None);
    };
    Ok(remote
        .url(gix::remote::Direction::Fetch)
        .map(|url| url.to_bstring().to_string()))
}

/// Add the remote `name` at `url`, or check that the one there points at it.
fn ensure_remote(ctx: &mut Context, name: &str, url: &str) -> CliResult<()> {
    match remote_url(ctx, name)? {
        Some(existing) if but_api::gitbutler_code::same_url(&existing, url) => Ok(()),
        Some(existing) => Err(bad_input(format!(
            "The remote {name} points at {existing}, not {url}"
        ))
        .hint(format!(
            "Rename or remove the remote {name}, then run this again"
        ))
        .into()),
        None => {
            but_api::legacy::forge::add_remote_to_config(&*ctx.repo.get()?, name, url)?;
            reload(ctx)?;
            Ok(())
        }
    }
}

/// Read the repository's configuration again after changing its remotes.
fn reload(ctx: &mut Context) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.reload_repo_and_invalidate_workspace(guard.write_permission())
}

#[must_use]
struct MoveOutcome {
    canonical: String,
    canonical_url: String,
    /// Whether this run created the organization's project.
    created: bool,
    /// Whether the repository was on GitButler Code before this run.
    already: bool,
    copy: but_api::gitbutler_code::Copy,
}

impl CliOutputHuman for MoveOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _agent: bool, t: &'static Theme) -> Result<()> {
        let Self {
            canonical: _,
            canonical_url,
            created,
            already,
            copy,
        } = self;
        let web = |url: &str| url.trim_end_matches(".git").to_owned();
        let new = |yes: bool| if yes { " (new)" } else { "" };
        let headline = if already {
            "You're already on GitButler Code"
        } else {
            "You're on GitButler Code!"
        };
        writeln!(out, "{} {}", t.sym().success, t.success.paint(headline))?;
        writeln!(out)?;
        writeln!(
            out,
            "  Project    {}{}",
            t.config_value.paint(web(&canonical_url)),
            t.hint.paint(new(created))
        )?;
        writeln!(
            out,
            "  Your copy  {}{}",
            t.config_value.paint(web(&copy.space.url)),
            t.hint.paint(new(copy.space.created))
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "{} sends your branches to your copy, where only you can see them.",
            t.command_suggestion.paint("but push")
        )?;
        writeln!(
            out,
            "Open pull requests on the project's page. {}",
            t.hint.paint(if already {
                "GitHub stays reachable as the `github` remote."
            } else {
                "GitHub is still there as the `github` remote."
            })
        )?;
        Ok(())
    }
}

impl CliOutput for MoveOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            canonical: String,
            canonical_url: String,
            created: bool,
            copy: String,
            copy_url: String,
            copy_created: bool,
            target: String,
            push_remote: String,
        }
        Output {
            canonical: self.canonical,
            canonical_url: self.canonical_url,
            created: self.created,
            copy: self.copy.space.project,
            copy_url: self.copy.space.url,
            copy_created: self.copy.space.created,
            target: format!("{HOME_REMOTE}/{MAIN}"),
            push_remote: self.copy.remote,
        }
    }
}
