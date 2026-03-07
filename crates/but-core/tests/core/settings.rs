mod git {
    use but_core::{GitConfigSettings, RepositoryExt};
    use but_testsupport::gix_testtools;
    use std::path::Path;

    fn storage_key() -> String {
        match option_env!("CHANNEL") {
            Some("release") => "gitbutler.storagePath".to_string(),
            Some(channel) => format!("gitbutler.{channel}.storagePath"),
            None => "gitbutler.dev.storagePath".to_string(),
        }
    }

    #[test]
    fn set_git_settings() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        let actual = repo.git_settings()?;
        assert_eq!(
            actual,
            GitConfigSettings {
                gitbutler_sign_commits: Some(false),
                ..GitConfigSettings {
                    gitbutler_gerrit_mode: Some(false),
                    ..Default::default()
                }
            },
            "by default, None of these are set in a new repository, except for the explicit gpg-sign logic"
        );
        let expected = GitConfigSettings {
            gitbutler_sign_commits: Some(true),
            gitbutler_gerrit_mode: Some(false),
            gitbutler_forge_review_template_path: None,
            gitbutler_gitlab_project_id: None,
            gitbutler_gitlab_upstream_project_id: None,
            signing_key: Some("signing key".into()),
            signing_format: Some("signing format".into()),
            gpg_program: Some("gpg program".into()),
            gpg_ssh_program: Some("gpg ssh program".into()),
        };
        repo.set_git_settings(&expected)?;
        let actual = repo.git_settings()?;

        assert_ne!(
            actual, expected,
            "round-tripping isn't possible due to the way this works - it would need mutability."
        );

        let repo = but_testsupport::open_repo(repo.path())?;
        let actual = repo.git_settings()?;
        assert_eq!(
            actual, expected,
            "but it works once the settings are reloaded, they were persisted to disk."
        );
        Ok(())
    }

    #[test]
    fn set_partial_git_settings() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        let expected = GitConfigSettings {
            gitbutler_sign_commits: Some(true),
            ..GitConfigSettings {
                gitbutler_gerrit_mode: Some(false),
                ..Default::default()
            }
        };

        // need a reload, see `set_git_settings` for details on why.
        repo.set_git_settings(&expected)?;

        let repo = but_testsupport::open_repo(repo.path())?;
        let actual = repo.git_settings()?;
        assert_eq!(
            actual, expected,
            "it only writes what is given (as changed)"
        );

        Ok(())
    }

    #[test]
    fn storage_path_from_absolute_config() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;

        let custom_path = if cfg!(windows) {
            Path::new(r"C:\gitbutler-storage")
        } else {
            Path::new("/tmp/gitbutler-storage")
        };
        let custom = custom_path
            .to_str()
            .expect("test path should be utf-8")
            .to_owned();
        let key = storage_key();
        git2::Repository::open(repo.path())?
            .config()?
            .set_str(&key, custom.as_str())?;

        let repo = gix::open_opts(repo.path(), gix::open::Options::isolated())?;

        assert_eq!(repo.gitbutler_storage_path()?, custom_path);
        Ok(())
    }

    #[test]
    fn storage_path_from_relative_config() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        let key = storage_key();

        git2::Repository::open(repo.path())?
            .config()?
            .set_str(&key, "gitbutler-custom")?;
        let repo = gix::open_opts(repo.path(), gix::open::Options::isolated())?;

        assert_eq!(
            repo.gitbutler_storage_path()?,
            repo.git_dir().join("gitbutler-custom")
        );
        Ok(())
    }
}
