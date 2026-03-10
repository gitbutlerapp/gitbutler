//! Typed payload helpers used at service boundaries.

use serde::{Deserialize, Serialize};

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

/// Structured discovery payload accepted by the service layer.
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
}

/// Typed intent/declaration payload accepted by the service layer.
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
