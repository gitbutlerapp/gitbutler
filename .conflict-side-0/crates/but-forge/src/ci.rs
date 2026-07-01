use serde::{Deserialize, Serialize};

use crate::ForgeName;

pub fn ci_checks_for_ref_with_cache(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
    reference: &str,
    db: &mut but_db::DbHandle,
    cache_config: Option<crate::CacheConfig>,
) -> anyhow::Result<Vec<CiCheck>> {
    let cache_config = cache_config.unwrap_or_default();
    let checks = match cache_config {
        crate::CacheConfig::CacheOnly => crate::db::ci_checks_from_cache(db, reference)?,
        crate::CacheConfig::CacheWithFallback { max_age_seconds } => {
            let cached = crate::db::ci_checks_from_cache(db, reference)?;
            if let Some(last_sync) = cached.first().map(|c| c.last_sync_at) {
                let age = chrono::Local::now().naive_local() - last_sync;
                if !cached.is_empty() && age.num_seconds() as u64 <= max_age_seconds {
                    return Ok(cached);
                }
            }
            let checks =
                ci_checks_for_ref(preferred_forge_user, forge_repo_info, storage, reference)?;
            crate::db::cache_ci_checks(db, reference, &checks).ok();
            checks
        }
        crate::CacheConfig::NoCache => {
            let checks =
                ci_checks_for_ref(preferred_forge_user, forge_repo_info, storage, reference)?;
            crate::db::cache_ci_checks(db, reference, &checks).ok();
            checks
        }
    };
    Ok(checks)
}

fn ci_checks_for_ref(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
    reference: &str,
) -> anyhow::Result<Vec<CiCheck>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.github().cloned());
            let gh = but_github::GitHubClient::from_storage(storage, preferred_account.as_ref())?;

            // Clone owned data for thread
            let owner = owner.clone();
            let repo = repo.clone();
            let reference = reference.to_string();
            let reference_for_checks = reference.clone();

            let checks = std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(gh.list_checks_for_ref(&owner, &repo, &reference))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))?;
            checks.map(|c| {
                c.into_iter()
                    .map(|check| {
                        let mut ci_check = CiCheck::from(check);
                        ci_check.reference = reference_for_checks.to_string();
                        ci_check
                    })
                    .collect()
            })
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitlab().cloned());
            let gl = but_gitlab::GitLabClient::from_storage(storage, preferred_account.as_ref())?;

            // Clone owned data for thread
            let project_id = but_gitlab::GitLabProjectId::new(owner, repo);
            let reference = reference.to_string();
            let reference_for_checks = reference.clone();

            let pipelines = std::thread::spawn(move || -> anyhow::Result<_> {
                let runtime = tokio::runtime::Runtime::new()
                    .map_err(|err| anyhow::anyhow!("Failed to create tokio runtime: {err}"))?;
                runtime.block_on(gl.list_pipeline_jobs_for_ref(project_id, &reference))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;
            Ok(pipelines
                .into_iter()
                .map(|pipeline| {
                    let mut ci_check = CiCheck::from(pipeline);
                    ci_check.reference = reference_for_checks.to_string();
                    ci_check
                })
                .collect())
        }
        _ => Err(anyhow::anyhow!(
            "Listing ci checks for forge {forge:?} is not implemented yet."
        )),
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CiCheck {
    pub id: i64,
    pub name: String,
    pub output: CiOutput,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: CiStatus,
    pub head_sha: String,
    pub url: String,
    pub html_url: String,
    pub details_url: String,
    pub pull_requests: Vec<PullRequestMinimal>,
    #[serde(skip_serializing)]
    pub reference: String,
    #[serde(skip_serializing)]
    pub last_sync_at: chrono::NaiveDateTime,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CiCheck);

impl CiCheck {
    /// The struct version for persistence compatibility purposes
    pub fn struct_version() -> i32 {
        1
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CiOutput {
    pub summary: String,
    pub text: String,
    pub title: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CiOutput);

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum CiStatus {
    Complete {
        conclusion: CiConclusion,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    InProgress,
    Queued,
    Unknown,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CiStatus);

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum CiConclusion {
    ActionRequired,
    Cancelled,
    Failure,
    Neutral,
    Skipped,
    Success,
    TimedOut,
    Unknown,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CiConclusion);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PullRequestMinimal {
    pub id: i64,
    pub number: i64,
    pub url: String,
    pub base_ref: String,
    pub base_repo_url: Option<String>,
    pub head_ref: String,
    pub head_repo_url: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PullRequestMinimal);

impl From<but_gitlab::GitLabPipelineJob> for CiCheck {
    fn from(job: but_gitlab::GitLabPipelineJob) -> Self {
        let started_at = job
            .started_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let completed_at = job
            .finished_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let status = match job.status.as_str() {
            "success" => CiStatus::Complete {
                conclusion: CiConclusion::Success,
                completed_at,
            },
            "failed" => CiStatus::Complete {
                conclusion: if job.allow_failure {
                    CiConclusion::Neutral
                } else {
                    CiConclusion::Failure
                },
                completed_at,
            },
            "canceled" => CiStatus::Complete {
                conclusion: CiConclusion::Cancelled,
                completed_at,
            },
            "skipped" => CiStatus::Complete {
                conclusion: CiConclusion::Skipped,
                completed_at,
            },
            "manual" => CiStatus::Complete {
                conclusion: if job.allow_failure {
                    CiConclusion::Neutral
                } else {
                    CiConclusion::ActionRequired
                },
                completed_at,
            },
            "running" | "canceling" => CiStatus::InProgress,
            "pending"
            | "created"
            | "waiting_for_resource"
            | "waiting_for_callback"
            | "preparing"
            | "scheduled" => CiStatus::Queued,
            _ => CiStatus::Unknown,
        };

        let job_url = job.web_url.clone().unwrap_or_default();
        let pipeline_url = job
            .pipeline
            .web_url
            .clone()
            .unwrap_or_else(|| job_url.clone());

        CiCheck {
            id: job.id,
            name: job.name,
            output: CiOutput::default(),
            started_at,
            status,
            head_sha: String::new(),
            url: job_url.clone(),
            html_url: job_url.clone(),
            details_url: pipeline_url,
            pull_requests: Vec::new(),
            reference: String::new(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }
}

impl From<but_github::CheckRun> for CiCheck {
    fn from(value: but_github::CheckRun) -> Self {
        let completed_at = value
            .completed_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let status = if value.status == "completed" {
            if let Some(conclusion) = value.conclusion {
                CiStatus::Complete {
                    conclusion: match conclusion.as_str() {
                        "action_required" => CiConclusion::ActionRequired,
                        "cancelled" => CiConclusion::Cancelled,
                        "failure" => CiConclusion::Failure,
                        "neutral" => CiConclusion::Neutral,
                        "skipped" => CiConclusion::Skipped,
                        "success" => CiConclusion::Success,
                        "timed_out" => CiConclusion::TimedOut,
                        _ => CiConclusion::Unknown,
                    },
                    completed_at,
                }
            } else {
                CiStatus::Unknown
            }
        } else if value.status == "in_progress" {
            CiStatus::InProgress
        } else if value.status == "queued" {
            CiStatus::Queued
        } else {
            CiStatus::Unknown
        };

        let started_at = value
            .started_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        CiCheck {
            id: value.id,
            name: value.name,
            output: CiOutput::default(),
            started_at,
            status,
            head_sha: value.head_sha.unwrap_or_default(),
            url: value
                .url
                .or_else(|| value.html_url.clone())
                .unwrap_or_default(),
            html_url: value.html_url.clone().unwrap_or_default(),
            details_url: value.details_url.or(value.html_url).unwrap_or_default(),
            pull_requests: Vec::new(),
            reference: String::new(), // Will be set by the caller
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CiCheck, CiConclusion, CiStatus};

    fn job(status: &str, web_url: Option<&str>) -> but_gitlab::GitLabPipelineJob {
        but_gitlab::GitLabPipelineJob {
            id: 42,
            name: "job".into(),
            status: status.into(),
            allow_failure: false,
            started_at: Some("2026-05-01T12:00:00Z".into()),
            finished_at: Some("2026-05-01T12:05:00Z".into()),
            web_url: web_url.map(str::to_owned),
            pipeline: but_gitlab::GitLabPipelineRef {
                id: 7,
                web_url: None,
                status: None,
            },
        }
    }

    #[test]
    fn maps_manual_jobs_to_action_required_complete_status() {
        let check = CiCheck::from(job("manual", Some("https://example.com/job")));

        assert!(matches!(
            check.status,
            CiStatus::Complete {
                conclusion: CiConclusion::ActionRequired,
                ..
            }
        ));
    }

    #[test]
    fn maps_allowed_failure_jobs_to_neutral_complete_status() {
        let mut job = job("failed", Some("https://example.com/job"));
        job.allow_failure = true;

        let check = CiCheck::from(job);

        assert!(matches!(
            check.status,
            CiStatus::Complete {
                conclusion: CiConclusion::Neutral,
                ..
            }
        ));
    }

    #[test]
    fn maps_optional_manual_jobs_to_neutral_complete_status() {
        let mut job = job("manual", Some("https://example.com/job"));
        job.allow_failure = true;

        let check = CiCheck::from(job);

        assert!(matches!(
            check.status,
            CiStatus::Complete {
                conclusion: CiConclusion::Neutral,
                ..
            }
        ));
    }

    #[test]
    fn maps_canceling_jobs_to_in_progress_status() {
        let check = CiCheck::from(job("canceling", Some("https://example.com/job")));

        assert!(matches!(check.status, CiStatus::InProgress));
    }

    #[test]
    fn maps_waiting_for_callback_jobs_to_queued_status() {
        let check = CiCheck::from(job("waiting_for_callback", Some("https://example.com/job")));

        assert!(matches!(check.status, CiStatus::Queued));
    }
}
