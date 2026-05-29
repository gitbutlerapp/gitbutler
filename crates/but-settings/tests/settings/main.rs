#[expect(clippy::bool_assert_comparison)]
mod load {
    use but_settings::AppSettings;
    use serde_json::json;

    #[test]
    fn without_customizations() {
        let settings = AppSettings::load(
            "tests/fixtures/modify_default_true_to_false.json".as_ref(),
            None,
        )
        .unwrap();
        assert_eq!(settings.telemetry.app_metrics_enabled, false, "modified");
        assert_eq!(
            settings.telemetry.app_error_reporting_enabled, true,
            "default"
        );
        assert_eq!(settings.telemetry.app_distinct_id, None, "default");
        assert_eq!(settings.onboarding_complete, false, "default");
        assert_eq!(
            settings.github_oauth_app.oauth_client_id, "cd51880daa675d9e6452",
            "default"
        );
        assert_eq!(settings.feature_flags.unapply_v3, true, "default");
    }

    #[test]
    fn unapply_v3_defaults_on_but_can_be_disabled() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config_path = temp_dir.path().join("settings.json");

        std::fs::write(&config_path, "{}").unwrap();
        let settings = AppSettings::load(&config_path, None).unwrap();
        assert!(
            settings.feature_flags.unapply_v3,
            "Unapply v3 should be enabled when the user has no explicit setting"
        );

        std::fs::write(
            &config_path,
            r#"{
                "featureFlags": {
                    "unapplyV3": false
                }
            }"#,
        )
        .unwrap();
        let settings = AppSettings::load(&config_path, None).unwrap();
        assert!(
            !settings.feature_flags.unapply_v3,
            "An explicit user setting should still disable Unapply v3"
        );
    }

    #[test]
    fn with_customizations() {
        let settings = AppSettings::load(
            "tests/fixtures/modify_default_true_to_false.json".as_ref(),
            Some(json!({
                "telemetry": {
                    "appMetricsEnabled": true,
                    "appErrorReportingEnabled": false,
                },
                "githubOauthApp": {
                    "oauthClientId": "other"
                }
            })),
        )
        .unwrap();
        assert_eq!(
            settings.telemetry.app_metrics_enabled, true,
            "custom override"
        );
        assert_eq!(
            settings.telemetry.app_error_reporting_enabled, false,
            "custom override"
        );
        assert_eq!(settings.telemetry.app_distinct_id, None, "default");
        assert_eq!(settings.onboarding_complete, false, "default");
        assert_eq!(
            settings.github_oauth_app.oauth_client_id, "other",
            "custom override"
        );
    }

    mod customization {
        use but_settings::AppSettings;
        use serde_json::json;

        #[test]
        fn packaged_but_binary() {
            let settings = AppSettings::load(
                "tests/fixtures/modify_default_true_to_false.json".as_ref(),
                Some(but_settings::customization::packaged_but_binary()),
            )
            .unwrap();
            assert_eq!(
                settings.ui.cli_is_managed_by_package_manager, true,
                "overridden to tell the GUI that it shouldn't provide the usual installation options"
            );
        }

        #[test]
        fn disable_auto_update_checks() {
            let settings = AppSettings::load(
                "tests/fixtures/modify_default_true_to_false.json".as_ref(),
                Some(but_settings::customization::disable_auto_update_checks()),
            )
            .unwrap();

            #[expect(deprecated)]
            let value = settings.ui.check_for_updates_interval_in_seconds;
            assert_eq!(
                value, 0,
                "overridden to tell the GUI that no updates should be performed"
            );
        }

        #[test]
        fn merge() {
            let first = json!({
                "a": 1
            });
            let actual = but_settings::customization::merge_two(first.clone(), None);
            assert_eq!(actual, first, "second side with `None` has no effect");
            let actual = but_settings::customization::merge_two(first, Some(json!({"b": 2})));
            assert_eq!(
                actual,
                json!({"a": 1, "b": 2}),
                "if second is Some, it's merged"
            );
        }
    }
}
