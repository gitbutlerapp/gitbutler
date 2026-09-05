use anyhow::Result;
use but_api::legacy::forge::{forge_info, forge_provider};
use but_forge::ForgeName;
use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn forge_state_follows_target_initialization() -> Result<()> {
    let (repo, tmp) = crate::support::repo_with_feature_branch()?;
    drop(repo);
    git_at_dir(tmp.path())
        .args(["branch", "-D", "feature"])
        .run();
    git_at_dir(tmp.path())
        .args([
            "config",
            "remote.origin.url",
            "git@gitlab.example.com:acme/widgets.git",
        ])
        .run();
    git_at_dir(tmp.path())
        .args([
            "config",
            "remote.fork.url",
            "https://github.com/acme/widgets.git",
        ])
        .run();

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    assert!(
        forge_info(&ctx)?.is_none(),
        "configured remotes are ambiguous until the single-branch project has a target"
    );
    assert!(
        forge_provider(&ctx)?.is_none(),
        "the provider follows the same targetless state"
    );

    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    assert_eq!(
        forge_provider(&ctx)?,
        Some(ForgeName::GitLab),
        "the target remote determines the forge"
    );

    let existing_ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    assert_eq!(
        forge_info(&existing_ctx)?.map(|info| info.name),
        Some(ForgeName::GitLab),
        "persisted target metadata remains available after reopening the project"
    );
    Ok(())
}

#[test]
fn list_reviews_reports_an_unrecognized_forge_as_a_typed_state() -> Result<()> {
    use but_api::legacy::forge::list_reviews;
    use but_error::AnyhowContextExt as _;

    let (repo, tmp) = crate::support::repo_with_feature_branch()?;
    drop(repo);
    git_at_dir(tmp.path())
        .args([
            "config",
            "remote.origin.url",
            "https://git.example.com/acme/widgets.git",
        ])
        .run();
    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    assert!(
        forge_info(&ctx)?.is_none(),
        "the capability gate agrees that no forge is recognized"
    );

    // The stale-cache fallback must not turn a missing forge identity into
    // "outage, serve the last listing": there is no forge to be stale about.
    for cache_config in [
        None,
        Some(but_forge::CacheConfig::CacheWithFallback {
            max_age_seconds: 300,
        }),
    ] {
        let err = list_reviews(&ctx, cache_config).expect_err("no forge, no listing");
        assert_eq!(
            err.to_string(),
            "No forge could be determined for this repository branch",
            "the message the CLI and Lite print stays the same"
        );
        assert_eq!(
            err.custom_context_or_error_chain().code.to_string(),
            "ForgeUnrecognized",
            "the wire code lets the desktop treat this as an expected, terminal state"
        );
    }
    Ok(())
}
