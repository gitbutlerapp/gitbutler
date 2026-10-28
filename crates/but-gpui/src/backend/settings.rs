//! Settings the app shares with GitButler's other clients, a project's own settings, and
//! the editors and terminals files can open in.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use but_api::legacy::{config, projects};
use but_core::settings::git::ui::GitConfigSettings;
use but_settings::AppSettingsWithDiskSync;

use super::ops::changes_source;
use super::{Forge, open};

/// A program to open files or the project in, by id and name.
pub type Program = (String, String);

pub fn editors() -> Vec<Program> {
    but_api::open::list_editors()
        .unwrap_or_default()
        .into_iter()
        .map(|editor| (editor.id, editor.name))
        .collect()
}

pub fn terminals() -> Vec<Program> {
    but_api::open::terminal::get_terminal_options_for_platform(std::env::consts::OS.into())
        .unwrap_or_default()
        .into_iter()
        .map(|terminal| (terminal.identifier, terminal.display_name))
        .collect()
}

/// Open `file` of the project at `path`, or of its linked `worktree`, in `editor` at `line`.
pub fn open_in_editor(
    path: &Path,
    worktree: Option<&bstr::BStr>,
    editor: &str,
    file: &str,
    line: Option<u32>,
) -> Result<()> {
    let mut ctx = open(path)?;
    let source = changes_source(worktree);
    but_api::open::open_in_program(&mut ctx, editor.to_owned(), source, file.to_owned(), line)
}

/// Open `file`, one of the app's own rather than a project's, in `editor`.
pub fn open_file_in_editor(editor: &str, file: &Path) -> Result<()> {
    let program = (but_api::open::list_program_specs().into_iter())
        .find(|program| program.id == editor && program.is_gui_editor())
        .with_context(|| format!("found no editor '{editor}'"))?;
    let file = but_api::open::program::OpenSpec::File(file.to_owned());
    but_api::open::program::open_in_program_unchecked(&program, file)
}

/// Open the project at `path` in `terminal`, or with none chosen, in the one the platform has.
pub fn open_in_terminal(path: &Path, terminal: Option<String>) -> Result<()> {
    let terminal = match terminal {
        Some(terminal) => terminal,
        None => {
            but_api::open::terminal::get_recommended_terminal_for_platform(
                std::env::consts::OS.into(),
            )?
            .context("found no terminal to open")?
            .identifier
        }
    };
    but_api::open::open_in_terminal(terminal, path.display().to_string())
}

/// How often to fetch in the background, in minutes; `None` when it's off.
pub fn fetch_interval() -> Option<u64> {
    let settings = app_settings().ok()?;
    let minutes = settings.get().ok()?.fetch.auto_fetch_interval_minutes;
    u64::try_from(minutes).ok().filter(|minutes| *minutes > 0)
}

pub fn set_fetch_interval(minutes: Option<u64>) -> Result<()> {
    let minutes = minutes.map_or(-1, |minutes| minutes as isize);
    app_settings()?.update_fetch(but_settings::api::FetchUpdate {
        auto_fetch_interval_minutes: Some(minutes),
    })
}

/// GitButler's feature flags, as the other apps share them.
fn feature_flags() -> Option<but_settings::app_settings::FeatureFlags> {
    Some(app_settings().ok()?.get().ok()?.feature_flags.clone())
}

/// Whether the desktop app opens this client rather than its web app when it starts.
pub fn opens_at_start() -> bool {
    feature_flags().is_some_and(|flags| flags.gpui)
}

pub fn set_opens_at_start(on: bool) -> Result<()> {
    app_settings()?.update_feature_flags(but_settings::api::FeatureFlagsUpdate { gpui: Some(on) })
}

/// What the Project section of the settings shows.
#[derive(Debug, Clone)]
pub struct ProjectSettings {
    pub sign_commits: bool,
    /// The forge the project's remote, or a forge account with its host, names.
    pub detected_forge: Option<Forge>,
    /// The forge picked for the project, which counts only when none is detected.
    pub forge_override: Option<Forge>,
    /// The project's remotes, and the one new branches are pushed to.
    pub remotes: Vec<String>,
    pub push_remote: Option<String>,
}

pub fn project_settings(path: &Path) -> Result<ProjectSettings> {
    let ctx = open(path)?;
    let git = config::get_gb_config(&ctx)?;
    let (detected_forge, forge_override) = super::reviews::detected_and_saved_forge(&ctx);
    let repo = ctx.repo.get()?;
    Ok(ProjectSettings {
        sign_commits: git.gitbutler_sign_commits.unwrap_or(false),
        detected_forge,
        forge_override,
        remotes: repo
            .remote_names()
            .iter()
            .map(ToString::to_string)
            .collect(),
        // None with no target yet, or one an older GitButler stored ambiguously, which the
        // settings still show around.
        push_remote: (ctx.project_meta().ok())
            .and_then(|meta| meta.try_push_remote_name(&repo).ok().flatten()),
    })
}

/// Push new branches to `remote`, as a fork whose target is the original repository's.
pub fn set_push_remote(path: &Path, remote: String) -> Result<()> {
    but_api::workspace::set_push_remote(&mut open(path)?, remote)
}

/// Pick the forge the project uses when its remote doesn't name one, or clear the pick
/// (`None`). Saved as GitButler's other apps save it.
pub fn set_forge_override(id: &str, forge: Option<Forge>) -> Result<()> {
    let id = id.parse().context("Invalid project id")?;
    let mut request = gitbutler_project::UpdateRequest::default_with_id(id);
    match forge {
        Some(forge) => request.forge_override = Some(forge.name().to_lowercase()),
        None => request.unset_forge_override = true,
    }
    projects::update_project(request).map(|_| ())
}

/// Change the project's name.
pub fn rename_project(id: &str, name: String) -> Result<()> {
    projects::update_project_settings(
        id.parse().context("Invalid project id")?,
        projects::ProjectSettingsUpdate {
            title: Some(name),
            description: None,
            force_push_protection: None,
            omit_certificate_check: None,
        },
    )
}

/// What Git lacks of the author it records commits with for the project at `path`, as on a
/// machine where `user.name` and `user.email` were never set; `None` when it has both.
pub fn author_missing(path: &Path) -> Result<Option<MissingAuthor>> {
    let ctx = open(path)?;
    let author = config::get_author_info(&ctx)?;
    if author.name.is_some() && author.email.is_some() {
        return Ok(None);
    }
    let repo = ctx.repo.get()?;
    let config = repo.config_snapshot();
    Ok(Some(MissingAuthor {
        name: config.string(gix::config::tree::User::NAME).is_none(),
        email: config.string(gix::config::tree::User::EMAIL).is_none(),
        here: config.boolean("user.useConfigOnly") == Some(true),
    }))
}

/// Which of the author's fields Git lacks, and where they go.
#[derive(Clone, Copy)]
pub struct MissingAuthor {
    pub name: bool,
    pub email: bool,
    /// In the repository's own config, rather than the user's global one: `user.useConfigOnly`
    /// is how Git is told to ask for an email in each new clone, for an email per repository.
    pub here: bool,
}

/// Make `name` and `email` the author where Git has none, in the user's global Git config, as
/// GitButler's desktop app does, or with `here` the email in the repository's, where
/// `user.useConfigOnly` keeps it, and a missing name globally still.
pub fn set_author(path: &Path, name: &str, email: &str, here: bool) -> Result<()> {
    use gix::config::{Source, tree::User};
    let ctx = open(path)?;
    let (name, email) = (name.trim(), email.trim());
    if !here {
        return config::store_author_globally_if_unset(&ctx, name.to_owned(), email.to_owned());
    }
    let repo = ctx.repo.get()?;
    let set = |source, key, value: &str| {
        let edit =
            |config: &mut gix::config::File| Ok(config.set_raw_value(key, value).map(drop)?);
        but_core::git_config::edit_config(Some(&repo), source, edit)
    };
    if repo.config_snapshot().string(User::NAME).is_none() {
        set(Source::User, User::NAME, name)?;
    }
    // Asked for only when missing: an empty one leaves the repository's own alone.
    if !email.is_empty() {
        set(Source::Local, User::EMAIL, email)?;
    }
    Ok(())
}

/// Only the fields set are written to the repository's own config; what it reads from the
/// user's global config stays there.
pub fn set_sign_commits(path: &Path, sign: bool) -> Result<()> {
    let git = GitConfigSettings {
        gitbutler_sign_commits: Some(sign),
        ..Default::default()
    };
    config::set_gb_config(&open(path)?, git)
}

/// Sign a test commit with the project's settings, to tell whether signing works, or why not
/// in the signing program's own words, as a failed commit says it.
pub fn test_signing(path: &Path) -> Result<()> {
    but_api::legacy::repo::check_signing_settings(&open(path)?)
        .map_err(|err| anyhow::anyhow!(super::signing_failure(&err)))?;
    Ok(())
}

/// Whether Git's credentials work: fetch from the target's remote, or with `push`, push an
/// empty branch to the remote branches are pushed to, as a fork's own, and delete it again,
/// as GitButler's other apps test them.
pub fn test_credentials(path: &Path, push: bool) -> Result<()> {
    let ctx = open(path)?;
    let (remote, branch, push_remote) = {
        let repo = ctx.repo.get()?;
        let meta = ctx.project_meta()?;
        let target = (meta.remote_target_ref(&repo)?)
            .context("The target branch has no remote to test against")?;
        let (remote, branch) =
            but_core::extract_remote_name_and_short_name(target.as_ref(), &repo.remote_names())
                .context("Couldn't tell which remote the target branch is on")?;
        (remote, branch, meta.try_push_remote_name(&repo)?)
    };
    if push {
        let remote = push_remote.unwrap_or(remote);
        but_api::legacy::git::git_test_push(&ctx, remote, branch.to_string())
    } else {
        but_api::legacy::git::git_test_fetch(&ctx, remote, None)
    }
}

const NO_COMMITS: &str =
    "The repository has no commits yet. Make the first one, then pick the target.";

/// The project's local branches, like `refs/heads/main`, to pick a target from, as the
/// desktop app offers them: the one named as a remote's default branch and tracking it first,
/// then others tracking it, then the rest by name, leaving out GitButler's own.
pub fn target_branches(path: &Path) -> Result<Vec<super::FullName>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    // Its first branch comes with its first commit, so there's nothing to list yet.
    anyhow::ensure!(!repo.head()?.is_unborn(), NO_COMMITS);
    let mut defaults = Vec::new();
    for reference in repo.references()?.remote_branches()?.filter_map(Result::ok) {
        if let gix::refs::TargetRef::Symbolic(default) = reference.target() {
            defaults.push(default.to_owned());
        }
    }
    let mut names: Vec<(u8, super::FullName)> = Vec::new();
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let name = reference.name();
        if name.as_bstr().starts_with(b"refs/heads/gitbutler/") {
            continue;
        }
        let upstream = repo.branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch);
        let rank = match upstream
            .and_then(Result::ok)
            .filter(|u| defaults.contains(u))
        {
            Some(default) if default.as_bstr().ends_with(name.shorten()) => 0,
            Some(_) => 1,
            None => 2,
        };
        names.push((rank, name.to_owned()));
    }
    names.sort();
    Ok(names.into_iter().map(|(_, name)| name).collect())
}

/// Make `target` what the workspace integrates into: a local branch's full ref, or a remote
/// one that a single local branch tracks, as the CLI takes it. The branch's upstream is what
/// pulls bring in. Only the project's settings change: a new project stays on the branch it
/// has checked out, and branches rebase onto a changed target at the next update. New
/// branches go on pushing where they did, as a fork's to the fork when the target becomes the
/// original repository's main, as the desktop app keeps it.
///
/// A remote branch no local branch tracks, as an older GitButler stored the target, gets the
/// one named as it to track it, created there if missing, so pulls go on bringing in what
/// they did.
pub fn set_target(path: &Path, target: &super::FullName) -> Result<()> {
    use but_core::ref_metadata::{LocalTrackingBranchMatch, find_local_tracking_branch};
    let mut ctx = open(path)?;
    if target.category() == Some(gix::refs::Category::RemoteBranch) {
        let tracked = {
            let _exclusive = ctx.exclusive_worktree_access();
            let repo = ctx.repo.get()?;
            anyhow::ensure!(!repo.head()?.is_unborn(), NO_COMMITS);
            match find_local_tracking_branch(target.as_ref(), &repo)? {
                LocalTrackingBranchMatch::None => Some(track(&repo, target.as_ref())?),
                _ => None,
            }
        };
        // Read again, with the branch tracking it, once the lock is let go, as setting the
        // target takes it again. Like any target change this isn't on the undo timeline, and
        // couldn't be: a snapshot reads the target, unreadable until now.
        if let Some(local) = tracked {
            return set_target(path, &local);
        }
    }
    let (local, push_remote) = {
        let repo = ctx.repo.get()?;
        anyhow::ensure!(!repo.head()?.is_unborn(), NO_COMMITS);
        let local = but_core::ref_metadata::resolve_local_target(target.as_ref(), &repo)?;
        // One picked stays as it is. One that follows the old target's remote is kept only
        // where it would change: one pushing where the new target fetches from goes on
        // following the target, as through a rename of that remote.
        let fetches_from = repo
            .branch_remote_name(
                local.as_local_full_name_ref().shorten(),
                gix::remote::Direction::Fetch,
            )
            .map(|name| name.as_bstr().to_string());
        let pushes_to = (ctx.project_meta().ok())
            .filter(|meta| meta.push_remote.is_none())
            .and_then(|meta| meta.try_push_remote_name(&repo).ok().flatten())
            .filter(|to| Some(to) != fetches_from.as_ref());
        (local, pushes_to)
    };
    but_api::workspace::set_target_ref_and_init_project(
        &mut ctx,
        local.as_local_full_name_ref(),
        push_remote,
    )
}

/// Make the local branch named as the remote branch `remote`, like main for origin/main, track
/// it, as `git branch --set-upstream-to` does, creating it there first if it's missing.
fn track(repo: &gix::Repository, remote: &gix::refs::FullNameRef) -> Result<super::FullName> {
    let local = but_workspace::branch::local_tracking_branch(repo, remote)?;
    let (merge, from) = (repo.upstream_branch_and_remote_for_tracking_branch(remote)?)
        .with_context(|| format!("No remote fetches {}", remote.shorten()))?;
    let from = from.name().context("an anonymous remote fetches nothing")?;
    but_core::git_config::edit_repo_config(repo, gix::config::Source::Local, |config| {
        for (key, value) in [
            ("remote", from.as_bstr().to_string()),
            ("merge", merge.to_string()),
        ] {
            let key = format!("branch.{}.{key}", local.shorten());
            but_core::git_config::set_config_value(config, &key, &value)?;
        }
        Ok(())
    })?;
    Ok(local)
}

pub use but_api::ai::{AiConfiguration, AiConfigurationUpdate};

pub fn ai_configuration() -> Result<AiConfiguration> {
    but_api::ai::get_ai_configuration()
}

pub fn update_ai(update: AiConfigurationUpdate) -> Result<AiConfiguration> {
    but_api::ai::update_ai_configuration(update)
}

/// Where GitButler keeps its settings, for the build's channel.
pub fn config_dir() -> Result<PathBuf> {
    but_path::app_config_dir()
}

/// The build's channel, as About names it.
pub fn channel() -> &'static str {
    match but_path::AppChannel::new() {
        but_path::AppChannel::Release => "release",
        but_path::AppChannel::Nightly => "nightly",
        but_path::AppChannel::Dev => "development",
    }
}

/// The build is one of its own, as `cargo run` makes, not a nightly or a release.
pub fn dev_build() -> bool {
    but_path::AppChannel::new() == but_path::AppChannel::Dev
}

/// The version the desktop app's update `feed` offers in place of this build's, or `None`
/// when this build is the latest, which the feed says with no content.
pub fn newer_version(feed: &str) -> Result<Option<String>> {
    #[derive(serde::Deserialize)]
    struct Release {
        version: String,
    }
    // With a timeout, so a check that hangs doesn't hold up the later ones; no content
    // means it's up to date.
    let body = super::accounts::fetch(feed)?;
    if body.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice::<Release>(&body)?.version))
}

/// One of GitButler's releases, as the downloads API lists it.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Published {
    pub version: String,
    /// Markdown, when it has notes.
    pub notes: Option<String>,
    /// When it came out, in RFC 3339.
    pub released_at: String,
}

/// GitButler's newest releases, newest first, from the downloads API the website reads too.
/// They're kept for [`cached_releases`], as the API takes most of a second to answer.
pub fn releases() -> Result<Vec<Published>> {
    let url = "https://app.gitbutler.com/api/downloads?limit=40&channel=release";
    let releases: Vec<Published> = serde_json::from_slice(&super::accounts::fetch(url)?)?;
    // Without the downloads the API lists too, which are most of what it sends.
    let kept = releases_file().and_then(|file| {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Ok(std::fs::write(file, serde_json::to_vec(&releases)?)?)
    });
    if let Err(err) = kept {
        tracing::debug!("couldn't keep the release notes: {err:#}");
    }
    Ok(releases)
}

/// The releases as [`releases`] last read them.
pub fn cached_releases() -> Result<Vec<Published>> {
    Ok(serde_json::from_slice(&std::fs::read(releases_file()?)?)?)
}

fn releases_file() -> Result<PathBuf> {
    Ok(but_path::app_cache_dir()?.join("release-notes.json"))
}

/// GitButler's app settings as they are on disk now. Read afresh each time: the other apps
/// change them too, and saving a copy read earlier would put their changes back.
pub(super) fn app_settings() -> Result<AppSettingsWithDiskSync> {
    let config_dir = config_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    AppSettingsWithDiskSync::new_with_customization(config_dir, None)
}

#[cfg(test)]
mod tests {
    use crate::backend::ops::tests::{git, repo_with};

    #[test]
    fn a_legacy_target_no_branch_tracks_is_picked_by_the_branch_named_as_it_tracking_it() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::model::build::full;
            let dir = repo_with("one\n");
            let path = dir.path();
            git(
                path,
                &["remote", "add", "origin", "https://example.com/repo.git"],
            );
            git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
            git(path, &["update-ref", "refs/remotes/origin/release", "HEAD"]);
            // As an older GitButler stored it, with main pushed without `-u`.
            let legacy = |target: &str| {
                git(path, &["config", "gitbutler.project.targetRef", target]);
                git(path, &["config", "gitbutler.project.portedMeta", "true"]);
                let err = crate::backend::load_workspace(path, Default::default()).unwrap_err();
                assert_eq!(
                    crate::backend::read_failure(&err),
                    "Choose which branch is its target"
                );
                let target = crate::backend::unresolved_target(&err).unwrap();
                crate::backend::target_choice(path, target).unwrap()
            };

            let choice = legacy("refs/remotes/origin/main");
            assert_eq!(
                choice.to_track,
                Some((full("refs/heads/main"), false)),
                "main is offered to track origin/main, as no branch does"
            );
            super::set_target(path, &choice.remote).unwrap();
            assert_eq!(
                git(path, &["rev-parse", "--abbrev-ref", "main@{upstream}"]).trim(),
                "origin/main",
                "main tracks the old target, so pulls bring in what they did"
            );
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            assert_eq!(
                workspace.target.map(|target| target.name).as_deref(),
                Some("origin/main"),
                "the project reads again, its target going by its upstream"
            );

            let choice = legacy("refs/remotes/origin/release");
            assert_eq!(
                choice.to_track,
                Some((full("refs/heads/release"), true)),
                "a branch the remote alone has is offered too"
            );
            super::set_target(path, &choice.remote).unwrap();
            assert_eq!(
                git(path, &["rev-parse", "--abbrev-ref", "release@{upstream}"]).trim(),
                "origin/release",
                "picking it creates it, tracking the old target"
            );
            assert_eq!(
                git(path, &["config", "gitbutler.project.targetRef"]).trim(),
                "refs/heads/release"
            );
        })
    }

    #[test]
    fn the_default_branch_lists_first_and_picking_it_stays_on_the_branch() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            git(
                path,
                &["remote", "add", "origin", "https://example.com/repo.git"],
            );
            git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
            git(
                path,
                &[
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    "refs/remotes/origin/main",
                ],
            );
            git(path, &["update-ref", "refs/remotes/origin/feature", "HEAD"]);
            git(path, &["branch", "--set-upstream-to=origin/main", "main"]);
            git(path, &["branch", "another"]);
            git(path, &["branch", "gitbutler/edit"]);

            assert_eq!(
                super::target_branches(path).unwrap(),
                ["refs/heads/main", "refs/heads/another"].map(crate::backend::model::build::full),
                "the branch tracking the remote's default first, and none of GitButler's own"
            );
            super::set_target(
                path,
                &crate::backend::model::build::full("refs/remotes/origin/main"),
            )
            .unwrap();
            assert_eq!(
                git(path, &["symbolic-ref", "HEAD"]).trim(),
                "refs/heads/main",
                "picking a target leaves the checked-out branch alone, as the desktop app does"
            );
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            assert!(!workspace.managed);
            assert_eq!(
                workspace.target.map(|target| target.name).as_deref(),
                Some("origin/main"),
                "a remote ref picks the local branch tracking it, which goes by its upstream"
            );
        })
    }

    #[test]
    fn a_main_pushed_without_tracking_says_what_it_doesnt_track() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            // As `git push origin main` leaves it, without `-u`.
            git(
                path,
                &["remote", "add", "origin", "https://example.com/r.git"],
            );
            git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
            super::set_target(path, &crate::backend::model::build::full("refs/heads/main"))
                .unwrap();
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            assert_eq!(
                workspace
                    .target
                    .and_then(|target| target.untracked)
                    .as_deref(),
                Some("origin/main"),
                "what's pushed to origin/main doesn't count, so it's said why"
            );
        })
    }

    #[test]
    fn a_fork_targeting_the_original_repository_keeps_pushing_to_the_fork() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            for remote in ["origin", "upstream"] {
                let url = format!("https://example.com/{remote}.git");
                git(path, &["remote", "add", remote, &url]);
                git(
                    path,
                    &["update-ref", &format!("refs/remotes/{remote}/main"), "HEAD"],
                );
            }
            git(path, &["branch", "--set-upstream-to=origin/main", "main"]);
            git(
                path,
                &["branch", "--track", "upstream-main", "upstream/main"],
            );
            super::set_target(path, &crate::backend::model::build::full("refs/heads/main"))
                .unwrap();

            super::set_target(
                path,
                &crate::backend::model::build::full("refs/heads/upstream-main"),
            )
            .unwrap();
            let settings = super::project_settings(path).unwrap();
            assert_eq!(settings.remotes, ["origin", "upstream"]);
            assert_eq!(
                settings.push_remote.as_deref(),
                Some("origin"),
                "new branches go on to the fork, not to the original repository now targeted"
            );
            super::set_push_remote(path, "upstream".into()).unwrap();
            assert_eq!(
                super::project_settings(path)
                    .unwrap()
                    .push_remote
                    .as_deref(),
                Some("upstream"),
                "picking where branches go is saved"
            );
            git(path, &["remote", "remove", "upstream"]);
            super::set_target(path, &crate::backend::model::build::full("refs/heads/main"))
                .expect("a picked remote that's gone doesn't keep the target from changing");
        })
    }
}
