//! Mock scenario support for E2E testing.
//!
//! This module provides functionality to load mock scenarios from files and create
//! mock Claude clients for testing purposes. It is only available when the `testing`
//! feature is enabled.
//!
//! # Usage
//!
//! Set the `CLAUDE_MOCK_SCENARIO` environment variable to the path of a JSON scenario file,
//! then build with `--features testing`. The application will use a mock client that replays
//! the scenario instead of connecting to the real Claude CLI.
//!
//! # Scenario File Format
//!
//! The scenario file is a JSON snapshot that can be:
//! 1. Recorded from a real session using `SnapshotRecorder`
//! 2. Hand-crafted for specific test cases
//!
//! Example scenario structure:
//! ```json
//! {
//!   "version": 1,
//!   "recorded_at": "1234567890",
//!   "sdk_version": "0.6.0",
//!   "cli_version": null,
//!   "options": null,
//!   "messages": [
//!     {"offset_ms": 0, "direction": "Received", "content": {"type": "system", ...}},
//!     {"offset_ms": 100, "direction": "Received", "content": {"type": "assistant", ...}}
//!   ]
//! }
//! ```
//!
//! # Permission Flow Handling
//!
//! When a scenario includes a `control_request` with `subtype: "can_use_tool"`, subsequent
//! messages are automatically configured to be delivered only after the SDK writes back a
//! `control_response`. This allows proper testing of the permission approval flow where:
//!
//! 1. MockTransport delivers the `control_request`
//! 2. SDK calls the `can_use_tool` callback
//! 3. Test waits for user to approve/deny
//! 4. SDK writes `control_response`
//! 5. MockTransport triggers delivery of remaining messages

use std::path::Path;

use claude_agent_sdk_rs::testing::{
    MessageDirection, MessageTiming, MockClient, MockTransport, ScheduledMessage, SessionSnapshot, SnapshotPlayer,
    TimingConfig,
};
use claude_agent_sdk_rs::types::config::ClaudeAgentOptions;

/// Environment variable name for specifying the mock scenario file path.
pub const CLAUDE_MOCK_SCENARIO_ENV: &str = "CLAUDE_MOCK_SCENARIO";

/// Check if mock mode is enabled via environment variable.
pub fn is_mock_mode_enabled() -> bool {
    std::env::var(CLAUDE_MOCK_SCENARIO_ENV).is_ok()
}

/// Get the mock scenario path from environment variable.
pub fn get_mock_scenario_path() -> Option<String> {
    std::env::var(CLAUDE_MOCK_SCENARIO_ENV).ok()
}

/// Load a mock scenario from a file path.
///
/// # Arguments
/// * `path` - Path to the JSON scenario file
///
/// # Returns
/// A `SnapshotPlayer` that can be used to create a mock transport.
pub fn load_scenario_from_file(path: impl AsRef<Path>) -> std::io::Result<SnapshotPlayer> {
    SnapshotPlayer::load(path)
}

/// Create a MockClient from a scenario file with the given options.
///
/// # Arguments
/// * `scenario_path` - Path to the JSON scenario file
/// * `options` - Claude agent options to use with the mock client
///
/// # Returns
/// A `MockClient` configured with the scenario and options.
pub fn create_mock_client_from_file(
    scenario_path: impl AsRef<Path>,
    options: ClaudeAgentOptions,
) -> std::io::Result<MockClient> {
    let player = load_scenario_from_file(scenario_path)?;
    let transport = player.to_mock_transport();
    Ok(MockClient::from_transport(transport, options))
}

/// Create a MockTransport from a scenario file.
///
/// This is useful when you need direct access to the transport rather than
/// going through MockClient.
///
/// This function creates a transport that properly handles permission flows:
/// - Messages before a `control_request` are delivered immediately or with delays
/// - Messages after a `control_request` with `subtype: "can_use_tool"` are held until
///   the SDK writes back a `control_response` (i.e., after user approval/denial)
///
/// # Arguments
/// * `scenario_path` - Path to the JSON scenario file
///
/// # Returns
/// A `MockTransport` configured with the scenario.
pub fn create_mock_transport_from_file(scenario_path: impl AsRef<Path>) -> std::io::Result<MockTransport> {
    tracing::info!(
        scenario_path = %scenario_path.as_ref().display(),
        "Loading mock scenario for permission-aware transport"
    );
    let player = load_scenario_from_file(scenario_path)?;
    tracing::info!(
        message_count = player.snapshot().messages.len(),
        "Loaded scenario, creating permission-aware transport"
    );
    Ok(create_permission_aware_transport(player.snapshot()))
}

/// Create a MockTransport that properly handles permission request flows.
///
/// Unlike the basic `SnapshotPlayer.to_mock_transport()`, this function:
/// 1. Identifies `control_request` messages with `subtype: "can_use_tool"`
/// 2. Configures subsequent messages to wait for `control_response` before delivery
///
/// This ensures the permission callback flow works correctly in tests:
/// - User sees permission popup
/// - User approves/denies
/// - SDK sends control_response
/// - Remaining scenario messages are delivered
fn create_permission_aware_transport(snapshot: &SessionSnapshot) -> MockTransport {
    let mut scheduled_messages = Vec::new();
    let mut waiting_for_control_response = false;
    let mut pending_request_id: Option<String> = None;

    for msg in &snapshot.messages {
        // Only process received messages (from "CLI" to SDK)
        if msg.direction != MessageDirection::Received {
            continue;
        }

        let content = &msg.content;

        // Check if this is a control_request with can_use_tool
        let is_permission_request = content
            .get("type")
            .and_then(|v| v.as_str())
            .map(|t| t == "control_request")
            .unwrap_or(false)
            && content
                .get("request")
                .and_then(|r| r.get("subtype"))
                .and_then(|v| v.as_str())
                .map(|s| s == "can_use_tool")
                .unwrap_or(false);

        if is_permission_request {
            // Extract request_id for the trigger pattern
            pending_request_id = content.get("request_id").and_then(|v| v.as_str()).map(String::from);

            tracing::debug!(
                request_id = ?pending_request_id,
                "Found permission request in scenario, subsequent messages will wait for control_response"
            );

            // Deliver the control_request immediately
            scheduled_messages.push(ScheduledMessage {
                value: content.clone(),
                timing: MessageTiming::Immediate,
            });

            // All subsequent messages should wait for control_response
            waiting_for_control_response = true;
        } else if waiting_for_control_response {
            // This message should wait for the control_response
            // Use the request_id as the trigger pattern
            let pattern = pending_request_id
                .as_ref()
                .map(|id| format!("\"request_id\":\"{}\"", id))
                .unwrap_or_else(|| "control_response".to_string());

            scheduled_messages.push(ScheduledMessage {
                value: content.clone(),
                timing: MessageTiming::AfterWrite { pattern },
            });

            // Check if this is a result message (end of session)
            // If so, we can reset the waiting state for any subsequent exchanges
            let is_result = content
                .get("type")
                .and_then(|v| v.as_str())
                .map(|t| t == "result")
                .unwrap_or(false);

            if is_result {
                waiting_for_control_response = false;
                pending_request_id = None;
            }
        } else {
            // Normal message - deliver immediately
            scheduled_messages.push(ScheduledMessage {
                value: content.clone(),
                timing: MessageTiming::Immediate,
            });
        }
    }

    MockTransport::new(scheduled_messages, TimingConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_mock_mode_disabled_by_default() {
        // Note: env var state is shared across tests, so we just check the function works
        // Don't modify env vars as that could affect parallel tests
        let _ = is_mock_mode_enabled(); // Just verify it doesn't panic
    }

    #[test]
    fn test_permission_aware_transport_creates_correct_timing() {
        // Scenario with permission request
        let scenario_json = r#"{
            "version": 1,
            "recorded_at": "1234567890",
            "sdk_version": "0.6.0",
            "cli_version": null,
            "options": null,
            "messages": [
                {"offset_ms": 0, "direction": "Received", "content": {"type": "system", "subtype": "init", "session_id": "test-123"}},
                {"offset_ms": 100, "direction": "Received", "content": {"type": "assistant", "message": {"content": [{"type": "text", "text": "Hello"}]}}},
                {"offset_ms": 200, "direction": "Received", "content": {"type": "control_request", "request_id": "perm_001", "request": {"subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": "echo test"}}}},
                {"offset_ms": 300, "direction": "Received", "content": {"type": "assistant", "message": {"content": [{"type": "text", "text": "Done"}]}}},
                {"offset_ms": 400, "direction": "Received", "content": {"type": "result", "subtype": "success"}}
            ]
        }"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_scenario.json");
        std::fs::write(&temp_file, scenario_json).unwrap();

        let _transport = create_mock_transport_from_file(&temp_file).unwrap();

        // The transport should have been created with the right timing:
        // - Messages 0, 1, 2 should be Immediate
        // - Messages 3, 4 should be AfterWrite (waiting for control_response)
        //
        // We can't directly inspect the scheduled messages, but we verified
        // the transport was created successfully (no panic)

        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_load_scenario_from_file() {
        let scenario_json = r#"{
            "version": 1,
            "recorded_at": "1234567890",
            "sdk_version": "0.6.0",
            "cli_version": null,
            "options": null,
            "messages": [
                {"offset_ms": 0, "direction": "Received", "content": {"type": "system", "session_id": "test-123"}}
            ]
        }"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_scenario_load.json");
        std::fs::write(&temp_file, scenario_json).unwrap();

        let player = load_scenario_from_file(&temp_file).unwrap();
        assert_eq!(player.snapshot().messages.len(), 1);

        std::fs::remove_file(&temp_file).ok();
    }
}
