/// A module to bundle configuration we *write* per repository, but read as normal.
pub mod git {
    use std::{borrow::Cow, ffi::OsString};

    use anyhow::Result;
    use bstr::{BStr, BString, ByteSlice, ByteVec};

    const GIT_SIGN_COMMITS: &str = "commit.gpgsign";
    const GITBUTLER_SIGN_COMMITS: &str = "gitbutler.signCommits";
    const GITBUTLER_GERRIT_MODE: &str = "gitbutler.gerritMode";
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

        /// See [`GitConfigSettings`](crate::GitConfigSettings) for the docs.
        #[derive(Debug, PartialEq, Clone, Default, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "export-ts", ts(export, export_to = "./settings/gitConfigSettings.ts"))]
        #[expect(missing_docs)]
        pub struct GitConfigSettings {
            #[serde(rename = "signCommits")]
            pub gitbutler_sign_commits: Option<bool>,
            pub gitbutler_gerrit_mode: Option<bool>,
            #[cfg_attr(feature = "export-ts", ts(type = "string | null"))]
            pub gitbutler_forge_review_template_path: Option<BStringForFrontend>,
            pub gitbutler_gitlab_project_id: Option<String>,
            pub gitbutler_gitlab_upstream_project_id: Option<String>,
            #[cfg_attr(feature = "export-ts", ts(type = "string | null"))]
            pub signing_key: Option<BStringForFrontend>,
            #[cfg_attr(feature = "export-ts", ts(type = "string | null"))]
            pub signing_format: Option<BStringForFrontend>,
            #[cfg_attr(feature = "export-ts", ts(type = "string | null"))]
            pub gpg_program: Option<BStringForFrontend>,
            #[cfg_attr(feature = "export-ts", ts(type = "string | null"))]
            pub gpg_ssh_program: Option<BStringForFrontend>,
        }

        impl From<crate::GitConfigSettings> for GitConfigSettings {
            fn from(
                crate::GitConfigSettings {
                    gitbutler_sign_commits,
                    gitbutler_gerrit_mode,
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
                    gitbutler_forge_review_template_path: gitbutler_forge_review_template_path.map(Into::into),
                    gitbutler_gitlab_project_id,
                    gitbutler_gitlab_upstream_project_id,
                    signing_key: signing_key.map(Into::into),
                    signing_format: signing_format.map(Into::into),
                    gpg_program: gpg_program.and_then(|v| gix::path::os_string_into_bstring(v).ok().map(Into::into)),
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
                    gitbutler_forge_review_template_path: gitbutler_forge_review_template_path.map(Into::into),
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

    use crate::RepositoryExt;

    impl GitConfigSettings {
        /// Read all settings from the given snapshot.
        pub fn try_from_snapshot(config: &gix::config::Snapshot<'_>) -> anyhow::Result<Self> {
            fn string_or_ignore(v: Cow<'_, BStr>) -> Option<String> {
                Vec::from(v.into_owned()).into_string().ok()
            }
            let gitbutler_sign_commits = config
                .boolean(GITBUTLER_SIGN_COMMITS)
                .or_else(|| config.boolean(GIT_SIGN_COMMITS))
                .or(Some(false));
            let gitbutler_gerrit_mode = config.boolean(GITBUTLER_GERRIT_MODE).or(Some(false));
            let gitbutler_forge_review_template_path =
                config.string(GITBUTLER_FORGE_TEMPLATE_PATH).map(Cow::into_owned);
            let gitbutler_gitlab_project_id = config.string(GITBUTLER_GITLAB_PROJECT_ID).and_then(string_or_ignore);
            let gitbutler_gitlab_upstream_project_id = config
                .string(GITBUTLER_GITLAB_UPSTREAM_PROJECT_ID)
                .and_then(string_or_ignore);
            let signing_key = config.string(SIGNING_KEY).map(Cow::into_owned);
            let signing_format = config.string(SIGNING_FORMAT).map(Cow::into_owned);
            let gpg_program = config.trusted_program(GPG_PROGRAM).map(Cow::into_owned);
            let gpg_ssh_program = config.trusted_program(GPG_SSH_PROGRAM).map(Cow::into_owned);
            Ok(GitConfigSettings {
                gitbutler_sign_commits,
                gitbutler_gerrit_mode,
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
            let mut config = repo.local_common_config_for_editing()?;
            if let Some(sign_commits) = self.gitbutler_sign_commits {
                config.set_raw_value(&GITBUTLER_SIGN_COMMITS, if sign_commits { "true" } else { "false" })?;
            };
            if let Some(gerrit_mode) = self.gitbutler_gerrit_mode {
                config.set_raw_value(&GITBUTLER_GERRIT_MODE, if gerrit_mode { "true" } else { "false" })?;
            };
            if let Some(forge_template_path) = &self.gitbutler_forge_review_template_path {
                config.set_raw_value(&GITBUTLER_FORGE_TEMPLATE_PATH, forge_template_path.as_bstr())?;
            };
            if let Some(signing_key) = &self.signing_key {
                config.set_raw_value(&SIGNING_KEY, signing_key.as_bstr())?;
            };
            if let Some(signing_format) = &self.signing_format {
                config.set_raw_value(&SIGNING_FORMAT, signing_format.as_bstr())?;
            }
            if let Some(gpg_program) = self.gpg_program.as_ref().and_then(osstring_into_bstring) {
                config.set_raw_value(&GPG_PROGRAM, gpg_program.as_bstr())?;
            }
            if let Some(gpg_ssh_program) = self.gpg_ssh_program.as_ref().and_then(osstring_into_bstring) {
                config.set_raw_value(&GPG_SSH_PROGRAM, gpg_ssh_program.as_bstr())?;
            }
            if let Some(gitlab_project_id) = self.gitbutler_gitlab_project_id.as_deref() {
                config.set_raw_value(&GITBUTLER_GITLAB_PROJECT_ID, gitlab_project_id)?;
            }
            if let Some(gitlab_upstream_project_id) = self.gitbutler_gitlab_upstream_project_id.as_deref() {
                config.set_raw_value(&GITBUTLER_GITLAB_UPSTREAM_PROJECT_ID, gitlab_upstream_project_id)?;
            }

            repo.write_local_common_config(&config)?;
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
