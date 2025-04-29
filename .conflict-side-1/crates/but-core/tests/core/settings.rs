mod git {
    use but_core::{GitConfigSettings, RepositoryExt};
    use but_testsupport::gix_testtools;

    #[test]
    fn set_git_settings() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        let actual = repo.git_settings()?;
        assert_eq!(
            actual,
            GitConfigSettings::default(),
            "by default, None of these are set in a new repository"
        );
        let expected = GitConfigSettings {
            gitbutler_sign_commits: Some(true),
            signing_key: Some("signing key".into()),
            signing_format: Some("signing format".into()),
            gpg_program: Some("gpg program".into()),
            gpg_ssh_program: Some("gpg ssh program".into()),
        };
        repo.set_git_settings(&expected)?;
        let actual = repo.git_settings()?;

        assert_eq!(
            actual, expected,
            "round-tripping should work, and so should serialization to disk"
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
            ..Default::default()
        };

        repo.set_git_settings(&expected)?;
        let actual = repo.git_settings()?;
        assert_eq!(
            actual, expected,
            "it only writes what is given (as changed)"
        );

        Ok(())
    }
}
