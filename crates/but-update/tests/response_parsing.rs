use but_update::CheckUpdateStatus;
use serde_json::json;

#[test]
fn deserializes_update_available_response() {
    let json_response = json!({
        "up_to_date": false,
        "latest_version": "0.19.0",
        "release_notes": "## What's New\n- Feature A\n- Feature B",
        "url": "https://releases.gitbutler.com/app/0.19.0/GitButler.dmg",
        "signature": "abc123def456"
    });

    let status: CheckUpdateStatus =
        serde_json::from_value(json_response).expect("Failed to deserialize");

    assert!(!status.up_to_date);
    assert_eq!(status.latest_version, "0.19.0");
    assert_eq!(
        status.release_notes.as_deref(),
        Some("## What's New\n- Feature A\n- Feature B")
    );
    assert_eq!(
        status.url.as_deref(),
        Some("https://releases.gitbutler.com/app/0.19.0/GitButler.dmg")
    );
    assert_eq!(status.signature.as_deref(), Some("abc123def456"));
}

#[test]
fn deserializes_up_to_date_response() {
    let json_response = json!({
        "up_to_date": true,
        "latest_version": "0.18.3"
    });

    let status: CheckUpdateStatus =
        serde_json::from_value(json_response).expect("Failed to deserialize");

    assert!(status.up_to_date);
    assert_eq!(status.latest_version, "0.18.3");
    assert!(status.release_notes.is_none());
    assert!(status.url.is_none());
    assert!(status.signature.is_none());
}

#[test]
fn rejects_missing_required_fields() {
    // Missing up_to_date field
    let json_missing_up_to_date = json!({
        "latest_version": "0.19.0"
    });

    let result = serde_json::from_value::<CheckUpdateStatus>(json_missing_up_to_date);
    assert!(result.is_err(), "Should fail when up_to_date is missing");

    // Missing latest_version field
    let json_missing_version = json!({
        "up_to_date": true
    });

    let result = serde_json::from_value::<CheckUpdateStatus>(json_missing_version);
    assert!(
        result.is_err(),
        "Should fail when latest_version is missing"
    );
}

#[test]
fn rejects_wrong_types() {
    // up_to_date should be boolean, not string
    let json_wrong_type = json!({
        "up_to_date": "true",
        "latest_version": "0.19.0"
    });

    let result = serde_json::from_value::<CheckUpdateStatus>(json_wrong_type);
    assert!(
        result.is_err(),
        "Should fail when up_to_date is not a boolean"
    );

    // latest_version should be string, not number
    let json_version_number = json!({
        "up_to_date": true,
        "latest_version": 19
    });

    let result = serde_json::from_value::<CheckUpdateStatus>(json_version_number);
    assert!(
        result.is_err(),
        "Should fail when latest_version is not a string"
    );
}

#[test]
fn ignores_extra_fields_for_forward_compatibility() {
    // Server might add new fields in the future
    let json_with_extras = json!({
        "up_to_date": false,
        "latest_version": "0.19.0",
        "release_notes": "New stuff",
        "url": "https://example.com",
        "signature": "sig123",
        "download_size": 50000000,
        "minimum_version": "0.17.0",
        "deprecated": false
    });

    let result = serde_json::from_value::<CheckUpdateStatus>(json_with_extras);
    assert!(result.is_ok(), "Should ignore unknown fields");

    let status = result.unwrap();
    assert!(!status.up_to_date);
    assert_eq!(status.latest_version, "0.19.0");
}

#[test]
fn handles_explicit_nulls_for_optional_fields() {
    let json_with_nulls = json!({
        "up_to_date": true,
        "latest_version": "0.18.3",
        "release_notes": null,
        "url": null,
        "signature": null
    });

    let result = serde_json::from_value::<CheckUpdateStatus>(json_with_nulls);
    assert!(result.is_ok(), "Should handle explicit nulls");

    let status = result.unwrap();
    assert!(status.up_to_date);
    assert!(status.release_notes.is_none());
    assert!(status.url.is_none());
    assert!(status.signature.is_none());
}
