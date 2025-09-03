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
