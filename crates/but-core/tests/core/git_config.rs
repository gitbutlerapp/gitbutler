mod local {
    use but_core::git_config::{edit_repo_config, remove_config_value, set_config_value};
    use but_testsupport::{invoke_bash, writable_scenario};

    #[test]
    fn writes_back_local_config_when_requested() -> anyhow::Result<()> {
        let (mut repo, _tmp) = writable_scenario("git-config-empty");

        assert!(
            !edit_repo_config(&repo, gix::config::Source::Local, |_| Ok(()))?,
            "an unchanged edit is reported as a no-op"
        );
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

#[test]
fn replacing_and_removing_config_values_covers_repeated_sections() -> anyhow::Result<()> {
    use but_core::git_config::{remove_config_value, set_config_value};

    let initial = r#"
[remote "origin"]
    url = first
    fetch = +refs/heads/*:refs/remotes/origin/*
[remote "other"]
    url = untouched
[remote "origin"]
    url
    url = ""
    url = last
"#;
    let mut config = gix::config::File::try_from(initial)?;
    set_config_value(&mut config, "remote.origin.url", "replacement")?;
    assert_eq!(
        config.raw_values("remote.origin.url")?,
        [gix::bstr::BString::from("replacement")],
        "setting a scalar replaces all earlier explicit and implicit values"
    );
    assert_eq!(
        config.raw_value("remote.other.url")?,
        "untouched",
        "a different subsection is left unchanged"
    );
    assert_eq!(
        config.raw_value("remote.origin.fetch")?,
        "+refs/heads/*:refs/remotes/origin/*",
        "neighboring values are retained"
    );

    let mut config = gix::config::File::try_from(initial)?;
    remove_config_value(&mut config, "remote.origin.url")?;
    assert!(
        config.raw_values("remote.origin.url").is_err(),
        "removing a scalar cannot uncover a previous value"
    );
    remove_config_value(&mut config, "remote.origin.url")?;
    set_config_value(&mut config, "remote.origin.url", "restored")?;
    assert_eq!(
        config.raw_values("remote.origin.url")?,
        [gix::bstr::BString::from("restored")],
        "missing values can still be removed or recreated"
    );
    Ok(())
}
