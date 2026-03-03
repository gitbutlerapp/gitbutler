#[allow(dead_code)]
#[path = "../src/payloads.rs"]
mod payloads_impl;
#[allow(dead_code)]
#[path = "../src/text.rs"]
mod text;

use payloads_impl::{
    DiscoveryEvidence, DiscoveryPayload, DiscoverySuggestedAction, SurfacePayload,
};

#[test]
fn discovery_payload_round_trip() -> anyhow::Result<()> {
    let payload = DiscoveryPayload {
        title: "Missing retry".to_owned(),
        evidence: vec![DiscoveryEvidence {
            detail: "rpc errors".to_owned(),
        }],
        suggested_action: DiscoverySuggestedAction {
            cmd: "cargo test".to_owned(),
        },
        signal: Some("high".to_owned()),
    };

    let json = serde_json::to_string(&payload)?;
    let decoded: DiscoveryPayload = serde_json::from_str(&json)?;

    decoded.validate()?;
    assert_eq!(decoded, payload);
    Ok(())
}

#[test]
fn surface_payload_round_trip() -> anyhow::Result<()> {
    let payload = SurfacePayload {
        scope: "crate::auth".to_owned(),
        tags: vec!["api".to_owned()],
        surface: vec!["AuthToken".to_owned()],
        paths: vec!["src/auth.rs".to_owned()],
    };

    let json = serde_json::to_string(&payload)?;
    let decoded: SurfacePayload = serde_json::from_str(&json)?;

    decoded.validate()?;
    assert_eq!(decoded, payload);
    Ok(())
}
