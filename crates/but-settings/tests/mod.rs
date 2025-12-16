use but_settings::AppSettings;

#[test]
#[expect(clippy::bool_assert_comparison)]
fn test_load_settings() {
    let settings =
        AppSettings::load("tests/fixtures/modify_default_true_to_false.json".as_ref()).unwrap();
    assert_eq!(settings.telemetry.app_metrics_enabled, false); // modified
    assert_eq!(settings.telemetry.app_error_reporting_enabled, true); // default
    assert_eq!(settings.telemetry.app_non_anon_metrics_enabled, false); // default
    assert_eq!(settings.telemetry.app_distinct_id, None); // default
    assert_eq!(settings.onboarding_complete, false); // default
    assert_eq!(
        settings.github_oauth_app.oauth_client_id,
        "cd51880daa675d9e6452"
    ); // default
}

#[test]
fn test_load_cli_managed_by_package_manager() {
    let settings =
        AppSettings::load("tests/fixtures/cli_managed_by_package_manager.json".as_ref()).unwrap();
    assert!(settings.cli.managed_by_package_manager); // modified
    // Verify other defaults remain unchanged
    assert!(!settings.onboarding_complete); // default
    assert!(settings.telemetry.app_metrics_enabled); // default
}
