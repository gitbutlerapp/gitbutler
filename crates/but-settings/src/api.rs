use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::AppSettingsWithDiskSync;
use crate::app_settings::IrcConnectionSettings;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::TelemetrySettings`].
pub struct TelemetryUpdate {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
    pub app_non_anon_metrics_enabled: Option<bool>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::FeatureFlags`].
pub struct FeatureFlagsUpdate {
    pub cv3: Option<bool>,
    pub apply3: Option<bool>,
    pub rules: Option<bool>,
    pub single_branch: Option<bool>,
    pub irc: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::Claude`].
pub struct ClaudeUpdate {
    pub executable: Option<String>,
    pub notify_on_completion: Option<bool>,
    pub notify_on_permission_request: Option<bool>,
    pub dangerously_allow_all_permissions: Option<bool>,
    pub auto_commit_after_completion: Option<bool>,
    pub use_configured_model: Option<bool>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::Reviews`].
pub struct ReviewsUpdate {
    pub auto_fill_pr_description_from_commit: Option<bool>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::Fetch`].
pub struct FetchUpdate {
    pub auto_fetch_interval_minutes: Option<isize>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::UiSettings`].
pub struct UiUpdate {
    pub use_native_title_bar: Option<bool>,
    // Note that the CLI related information cannot be set - it's set at compile time.
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::IrcSettings`].
pub struct IrcUpdate {
    pub server: Option<IrcServerUpdate>,
    pub auto_share: Option<bool>,
    pub project_channel: Option<Option<String>>,
    pub connection: Option<IrcConnectionUpdate>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::IrcServerSettings`].
pub struct IrcServerUpdate {
    pub host: Option<String>,
    pub port: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::IrcConnectionSettings`].
/// Used for connection updates.
pub struct IrcConnectionUpdate {
    pub enabled: Option<bool>,
    pub nickname: Option<Option<String>>,
    pub server_password: Option<Option<String>>,
    pub sasl_password: Option<Option<String>>,
    pub realname: Option<Option<String>>,
}

/// Mutation, immediately followed by writing everything to disk.
impl AppSettingsWithDiskSync {
    pub fn update_onboarding_complete(&self, update: bool) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        settings.onboarding_complete = update;
        settings.save()
    }

    pub fn update_telemetry(&self, update: TelemetryUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(app_metrics_enabled) = update.app_metrics_enabled {
            settings.telemetry.app_metrics_enabled = app_metrics_enabled;
        }
        if let Some(app_error_reporting_enabled) = update.app_error_reporting_enabled {
            settings.telemetry.app_error_reporting_enabled = app_error_reporting_enabled;
        }
        if let Some(app_non_anon_metrics_enabled) = update.app_non_anon_metrics_enabled {
            settings.telemetry.app_non_anon_metrics_enabled = app_non_anon_metrics_enabled;
        }
        settings.save()
    }

    pub fn update_telemetry_distinct_id(&self, app_distinct_id: Option<String>) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        settings.telemetry.app_distinct_id = app_distinct_id;
        settings.save()
    }

    pub fn update_feature_flags(
        &self,
        FeatureFlagsUpdate {
            cv3,
            apply3,
            rules,
            single_branch,
            irc,
        }: FeatureFlagsUpdate,
    ) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(cv3) = cv3 {
            settings.feature_flags.cv3 = cv3;
        }
        if let Some(apply3) = apply3 {
            settings.feature_flags.apply3 = apply3;
        }
        if let Some(rules) = rules {
            settings.feature_flags.rules = rules;
        }
        if let Some(single_branch) = single_branch {
            settings.feature_flags.single_branch = single_branch;
        }
        if let Some(irc) = irc {
            settings.feature_flags.irc = irc;
        }
        settings.save()
    }

    pub fn update_claude(&self, update: ClaudeUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(executable) = update.executable {
            settings.claude.executable = executable;
        }
        if let Some(notify_on_completion) = update.notify_on_completion {
            settings.claude.notify_on_completion = notify_on_completion;
        }
        if let Some(notify_on_permission_request) = update.notify_on_permission_request {
            settings.claude.notify_on_permission_request = notify_on_permission_request;
        }
        if let Some(dangerously_allow_all_permissions) = update.dangerously_allow_all_permissions {
            settings.claude.dangerously_allow_all_permissions = dangerously_allow_all_permissions;
        }
        if let Some(auto_commit_after_completion) = update.auto_commit_after_completion {
            settings.claude.auto_commit_after_completion = auto_commit_after_completion;
        }
        if let Some(use_configured_model) = update.use_configured_model {
            settings.claude.use_configured_model = use_configured_model;
        }
        settings.save()
    }

    pub fn update_reviews(&self, update: ReviewsUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(auto_fill_pr_description_from_commit) =
            update.auto_fill_pr_description_from_commit
        {
            settings.reviews.auto_fill_pr_description_from_commit =
                auto_fill_pr_description_from_commit;
        }
        settings.save()
    }

    pub fn update_fetch(&self, update: FetchUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(auto_fetch_interval_minutes) = update.auto_fetch_interval_minutes {
            settings.fetch.auto_fetch_interval_minutes = auto_fetch_interval_minutes;
        }
        settings.save()
    }

    pub fn update_ui(&self, update: UiUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(use_native_title_bar) = update.use_native_title_bar {
            settings.ui.use_native_title_bar = use_native_title_bar;
        }
        settings.save()
    }

    pub fn update_irc(&self, update: IrcUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(server_update) = update.server {
            if let Some(host) = server_update.host {
                settings.irc.server.host = host;
            }
            if let Some(port) = server_update.port {
                settings.irc.server.port = port;
            }
        }
        if let Some(auto_share) = update.auto_share {
            settings.irc.auto_share = auto_share;
        }
        if let Some(project_channel) = update.project_channel {
            settings.irc.project_channel = project_channel;
        }
        if let Some(connection_update) = update.connection {
            apply_connection_update(&mut settings.irc.connection, connection_update);
        }
        settings.save()
    }
}

pub(crate) fn apply_connection_update(
    conn: &mut IrcConnectionSettings,
    update: IrcConnectionUpdate,
) {
    if let Some(enabled) = update.enabled {
        conn.enabled = enabled;
    }
    if let Some(nickname) = update.nickname {
        conn.nickname = nickname;
    }
    if let Some(server_password) = update.server_password {
        conn.server_password = server_password;
    }
    if let Some(sasl_password) = update.sasl_password {
        conn.sasl_password = sasl_password;
    }
    if let Some(realname) = update.realname {
        conn.realname = realname;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_settings::IrcConnectionSettings;

    fn default_connection() -> IrcConnectionSettings {
        IrcConnectionSettings {
            enabled: false,
            nickname: None,
            server_password: None,
            sasl_password: None,
            realname: None,
        }
    }

    // =========================================================================
    // apply_connection_update — partial updates
    // =========================================================================

    #[test]
    fn apply_connection_update_nickname_only() {
        let mut conn = default_connection();
        apply_connection_update(
            &mut conn,
            IrcConnectionUpdate {
                enabled: None,
                nickname: Some(Some("myuser".to_string())),
                server_password: None,
                sasl_password: None,
                realname: None,
            },
        );
        assert_eq!(conn.nickname, Some("myuser".to_string()));
        assert_eq!(conn.sasl_password, None);
    }

    #[test]
    fn apply_connection_update_multiple_fields() {
        let mut conn = default_connection();
        apply_connection_update(
            &mut conn,
            IrcConnectionUpdate {
                enabled: None,
                nickname: Some(Some("nick".to_string())),
                server_password: None,
                sasl_password: None,
                realname: None,
            },
        );
        assert_eq!(conn.nickname, Some("nick".to_string()));
        assert_eq!(conn.sasl_password, None);
    }

    #[test]
    fn apply_connection_update_clear_nickname() {
        let mut conn = default_connection();
        conn.nickname = Some("oldnick".to_string());

        // Some(None) means "clear this field"
        apply_connection_update(
            &mut conn,
            IrcConnectionUpdate {
                enabled: None,
                nickname: Some(None),
                server_password: None,
                sasl_password: None,
                realname: None,
            },
        );
        assert_eq!(conn.nickname, None);
    }

    #[test]
    fn apply_connection_update_clear_sasl_password() {
        let mut conn = default_connection();
        conn.sasl_password = Some("secret".to_string());

        apply_connection_update(
            &mut conn,
            IrcConnectionUpdate {
                enabled: None,
                nickname: None,
                server_password: None,
                sasl_password: Some(None),
                realname: None,
            },
        );
        assert_eq!(conn.sasl_password, None);
    }

    #[test]
    fn apply_connection_update_no_op_when_all_none() {
        let mut conn = IrcConnectionSettings {
            enabled: true,
            nickname: Some("nick".to_string()),
            server_password: Some("gate".to_string()),
            sasl_password: Some("pass".to_string()),
            realname: Some("Real Name".to_string()),
        };
        let original = conn.clone();

        apply_connection_update(
            &mut conn,
            IrcConnectionUpdate {
                enabled: None,
                nickname: None,
                server_password: None,
                sasl_password: None,
                realname: None,
            },
        );
        assert_eq!(conn, original);
    }

    // =========================================================================
    // update_irc — integration tests with AppSettingsWithDiskSync
    // =========================================================================

    fn create_test_settings() -> (tempfile::TempDir, AppSettingsWithDiskSync) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let settings =
            AppSettingsWithDiskSync::new_with_customization(temp_dir.path(), None).unwrap();
        (temp_dir, settings)
    }

    #[test]
    fn update_irc_server_host_only() {
        let (_dir, settings) = create_test_settings();
        let original_port = settings.get().unwrap().irc.server.port;

        settings
            .update_irc(IrcUpdate {
                server: Some(IrcServerUpdate {
                    host: Some("new.irc.server".to_string()),
                    port: None,
                }),
                project_channel: None,

                auto_share: None,
                connection: None,
            })
            .unwrap();

        let s = settings.get().unwrap();
        assert_eq!(s.irc.server.host, "new.irc.server");
        assert_eq!(s.irc.server.port, original_port);
    }

    #[test]
    fn update_irc_connection() {
        let (_dir, settings) = create_test_settings();

        settings
            .update_irc(IrcUpdate {
                auto_share: None,

                project_channel: None,
                server: None,
                connection: Some(IrcConnectionUpdate {
                    enabled: None,
                    nickname: Some(Some("myuser".to_string())),
                    server_password: None,
                    sasl_password: None,
                    realname: None,
                }),
            })
            .unwrap();

        let s = settings.get().unwrap();
        assert_eq!(s.irc.connection.nickname, Some("myuser".to_string()));
    }

    #[test]
    fn update_irc_auto_share() {
        let (_dir, settings) = create_test_settings();

        settings
            .update_irc(IrcUpdate {
                server: None,
                project_channel: None,
                auto_share: Some(true),
                connection: None,
            })
            .unwrap();

        let s = settings.get().unwrap();
        assert!(s.irc.auto_share);
    }

    #[test]
    fn update_irc_persists_to_disk() {
        let (_dir, settings) = create_test_settings();

        settings
            .update_irc(IrcUpdate {
                server: Some(IrcServerUpdate {
                    host: Some("persisted.server".to_string()),
                    port: Some(7000),
                }),
                project_channel: None,

                auto_share: None,
                connection: Some(IrcConnectionUpdate {
                    enabled: None,
                    nickname: Some(Some("persisted_nick".to_string())),
                    server_password: None,
                    sasl_password: None,
                    realname: None,
                }),
            })
            .unwrap();

        // Load fresh from disk to verify persistence
        let reloaded = AppSettingsWithDiskSync::new_with_customization(_dir.path(), None).unwrap();
        let s = reloaded.get().unwrap();
        assert_eq!(s.irc.server.host, "persisted.server");
        assert_eq!(s.irc.server.port, 7000);
        assert_eq!(
            s.irc.connection.nickname,
            Some("persisted_nick".to_string())
        );
    }
}
