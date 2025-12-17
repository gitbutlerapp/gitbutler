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
        assert_eq!(
            settings.telemetry.app_non_anon_metrics_enabled, false,
            "default"
        );
        assert_eq!(settings.telemetry.app_distinct_id, None, "default");
        assert_eq!(settings.onboarding_complete, false, "default");
        assert_eq!(
            settings.github_oauth_app.oauth_client_id, "cd51880daa675d9e6452",
            "default"
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
        assert_eq!(
            settings.telemetry.app_non_anon_metrics_enabled, false,
            "default"
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

        #[test]
        fn packed_but_binary() {
            let settings = AppSettings::load(
                "tests/fixtures/modify_default_true_to_false.json".as_ref(),
                Some(but_settings::customization::packed_but_binary()),
            )
            .unwrap();
            assert_eq!(
                settings.ui.cli_is_managed_by_package_manager, true,
                "overridden to tell the GUI that it shouldn't provide the usual installation options"
            );
        }
    }
}
