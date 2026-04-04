mod local {
    use but_core::git_config::{edit_repo_config, remove_config_value, set_config_value};
    use but_testsupport::{invoke_bash, writable_scenario};

    #[test]
    fn writes_back_local_config_when_requested() -> anyhow::Result<()> {
        let (mut repo, _tmp) = writable_scenario("git-config-empty");

        assert!(edit_repo_config(
            &repo,
            gix::config::Source::Local,
            |config| {
                set_config_value(config, "gitbutler.testValue", "set")?;
                Ok(())
            }
        )?);

        repo.reload()?;
        assert_eq!(
            repo.config_snapshot()
                .string("gitbutler.testValue")
                .map(|value| value.to_string()),
            Some("set".to_owned())
        );
        Ok(())
    }

    #[test]
    fn writes_back_local_config_when_value_is_removed() -> anyhow::Result<()> {
        let (mut repo, _tmp) = writable_scenario("git-config-empty");
        invoke_bash("git config --local gitbutler.testValue kept", &repo);

        assert!(edit_repo_config(
            &repo,
            gix::config::Source::Local,
            |config| {
                remove_config_value(config, "gitbutler.testValue")?;
                Ok(())
            }
        )?);

        repo.reload()?;
        assert_eq!(
            repo.config_snapshot()
                .string("gitbutler.testValue")
                .map(|value| value.to_string()),
            None
        );
        Ok(())
    }
}
