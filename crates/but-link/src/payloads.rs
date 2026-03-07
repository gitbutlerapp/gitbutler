//! Typed payload helpers for message-backed coordination records.

use anyhow::Context;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Decode a JSON message body into a typed payload.
pub(crate) fn decode_json<T: DeserializeOwned>(body_json: &str) -> anyhow::Result<T> {
    serde_json::from_str(body_json).context("decode_json")
}

/// Discovery evidence entry.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DiscoveryEvidence {
    /// Human-readable evidence detail.
    #[serde(default)]
    pub detail: String,
}

/// Discovery suggested action payload.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DiscoverySuggestedAction {
    /// Suggested command to run next.
    #[serde(default)]
    pub cmd: String,
}

/// Structured discovery payload stored in transcript messages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DiscoveryPayload {
    /// Short discovery title.
    #[serde(default)]
    pub title: String,
    /// Supporting evidence entries.
    #[serde(default)]
    pub evidence: Vec<DiscoveryEvidence>,
    /// Suggested next action.
    #[serde(default)]
    pub suggested_action: DiscoverySuggestedAction,
    /// Optional signal level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

impl DiscoveryPayload {
    /// Decode a discovery payload from stored JSON.
    pub(crate) fn from_json_str(body_json: &str) -> anyhow::Result<Self> {
        decode_json(body_json)
    }

    /// Decode a discovery payload from a JSON value.
    pub(crate) fn from_value(value: Value) -> anyhow::Result<Self> {
        serde_json::from_value(value).context("discovery must be an object")
    }

    /// Validate the required discovery fields.
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.title.trim().is_empty(), "title required");
        anyhow::ensure!(!self.evidence.is_empty(), "evidence required");
        anyhow::ensure!(
            !self.suggested_action.cmd.trim().is_empty(),
            "suggested_action.cmd required"
        );
        Ok(())
    }

    /// Return whether the discovery is high-signal.
    pub(crate) fn is_high_signal(&self) -> bool {
        self.signal
            .as_deref()
            .is_some_and(|signal| signal.eq_ignore_ascii_case("high"))
    }

    /// Return the suggested command when present.
    pub(crate) fn command(&self) -> Option<&str> {
        (!self.suggested_action.cmd.trim().is_empty()).then_some(self.suggested_action.cmd.as_str())
    }

    /// Convert the payload into a JSON value and attach read-side metadata.
    pub(crate) fn to_value_with_metadata(
        &self,
        agent_id: &str,
        created_at_ms: Option<i64>,
        include_kind: bool,
    ) -> anyhow::Result<Value> {
        let mut value = serde_json::to_value(self)?;
        if let Value::Object(map) = &mut value {
            map.insert("agent_id".to_owned(), Value::String(agent_id.to_owned()));
            if let Some(created_at_ms) = created_at_ms {
                map.insert("created_at_ms".to_owned(), Value::from(created_at_ms));
            }
            if include_kind {
                map.insert("kind".to_owned(), Value::String("discovery".to_owned()));
            }
        }
        Ok(value)
    }
}

/// Typed intent/declaration payload stored in transcript messages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SurfacePayload {
    /// Shared module or system scope.
    #[serde(default)]
    pub scope: String,
    /// Tags that classify the declaration.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Surface tokens exposed by the declaration.
    #[serde(default)]
    pub surface: Vec<String>,
    /// Optional path scope.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl SurfacePayload {
    /// Decode a surface payload from stored JSON.
    pub(crate) fn from_json_str(body_json: &str) -> anyhow::Result<Self> {
        decode_json(body_json)
    }

    /// Decode a surface payload from a JSON value.
    pub(crate) fn from_value(value: Value) -> anyhow::Result<Self> {
        serde_json::from_value(value).context("payload must be an object")
    }

    /// Validate the required surface fields.
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.scope.trim().is_empty(), "scope required");
        anyhow::ensure!(
            !self.tags.is_empty(),
            "tags required (non-empty string array)"
        );
        anyhow::ensure!(
            !self.surface.is_empty(),
            "surface required (non-empty string array)"
        );
        Ok(())
    }
}

/// History payload for claim and release messages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ClaimHistory {
    /// Human-readable summary text.
    pub text: String,
    /// Claimed or released paths.
    pub paths: Vec<String>,
}

/// History payload for trusted acquisition messages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AcquireHistory {
    /// Human-readable summary text.
    pub text: String,
    /// Requested paths.
    pub paths: Vec<String>,
    /// Successfully acquired paths.
    #[serde(default)]
    pub acquired_paths: Vec<String>,
}

impl AcquireHistory {
    /// Decode an acquire history payload from stored JSON.
    pub(crate) fn from_json_str(body_json: &str) -> anyhow::Result<Self> {
        decode_json(body_json)
    }
}

/// History payload for typed block messages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BlockHistory {
    /// Human-readable summary text.
    pub text: String,
    /// Resolved block id.
    pub block_id: i64,
    /// Block mode string.
    pub mode: String,
    /// Human-readable reason.
    pub reason: String,
    /// Covered repo-relative paths.
    pub paths: Vec<String>,
}

impl BlockHistory {
    /// Decode a block history payload from stored JSON.
    pub(crate) fn from_json_str(body_json: &str) -> anyhow::Result<Self> {
        decode_json(body_json)
    }
}

/// History payload for typed acknowledgement messages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AckHistory {
    /// Human-readable summary text.
    pub text: String,
    /// Acknowledgement id.
    pub ack_id: i64,
    /// Directed target agent.
    pub target_agent_id: String,
    /// Covered repo-relative paths.
    pub paths: Vec<String>,
    /// Optional note body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl AckHistory {
    /// Decode an acknowledgement history payload from stored JSON.
    pub(crate) fn from_json_str(body_json: &str) -> anyhow::Result<Self> {
        decode_json(body_json)
    }
}

/// History payload for typed resolve messages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ResolveHistory {
    /// Human-readable summary text.
    pub text: String,
    /// Resolved block id.
    pub block_id: i64,
    /// Targeted block owner.
    pub target_agent_id: String,
    /// Agent that resolved the block.
    pub resolved_by_agent_id: String,
}

impl ResolveHistory {
    /// Decode a resolve history payload from stored JSON.
    pub(crate) fn from_json_str(body_json: &str) -> anyhow::Result<Self> {
        decode_json(body_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let decoded = DiscoveryPayload::from_json_str(&json)?;

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
        let decoded = SurfacePayload::from_json_str(&json)?;

        assert_eq!(decoded, payload);
        Ok(())
    }
}
