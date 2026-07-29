mod git {
    use std::io::Write;

    use but_core::{
        GitConfigSettings, GitHubStackingMode, RepositoryExt, ReviewStackingDescription,
    };
    use but_testsupport::gix_testtools;

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
            gitbutler_review_stacking_description: None,
            gitbutler_github_stacking_mode: None,
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
    fn empty_strings_remove_existing_values() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;

        repo.set_git_settings(&GitConfigSettings {
            gitbutler_sign_commits: Some(true),
            gitbutler_gerrit_mode: Some(false),
            gitbutler_review_stacking_description: None,
            gitbutler_github_stacking_mode: None,
            gitbutler_forge_review_template_path: Some("template.md".into()),
            gitbutler_gitlab_project_id: Some("project-id".into()),
            gitbutler_gitlab_upstream_project_id: Some("upstream-project-id".into()),
            signing_key: Some("signing key".into()),
            signing_format: Some("ssh".into()),
            gpg_program: Some("gpg".into()),
            gpg_ssh_program: Some("ssh-keygen".into()),
        })?;

        repo.set_git_settings(&GitConfigSettings {
            gitbutler_sign_commits: Some(true),
            gitbutler_gerrit_mode: Some(false),
            gitbutler_review_stacking_description: None,
            gitbutler_github_stacking_mode: None,
            gitbutler_forge_review_template_path: Some("".into()),
            gitbutler_gitlab_project_id: Some(String::new()),
            gitbutler_gitlab_upstream_project_id: Some(String::new()),
            signing_key: Some("".into()),
            signing_format: Some("".into()),
            gpg_program: Some("".into()),
            gpg_ssh_program: Some("".into()),
        })?;

        let repo = but_testsupport::open_repo(repo.path())?;
        let actual = repo.git_settings()?;
        assert_eq!(actual.gitbutler_forge_review_template_path, None);
        assert_eq!(actual.gitbutler_gitlab_project_id, None);
        assert_eq!(actual.gitbutler_gitlab_upstream_project_id, None);
        assert_eq!(actual.signing_key, None);
        assert_eq!(actual.signing_format, None);
        assert_eq!(actual.gpg_program, None);
        assert_eq!(actual.gpg_ssh_program, None);

        let config = repo.config_snapshot();
        assert!(
            config.string("gitbutler.forgeReviewTemplatePath").is_none(),
            "expected empty template path to remove the config key"
        );
        assert!(
            config.string("gitbutler.gitlabProjectId").is_none(),
            "expected empty project ID to remove the config key"
        );
        assert!(
            config.string("gitbutler.gitlabUpstreamProjectId").is_none(),
            "expected empty upstream project ID to remove the config key"
        );
        assert!(
            config.string("user.signingKey").is_none(),
            "expected empty signing key to remove the config key"
        );
        assert!(
            config.string("gpg.format").is_none(),
            "expected empty signing format to remove the config key"
        );
        assert!(
            config.trusted_program("gpg.program").is_none(),
            "expected empty gpg program to remove the config key"
        );
        assert!(
            config.trusted_program("gpg.ssh.program").is_none(),
            "expected empty gpg ssh program to remove the config key"
        );

        Ok(())
    }

    #[test]
    fn review_stacking_description_round_trips() -> anyhow::Result<()> {
        for value in [
            ReviewStackingDescription::Bottom,
            ReviewStackingDescription::Top,
            ReviewStackingDescription::Disabled,
        ] {
            let tmp = gix_testtools::tempfile::TempDir::new()?;
            gix::init(tmp.path())?;
            let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
            repo.set_git_settings(&GitConfigSettings {
                gitbutler_review_stacking_description: Some(value),
                ..Default::default()
            })?;

            let repo = but_testsupport::open_repo(repo.path())?;
            assert_eq!(
                repo.git_settings()?.gitbutler_review_stacking_description,
                Some(value),
                "the configured review stacking description should round-trip"
            );
        }
        Ok(())
    }

    #[test]
    fn github_stacking_mode_round_trips() -> anyhow::Result<()> {
        for value in [
            GitHubStackingMode::Auto,
            GitHubStackingMode::Disabled,
            GitHubStackingMode::Native,
        ] {
            let tmp = gix_testtools::tempfile::TempDir::new()?;
            gix::init(tmp.path())?;
            let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
            repo.set_git_settings(&GitConfigSettings {
                gitbutler_github_stacking_mode: Some(value),
                ..Default::default()
            })?;

            let repo = but_testsupport::open_repo(repo.path())?;
            assert_eq!(
                repo.git_settings()?.gitbutler_github_stacking_mode,
                Some(value),
                "the configured GitHub stacking mode should round-trip"
            );
        }
        Ok(())
    }

    #[test]
    fn review_stacking_description_defaults_when_absent_or_invalid() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        assert_eq!(
            repo.git_settings()?.gitbutler_review_stacking_description,
            None,
            "an absent key leaves policy selection to the API's bottom default"
        );

        let mut config = std::fs::OpenOptions::new()
            .append(true)
            .open(repo.path().join("config"))?;
        writeln!(
            config,
            "\n[gitbutler]\n\treviewStackingDescription = unexpected"
        )?;
        drop(config);

        let repo = but_testsupport::open_repo(repo.path())?;
        assert_eq!(
            repo.git_settings()?.gitbutler_review_stacking_description,
            Some(ReviewStackingDescription::Bottom),
            "an invalid value safely falls back to bottom"
        );
        Ok(())
    }

    #[test]
    fn setting_review_stacking_description_preserves_other_config() -> anyhow::Result<()> {
        let tmp = gix_testtools::tempfile::TempDir::new()?;
        gix::init(tmp.path())?;
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        repo.set_git_settings(&GitConfigSettings {
            signing_key: Some("existing-key".into()),
            gitbutler_gitlab_project_id: Some("existing-project".into()),
            ..Default::default()
        })?;
        repo.set_git_settings(&GitConfigSettings {
            gitbutler_review_stacking_description: Some(ReviewStackingDescription::Top),
            ..Default::default()
        })?;

        let repo = but_testsupport::open_repo(repo.path())?;
        let settings = repo.git_settings()?;
        assert_eq!(settings.signing_key, Some("existing-key".into()));
        assert_eq!(
            settings.gitbutler_gitlab_project_id.as_deref(),
            Some("existing-project")
        );
        assert_eq!(
            settings.gitbutler_review_stacking_description,
            Some(ReviewStackingDescription::Top)
        );
        Ok(())
    }
}
