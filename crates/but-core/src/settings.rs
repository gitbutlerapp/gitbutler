/// A module to bundle configuration we *write* per repository, but read as normal.
pub mod git {
    use std::ffi::OsString;

    use anyhow::Result;
    use bstr::{BString, ByteSlice, ByteVec};

    use crate::git_config::{edit_repo_config, remove_config_value};

    const GIT_SIGN_COMMITS: &str = "commit.gpgsign";
    const GITBUTLER_SIGN_COMMITS: &str = "gitbutler.signCommits";
    const GITBUTLER_GERRIT_MODE: &str = "gitbutler.gerritMode";
    const GITBUTLER_REVIEW_STACKING_DESCRIPTION: &str = "gitbutler.reviewStackingDescription";
    const GITBUTLER_GITHUB_STACKING_MODE: &str = "gitbutler.githubStackingMode";
    const GITBUTLER_FORGE_TEMPLATE_PATH: &str = "gitbutler.forgeReviewTemplatePath";
    const GITBUTLER_GITLAB_PROJECT_ID: &str = "gitbutler.gitlabProjectId";
    const GITBUTLER_GITLAB_UPSTREAM_PROJECT_ID: &str = "gitbutler.gitlabUpstreamProjectId";
    const SIGNING_KEY: &str = "user.signingKey";
    const SIGNING_FORMAT: &str = "gpg.format";
    const GPG_PROGRAM: &str = "gpg.program";
    const GPG_SSH_PROGRAM: &str = "gpg.ssh.program";

    /// UI types
    pub mod ui {
        use but_serde::BStringForFrontend;

        /// Controls where GitButler puts stack information in review descriptions.
        #[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
        #[serde(rename_all = "lowercase")]
        pub enum ReviewStackingDescription {
            /// Put the managed stack block after user content.
            Bottom,
            /// Put the managed stack block before user content.
            Top,
            /// Remove the managed stack block during the next synchronization.
            Disabled,
        }
        #[cfg(feature = "export-schema")]
        but_schemars::register_sdk_type!(ReviewStackingDescription);

        /// Controls whether GitButler registers reviewed stacks with GitHub's native stacks API.
        #[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
        #[serde(rename_all = "lowercase")]
        pub enum GitHubStackingMode {
            /// Use native stacks when available, description metadata otherwise.
            Auto,
            /// Keep using ordinary pull requests and GitButler-managed description metadata.
            Disabled,
            /// Register same-repository GitHub pull requests as native GitHub stacks.
            Native,
        }
        #[cfg(feature = "export-schema")]
        but_schemars::register_sdk_type!(GitHubStackingMode);

        /// See [`GitConfigSettings`](crate::GitConfigSettings) for the docs.
        #[derive(Debug, PartialEq, Clone, Default, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
        #[serde(rename_all = "camelCase")]
        #[expect(missing_docs)]
        pub struct GitConfigSettings {
            #[serde(rename = "signCommits")]
            pub gitbutler_sign_commits: Option<bool>,
            pub gitbutler_gerrit_mode: Option<bool>,
            pub gitbutler_review_stacking_description: Option<ReviewStackingDescription>,
            pub gitbutler_github_stacking_mode: Option<GitHubStackingMode>,
            #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
            pub gitbutler_forge_review_template_path: Option<BStringForFrontend>,
            pub gitbutler_gitlab_project_id: Option<String>,
            pub gitbutler_gitlab_upstream_project_id: Option<String>,
            #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
            pub signing_key: Option<BStringForFrontend>,
            #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
            pub signing_format: Option<BStringForFrontend>,
            #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
            pub gpg_program: Option<BStringForFrontend>,
            #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
            pub gpg_ssh_program: Option<BStringForFrontend>,
        }
        #[cfg(feature = "export-schema")]
        but_schemars::register_sdk_type!(GitConfigSettings);

        impl From<crate::GitConfigSettings> for GitConfigSettings {
            fn from(
                crate::GitConfigSettings {
                    gitbutler_sign_commits,
                    gitbutler_gerrit_mode,
                    gitbutler_review_stacking_description,
                    gitbutler_github_stacking_mode,
                    gitbutler_forge_review_template_path,
                    gitbutler_gitlab_project_id,
                    gitbutler_gitlab_upstream_project_id,
                    signing_key,
                    signing_format,
                    gpg_program,
                    gpg_ssh_program,
                }: crate::GitConfigSettings,
            ) -> Self {
                GitConfigSettings {
                    gitbutler_sign_commits,
                    gitbutler_gerrit_mode,
                    gitbutler_review_stacking_description,
                    gitbutler_github_stacking_mode,
                    gitbutler_forge_review_template_path: gitbutler_forge_review_template_path
                        .map(Into::into),
                    gitbutler_gitlab_project_id,
                    gitbutler_gitlab_upstream_project_id,
                    signing_key: signing_key.map(Into::into),
                    signing_format: signing_format.map(Into::into),
                    gpg_program: gpg_program
                        .and_then(|v| gix::path::os_string_into_bstring(v).ok().map(Into::into)),
                    gpg_ssh_program: gpg_ssh_program
                        .and_then(|v| gix::path::os_string_into_bstring(v).ok().map(Into::into)),
                }
            }
        }

        impl From<GitConfigSettings> for crate::GitConfigSettings {
            fn from(
                GitConfigSettings {
                    gitbutler_sign_commits,
                    gitbutler_gerrit_mode,
                    gitbutler_review_stacking_description,
                    gitbutler_github_stacking_mode,
                    gitbutler_forge_review_template_path,
                    gitbutler_gitlab_project_id,
                    gitbutler_gitlab_upstream_project_id,
                    signing_key,
                    signing_format,
                    gpg_program,
                    gpg_ssh_program,
                }: GitConfigSettings,
            ) -> Self {
                crate::GitConfigSettings {
                    gitbutler_sign_commits,
                    gitbutler_gerrit_mode,
                    gitbutler_review_stacking_description,
                    gitbutler_github_stacking_mode,
                    gitbutler_forge_review_template_path: gitbutler_forge_review_template_path
                        .map(Into::into),
                    gitbutler_gitlab_project_id,
                    gitbutler_gitlab_upstream_project_id,
                    signing_key: signing_key.map(Into::into),
                    signing_format: signing_format.map(Into::into),
                    gpg_program: gpg_program.map(Into::into),
                    gpg_ssh_program: gpg_ssh_program.map(Into::into),
                }
            }
        }
    }

    pub(crate) mod types {
        use std::ffi::OsString;

        pub use super::ui::{GitHubStackingMode, ReviewStackingDescription};
        use bstr::BString;

        /// Settings that are retrieved from Git and written into the repository-local configuration.
        ///
        /// Some are specific to GitButler.
        #[derive(Debug, PartialEq, Clone, Default)]
        pub struct GitConfigSettings {
            /// If `true` GitButler should sign commits.
            /// This value is always set when querying it:
            /// * if `gitbutler.signCommits` is set, this value takes precedence over
            /// * `commit.gpgsign` which is otherwise valid.
            /// * otherwise it defaults to `false` just like Git would.
            pub gitbutler_sign_commits: Option<bool>,
            /// If `true`, GitButler will create ChangeId trailers and will push references in the Gerrit way
            pub gitbutler_gerrit_mode: Option<bool>,
            /// Controls where GitButler puts stack information in review descriptions.
            pub gitbutler_review_stacking_description: Option<ReviewStackingDescription>,
            /// Controls whether GitHub pull requests are registered with the native stacks API.
            pub gitbutler_github_stacking_mode: Option<GitHubStackingMode>,
            /// The path to the review description template to be used for this repository.
            pub gitbutler_forge_review_template_path: Option<BString>,
            /// The project ID of the GitLab project this repository is associated with, if any.
            pub gitbutler_gitlab_project_id: Option<String>,
            /// The project ID of the upstream GitLab project this repository is associated with, if any.
            /// In the case of a fork, this is the project ID of the parent project, otherwise it is the same as `gitbutler_gitlab_project_id`.
            pub gitbutler_gitlab_upstream_project_id: Option<String>,
            /// `user.signingKey`.
            pub signing_key: Option<BString>,
            /// `gpg.format`
            pub signing_format: Option<BString>,
            /// `gpg.program`
            pub gpg_program: Option<OsString>,
            /// `gpg.ssh.program`
            pub gpg_ssh_program: Option<OsString>,
        }
    }
    use types::GitConfigSettings;
    use ui::{GitHubStackingMode, ReviewStackingDescription};

    impl GitConfigSettings {
        /// Read all settings from the given snapshot.
        pub fn try_from_snapshot(config: &gix::config::Snapshot<'_>) -> anyhow::Result<Self> {
            fn string_or_ignore(v: BString) -> Option<String> {
                Vec::from(v).into_string().ok()
            }
            let gitbutler_sign_commits = config
                .boolean(GITBUTLER_SIGN_COMMITS)
                .or_else(|| config.boolean(GIT_SIGN_COMMITS))
                .or(Some(false));
            let gitbutler_gerrit_mode = config.boolean(GITBUTLER_GERRIT_MODE).or(Some(false));
            let gitbutler_review_stacking_description = config
                .string(GITBUTLER_REVIEW_STACKING_DESCRIPTION)
                .map(|value| match value.as_slice() {
                    b"bottom" => ReviewStackingDescription::Bottom,
                    b"top" => ReviewStackingDescription::Top,
                    b"disabled" => ReviewStackingDescription::Disabled,
                    invalid => {
                        tracing::warn!(value = ?invalid, "Invalid gitbutler.reviewStackingDescription; using bottom");
                        ReviewStackingDescription::Bottom
                    }
                });
            let gitbutler_github_stacking_mode = config
                .string(GITBUTLER_GITHUB_STACKING_MODE)
                .map(|value| match value.as_slice() {
                    b"auto" => GitHubStackingMode::Auto,
                    b"disabled" => GitHubStackingMode::Disabled,
                    b"native" => GitHubStackingMode::Native,
                    invalid => {
                        tracing::warn!(value = ?invalid, "Invalid gitbutler.githubStackingMode; using auto");
                        GitHubStackingMode::Auto
                    }
                });
            let gitbutler_forge_review_template_path = config.string(GITBUTLER_FORGE_TEMPLATE_PATH);
            let gitbutler_gitlab_project_id = config
                .string(GITBUTLER_GITLAB_PROJECT_ID)
                .and_then(string_or_ignore);
            let gitbutler_gitlab_upstream_project_id = config
                .string(GITBUTLER_GITLAB_UPSTREAM_PROJECT_ID)
                .and_then(string_or_ignore);
            let signing_key = config.string(SIGNING_KEY);
            let signing_format = config.string(SIGNING_FORMAT);
            let gpg_program = config.trusted_program(GPG_PROGRAM);
            let gpg_ssh_program = config.trusted_program(GPG_SSH_PROGRAM);
            Ok(GitConfigSettings {
                gitbutler_sign_commits,
                gitbutler_gerrit_mode,
                gitbutler_review_stacking_description,
                gitbutler_github_stacking_mode,
                gitbutler_forge_review_template_path,
                gitbutler_gitlab_project_id,
                gitbutler_gitlab_upstream_project_id,
                signing_key,
                signing_format,
                gpg_program,
                gpg_ssh_program,
            })
        }

        /// Write our data back to the local `.git/config` file of the given `repo`.
        pub fn persist_to_local_config(&self, repo: &gix::Repository) -> Result<()> {
            // TODO: make this easier in `gix`. Could use config-snapshot-mut, but there is no way to
            //       auto-reload it/assure it's up-to-date.
            edit_repo_config(repo, gix::config::Source::Local, |config| {
                if let Some(sign_commits) = self.gitbutler_sign_commits {
                    config.set_raw_value(
                        GITBUTLER_SIGN_COMMITS,
                        if sign_commits { "true" } else { "false" },
                    )?;
                };
                if let Some(gerrit_mode) = self.gitbutler_gerrit_mode {
                    config.set_raw_value(
                        GITBUTLER_GERRIT_MODE,
                        if gerrit_mode { "true" } else { "false" },
                    )?;
                };
                if let Some(description) = self.gitbutler_review_stacking_description {
                    config.set_raw_value(
                        GITBUTLER_REVIEW_STACKING_DESCRIPTION,
                        match description {
                            ReviewStackingDescription::Bottom => "bottom",
                            ReviewStackingDescription::Top => "top",
                            ReviewStackingDescription::Disabled => "disabled",
                        },
                    )?;
                };
                if let Some(mode) = self.gitbutler_github_stacking_mode {
                    config.set_raw_value(
                        GITBUTLER_GITHUB_STACKING_MODE,
                        match mode {
                            GitHubStackingMode::Auto => "auto",
                            GitHubStackingMode::Disabled => "disabled",
                            GitHubStackingMode::Native => "native",
                        },
                    )?;
                };
                if let Some(forge_template_path) = &self.gitbutler_forge_review_template_path {
                    if forge_template_path.is_empty() {
                        remove_config_value(config, GITBUTLER_FORGE_TEMPLATE_PATH)?;
                    } else {
                        config.set_raw_value(
                            GITBUTLER_FORGE_TEMPLATE_PATH,
                            forge_template_path.as_bstr(),
                        )?;
                    }
                };
                if let Some(signing_key) = &self.signing_key {
                    if signing_key.is_empty() {
                        remove_config_value(config, SIGNING_KEY)?;
                    } else {
                        config.set_raw_value(SIGNING_KEY, signing_key.as_bstr())?;
                    }
                };
                if let Some(signing_format) = &self.signing_format {
                    if signing_format.is_empty() {
                        remove_config_value(config, SIGNING_FORMAT)?;
                    } else {
                        config.set_raw_value(SIGNING_FORMAT, signing_format.as_bstr())?;
                    }
                }
                if let Some(gpg_program) = self.gpg_program.as_ref().and_then(osstring_into_bstring)
                {
                    if gpg_program.is_empty() {
                        remove_config_value(config, GPG_PROGRAM)?;
                    } else {
                        config.set_raw_value(GPG_PROGRAM, gpg_program.as_bstr())?;
                    }
                }
                if let Some(gpg_ssh_program) = self
                    .gpg_ssh_program
                    .as_ref()
                    .and_then(osstring_into_bstring)
                {
                    if gpg_ssh_program.is_empty() {
                        remove_config_value(config, GPG_SSH_PROGRAM)?;
                    } else {
                        config.set_raw_value(GPG_SSH_PROGRAM, gpg_ssh_program.as_bstr())?;
                    }
                }
                if let Some(gitlab_project_id) = self.gitbutler_gitlab_project_id.as_deref() {
                    if gitlab_project_id.is_empty() {
                        remove_config_value(config, GITBUTLER_GITLAB_PROJECT_ID)?;
                    } else {
                        config.set_raw_value(GITBUTLER_GITLAB_PROJECT_ID, gitlab_project_id)?;
                    }
                }
                if let Some(gitlab_upstream_project_id) =
                    self.gitbutler_gitlab_upstream_project_id.as_deref()
                {
                    if gitlab_upstream_project_id.is_empty() {
                        remove_config_value(config, GITBUTLER_GITLAB_UPSTREAM_PROJECT_ID)?;
                    } else {
                        config.set_raw_value(
                            GITBUTLER_GITLAB_UPSTREAM_PROJECT_ID,
                            gitlab_upstream_project_id,
                        )?;
                    }
                }

                Ok(())
            })?;
            Ok(())
        }
    }

    fn osstring_into_bstring(s: &OsString) -> Option<BString> {
        match gix::path::os_str_into_bstr(s) {
            Ok(s) => Some(s.to_owned()),
            Err(err) => {
                tracing::warn!("Could not convert to string due to illegal UTF8: {err}");
                None
            }
        }
    }
}
