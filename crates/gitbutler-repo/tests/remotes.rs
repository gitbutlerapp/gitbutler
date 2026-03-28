use but_ctx::{Context, RepoOpenMode};
use but_settings::AppSettings;
use but_testsupport::legacy::test_repository;
use gitbutler_project as projects;
use gitbutler_repo::RepoCommands;
use tempfile::TempDir;

struct TestCtx {
    ctx: Context,
    _tmp: TempDir,
}

fn ctx() -> TestCtx {
    let (repo, tmp) = test_repository();
    let project = projects::Project::new_for_gitbutler_repo(
        repo.workdir().unwrap().to_path_buf(),
        projects::AuthKey::SystemExecutable,
    );
    TestCtx {
        ctx: Context::new_from_legacy_project_and_settings_with_repo_open_mode(
            &project,
            AppSettings::default(),
            RepoOpenMode::Isolated,
        )
        .expect("can create context"),
        _tmp: tmp,
    }
}

#[test]
fn add_remote_writes_fetch_url_to_local_config() -> anyhow::Result<()> {
    let test = ctx();
    let ctx = &test.ctx;

    ctx.add_remote("origin", "https://example.com/repo.git")?;

    let repo = ctx.open_isolated_repo()?;
    let remote = repo.find_remote("origin")?;
    assert_eq!(
        remote
            .url(gix::remote::Direction::Fetch)
            .map(|url| url.to_bstring().to_string()),
        Some("https://example.com/repo.git".to_owned())
    );
    Ok(())
}

#[test]
fn add_remote_rejects_duplicate_name() {
    let test = ctx();
    let ctx = &test.ctx;
    ctx.add_remote("origin", "https://example.com/repo.git")
        .expect("first add succeeds");

    let err = ctx
        .add_remote("origin", "https://example.com/other.git")
        .expect_err("duplicate name should fail");
    assert_eq!(err.to_string(), "Remote name 'origin' already exists");
}

#[test]
fn add_remote_rejects_duplicate_url() {
    let test = ctx();
    let ctx = &test.ctx;
    ctx.add_remote("origin", "https://example.com/repo.git")
        .expect("first add succeeds");

    let err = ctx
        .add_remote("upstream", "https://example.com/repo.git")
        .expect_err("duplicate url should fail");
    assert_eq!(
        err.to_string(),
        "Remote with url 'https://example.com/repo.git' already exists"
    );
}
