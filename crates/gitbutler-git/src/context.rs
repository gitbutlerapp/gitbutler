use std::{path::Path, time::UNIX_EPOCH};

use anyhow::{Context as _, Result, anyhow};
use but_askpass as askpass;
use but_core::{extract_remote_name_and_short_name, ref_metadata::StackId};
use but_ctx::Context;
use but_error::Code;
use serde::Serialize;

/// Summary information about branches pushed to a remote.
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    /// The name of the remote the push defaulted to.
    ///
    /// A branch may track a different remote, so read the remote off each entry's
    /// refname in [`Self::branch_to_remote`] rather than this one when acting on a branch.
    pub remote: String,
    /// The list of pushed branches with their remote refnames and the branch name on the remote.
    ///
    /// Format: `(branch_name, remote_refname, remote_branch_name)`. The last element is the
    /// refname with `refs/remotes/<remote>/` already stripped, so callers never have to
    /// know where the remote name ends.
    #[serde(serialize_with = "serialize_branch_to_remote")]
    pub branch_to_remote: Vec<(String, gix::refs::FullName, String)>,
    /// The list of branches with their before/after commit SHAs.
    ///
    /// Format: `(branch_name, before_sha, after_sha)`.
    pub branch_sha_updates: Vec<(String, String, String)>,
}

/// Higher-level fetch and push helpers implemented for [`Context`].
#[expect(clippy::too_many_arguments)]
pub trait GitContextExt {
    /// Fetch from the given remote using its configured fetch refspecs.
    fn fetch(&self, remote_name: &str, askpass: Option<String>) -> Result<()>;

    /// Push the given commit to the provided remote branch.
    ///
    /// Returns the stderr output of the Git executable if used.
    fn push<B>(
        &self,
        head: gix::ObjectId,
        branch: B,
        with_force: bool,
        force_push_protection: bool,
        refspec: Option<String>,
        askpass_broker: Option<Option<StackId>>,
        push_opts: Vec<String>,
    ) -> Result<String>
    where
        B: TryInto<gix::refs::FullName>,
        B::Error: Into<anyhow::Error>;

    /// Push a temporary branch to the remote and immediately delete it again.
    fn git_test_push(
        &self,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<StackId>>,
    ) -> Result<()>;
}

impl GitContextExt for Context {
    fn fetch(&self, remote_name: &str, askpass: Option<String>) -> Result<()> {
        fetch_with_askpass(self.workdir_or_gitdir()?, remote_name, askpass)
    }

    fn push<B>(
        &self,
        head: gix::ObjectId,
        branch: B,
        with_force: bool,
        force_push_protection: bool,
        refspec: Option<String>,
        askpass_broker: Option<Option<StackId>>,
        push_opts: Vec<String>,
    ) -> Result<String>
    where
        B: TryInto<gix::refs::FullName>,
        B::Error: Into<anyhow::Error>,
    {
        push_with_askpass(
            &*self.repo.get()?,
            head,
            branch,
            with_force,
            force_push_protection,
            refspec,
            askpass_broker,
            push_opts,
        )
    }

    fn git_test_push(
        &self,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<StackId>>,
    ) -> Result<()> {
        let target_branch_refname: gix::refs::FullName =
            format!("refs/remotes/{remote_name}/{branch_name}").try_into()?;
        let repo = self.repo.get()?;
        let mut branch = repo
            .try_find_reference(&target_branch_refname.to_string())?
            .ok_or(anyhow!("failed to find branch {target_branch_refname}"))?;

        let commit_id = branch.peel_to_commit()?.id;
        let branch_name = format!("test-push-{}", now_ms());
        let refname: gix::refs::FullName =
            format!("refs/remotes/{remote_name}/{branch_name}").try_into()?;

        self.push(
            commit_id,
            refname.clone(),
            false,
            false,
            None,
            askpass,
            vec![],
        )
        .map_err(|err| anyhow!(err.to_string()))?;

        let empty_refspec = Some(format!(":refs/heads/{branch_name}"));
        self.push(
            commit_id,
            refname,
            false,
            false,
            empty_refspec,
            askpass,
            vec![],
        )
        .map_err(|err| anyhow!(err.to_string()))?;

        Ok(())
    }
}

/// Fetch from `remote_name`, forwarding credential prompts through the application askpass broker
/// when it is enabled.
///
/// The fetch runs on its own thread and runtime so synchronous API callers don't block the runtime
/// responsible for delivering askpass responses.
pub fn fetch_with_askpass(
    repo_path: impl AsRef<Path>,
    remote_name: &str,
    action: Option<String>,
) -> Result<()> {
    let on_prompt = if askpass::get_broker().is_some() {
        Some(move |prompt: String| handle_git_prompt_fetch(prompt, action.clone()))
    } else {
        None
    };

    let repo_path = repo_path.as_ref().to_owned();
    let remote = remote_name.to_owned();
    let result = std::thread::spawn(move || -> Result<_> {
        let runtime = tokio::runtime::Runtime::new().context(
            but_error::Context::new("failed to initialize async runtime for git fetch")
                .with_code(Code::Unknown),
        )?;
        Ok(runtime.block_on(crate::fetch(
            repo_path,
            crate::tokio::TokioExecutor,
            &remote,
            on_prompt,
        )))
    })
    .join()
    .map_err(|panic| {
        let reason = if let Some(message) = panic.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = panic.downcast_ref::<&'static str>() {
            (*message).to_owned()
        } else {
            "unknown panic payload".to_owned()
        };

        anyhow!("git fetch worker thread panicked: {reason}").context(
            but_error::Context::new("git fetch failed unexpectedly").with_code(Code::Unknown),
        )
    })??;
    result.map_err(map_needs_authorization)
}

/// The concrete error type produced by fetch/push through the tokio executor.
type GitError = crate::Error<crate::repository::Error<crate::tokio::TokioExecutor>>;

/// Convert the error into an anyhow error, giving `NeedsAuthorization` failures a message that
/// tells the user what to do about it (see [`needs_authorization_message`]); every other error
/// converts as-is.
fn map_needs_authorization(err: GitError) -> anyhow::Error {
    let crate::Error::Backend(crate::repository::RepositoryError::NeedsAuthorization(ref prompt)) =
        err
    else {
        return err.into();
    };
    let context = but_error::Context::new(needs_authorization_message(prompt, &err))
        .with_code(Code::ProjectGitAuth);
    anyhow::Error::from(err).context(context)
}

/// Turn the raw askpass prompt of a `NeedsAuthorization` failure into a message that tells the
/// user what to do about it. Without this, the user sees the prompt Git wanted answered
/// (e.g. `Username for 'https://github.com': `) with no hint at the actual problem: nothing could
/// answer the prompt, most commonly because no credential helper is configured. The original error is appended since only this
/// message reaches the app, while the error chain stays behind in the logs.
fn needs_authorization_message(prompt: &str, original_error: impl std::fmt::Display) -> String {
    let prompt = prompt.trim();
    let credential_url = prompt
        .strip_prefix("Username for ")
        .or_else(|| prompt.strip_prefix("Password for "))
        .map(|url| url.trim_end_matches(':').trim().trim_matches('\''));
    let advice = match credential_url {
        Some(url) => format!(
            "Git couldn't obtain credentials for {url}. Configure a git credential helper (for example Git Credential Manager), or switch the remote to SSH."
        ),
        None => format!("Git asked for input and none was provided: {prompt}"),
    };
    format!("{advice}\n\nOriginal error: {original_error}")
}

/// Push the given commit to the provided remote branch.
///
/// Returns the stderr output of the Git executable if used.
#[allow(clippy::too_many_arguments)]
pub fn push_with_askpass<B>(
    repo: &gix::Repository,
    head: gix::ObjectId,
    branch: B,
    with_force: bool,
    force_push_protection: bool,
    refspec: Option<String>,
    askpass_broker: Option<Option<but_core::Id<'S'>>>,
    push_opts: Vec<String>,
) -> Result<String>
where
    B: TryInto<gix::refs::FullName>,
    B::Error: Into<anyhow::Error>,
{
    let branch: gix::refs::FullName = branch.try_into().map_err(Into::into)?;
    let (remote, branch_name) = remote_tracking_branch_parts(repo, branch.as_ref())?;
    let refspec = refspec.unwrap_or_else(|| format!("{head}:refs/heads/{branch_name}"));

    let on_prompt = if askpass::get_broker().is_some() {
        Some(move |prompt: String| handle_git_prompt_push(prompt, askpass_broker))
    } else {
        None
    };

    let repo_path = repo.git_dir().to_owned();
    let result = std::thread::spawn(move || -> Result<_> {
        let runtime = tokio::runtime::Runtime::new().context(
            but_error::Context::new("failed to initialize async runtime for git push")
                .with_code(Code::Unknown),
        )?;
        let refspec = crate::RefSpec::parse(&refspec).context(
            but_error::Context::new(format!("failed to parse git push refspec `{refspec}`"))
                .with_code(Code::Validation),
        )?;
        Ok(runtime.block_on(crate::push(
            repo_path,
            crate::tokio::TokioExecutor,
            &remote,
            refspec,
            with_force,
            force_push_protection,
            on_prompt,
            push_opts,
        )))
    })
    .join()
    .map_err(|panic| {
        let reason = if let Some(message) = panic.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = panic.downcast_ref::<&'static str>() {
            (*message).to_owned()
        } else {
            "unknown panic payload".to_owned()
        };

        anyhow!("git push worker thread panicked: {reason}").context(
            but_error::Context::new("git push failed unexpectedly").with_code(Code::Unknown),
        )
    })??;
    match result {
            Ok(stderr) => Ok(stderr),
            Err(err) => match err {
                crate::Error::ForcePushProtection(e) => Err(anyhow!(
                    "The force push was blocked because the remote branch contains commits that would be overwritten.\n\n{e}"
                )
                .context(Code::GitForcePushProtection)),
                crate::Error::GerritNoNewChanges(_) => {
                    // Treat "no new changes" as success for Gerrit.
                    Ok(String::new())
                }
                crate::Error::NonFastForward(_) => Err(err).context(Code::GitNonFastForward),
                _ => Err(map_needs_authorization(err)),
            },
        }
}

fn serialize_branch_to_remote<S>(
    branch_to_remote: &[(String, gix::refs::FullName, String)],
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    branch_to_remote
        .iter()
        .map(|(branch_name, refname, remote_branch_name)| {
            (branch_name, refname.to_string(), remote_branch_name)
        })
        .collect::<Vec<_>>()
        .serialize(serializer)
}

fn remote_tracking_branch_parts(
    repo: &gix::Repository,
    branch: &gix::refs::FullNameRef,
) -> Result<(String, String)> {
    let (remote, short_name) = extract_remote_name_and_short_name(branch, &repo.remote_names())
        .ok_or_else(|| anyhow!("failed to determine remote and branch name for `{branch}`"))?;
    let short_name = std::str::from_utf8(short_name.as_ref())
        .context(format!("branch name for `{branch}` is not valid UTF-8"))?
        .to_owned();
    Ok((remote, short_name))
}

fn now_ms() -> u128 {
    UNIX_EPOCH
        .elapsed()
        .expect("system time is set before the Unix epoch")
        .as_millis()
}

async fn handle_git_prompt_push(
    prompt: String,
    askpass: Option<Option<StackId>>,
) -> Option<String> {
    if let Some(branch_id) = askpass {
        tracing::info!("received prompt for branch push {branch_id:?}: {prompt:?}");
        askpass::get_broker()
            .expect("askpass broker must be initialized")
            .submit_prompt(prompt, askpass::Context::Push { branch_id })
            .await
    } else {
        tracing::warn!("received askpass push prompt but no broker was supplied; returning None");
        None
    }
}

async fn handle_git_prompt_fetch(prompt: String, askpass: Option<String>) -> Option<String> {
    if let Some(action) = askpass {
        tracing::info!("received prompt for fetch with action {action:?}: {prompt:?}");
        askpass::get_broker()
            .expect("askpass broker must be initialized")
            .submit_prompt(prompt, askpass::Context::Fetch { action })
            .await
    } else {
        tracing::warn!("received askpass fetch prompt but no broker was supplied; returning None");
        None
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use but_testsupport::{gix_testtools, open_repo};

    use super::{needs_authorization_message, remote_tracking_branch_parts};

    fn repo_with_registered_remotes() -> anyhow::Result<gix::Repository> {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../but-core/tests/fixtures/scenario/multiple-remotes-with-tracking-branches.sh");
        let root = gix_testtools::scripted_fixture_read_only(fixture_path)
            .map_err(anyhow::Error::from_boxed)?;
        Ok(open_repo(&root)?.with_object_memory())
    }

    #[test]
    fn needs_authorization_message_extracts_credential_url() {
        for prompt in [
            "Username for 'https://github.com': ",
            "Password for 'https://github.com': ",
        ] {
            assert_eq!(
                needs_authorization_message(prompt, "backend error: original"),
                "Git couldn't obtain credentials for https://github.com. Configure a git credential helper (for example Git Credential Manager), or switch the remote to SSH.\n\nOriginal error: backend error: original"
            );
        }
    }

    #[test]
    fn needs_authorization_message_keeps_unrecognized_prompts() {
        assert_eq!(
            needs_authorization_message(
                "Enter passphrase for key '/home/user/.ssh/id_ed25519': ",
                "backend error: original"
            ),
            "Git asked for input and none was provided: Enter passphrase for key '/home/user/.ssh/id_ed25519':\n\nOriginal error: backend error: original"
        );
    }

    #[test]
    fn remote_tracking_branch_parts_handles_registered_remote_with_slashes() -> anyhow::Result<()> {
        let repo = repo_with_registered_remotes()?;
        let branch: &gix::refs::FullNameRef = "refs/remotes/nested/remote/feature/a".try_into()?;

        let (remote, short_name) = remote_tracking_branch_parts(&repo, branch)?;

        assert_eq!(remote, "nested/remote");
        assert_eq!(short_name, "feature/a");
        Ok(())
    }

    #[test]
    fn remote_tracking_branch_parts_rejects_ambiguous_unregistered_remote() -> anyhow::Result<()> {
        let repo = repo_with_registered_remotes()?;
        let branch: &gix::refs::FullNameRef =
            "refs/remotes/nested/non-existing/feature".try_into()?;

        let err = remote_tracking_branch_parts(&repo, branch).unwrap_err();

        assert_eq!(
            err.to_string(),
            "failed to determine remote and branch name for `refs/remotes/nested/non-existing/feature`"
        );
        Ok(())
    }
}
