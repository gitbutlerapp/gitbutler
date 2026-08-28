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
            fetch_and_refresh_cache(
                preferred_forge_user,
                forge_repo_info,
                storage,
                reference,
                db,
            )?
        }
        crate::CacheConfig::NoCache => fetch_and_refresh_cache(
            preferred_forge_user,
            forge_repo_info,
            storage,
            reference,
            db,
        )?,
    };
    Ok(checks)
}

/// Fetch fresh checks from the forge and refresh the cache with the result.
///
/// A resolvable ref returns an authoritative list — even an empty one, which
/// replaces the cache (e.g. after a force-push to a commit that has no CI). An
/// unresolvable ref (`None`, e.g. a transient GitHub 422 or a deleted Bitbucket
/// branch) is surfaced as "no checks" for display but must not overwrite the
/// previously-good checks that cache-only readers such as `but status` rely on,
/// since it can be transient.
fn fetch_and_refresh_cache(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
    reference: &str,
    db: &mut but_db::DbHandle,
) -> anyhow::Result<Vec<CiCheck>> {
    let fetched = ci_checks_for_ref(preferred_forge_user, forge_repo_info, storage, reference)?;
    Ok(refresh_cache_with_fetched(db, reference, fetched))
}

/// Apply a freshly-fetched result to the cache and return the checks to display.
///
/// `Some(checks)` is authoritative and refreshes the cache — even an empty vec,
/// which clears it (e.g. after a force-push to a commit that has no CI). `None`
/// means the ref did not resolve; it is surfaced as "no checks" for display but
/// deliberately leaves the cache untouched, since a
/// cache-only reader such as `but status` must not lose previously-good checks
/// to a momentary blip.
fn refresh_cache_with_fetched(
    db: &mut but_db::DbHandle,
    reference: &str,
    fetched: Option<Vec<CiCheck>>,
) -> Vec<CiCheck> {
    match fetched {
        Some(checks) => {
            crate::db::cache_ci_checks(db, reference, &checks).ok();
            checks
        }
        None => Vec::new(),
    }
}

/// Fetch checks from the forge. `None` means the ref did not resolve.
fn ci_checks_for_ref(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
    reference: &str,
) -> anyhow::Result<Option<Vec<CiCheck>>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.github().cloned());

            // Clone owned data for thread
            let owner = owner.clone();
            let repo = repo.clone();
            let storage = storage.clone();
            let reference = reference.to_string();
            let reference_for_checks = reference.clone();

            let checks = std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(but_github::checks::list_for_ref(
                        preferred_account.as_ref(),
                        &owner,
                        &repo,
                        &reference,
                        &storage,
                    ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))?;
            checks.map(|maybe_runs| {
                maybe_runs.map(|runs| {
                    runs.into_iter()
                        .map(|check| {
                            let mut ci_check = CiCheck::from(check);
                            ci_check.reference = reference_for_checks.to_string();
                            ci_check
                        })
                        .collect()
                })
            })
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitlab().cloned());

            // Clone owned data for thread
            let project_id = but_gitlab::GitLabProjectId::new(owner, repo);
            let storage = storage.clone();
            let reference = reference.to_string();
            let reference_for_checks = reference.clone();

            let pipelines = std::thread::spawn(move || -> anyhow::Result<_> {
                let runtime = tokio::runtime::Runtime::new()
                    .map_err(|err| anyhow::anyhow!("Failed to create tokio runtime: {err}"))?;
                runtime.block_on(but_gitlab::checks::list_pipeline_jobs_for_ref(
                    preferred_account.as_ref(),
                    project_id,
                    &reference,
                    &storage,
                ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;
            Ok(Some(
                pipelines
                    .into_iter()
                    .map(|pipeline| {
                        let mut ci_check = CiCheck::from(pipeline);
                        ci_check.reference = reference_for_checks.to_string();
                        ci_check
                    })
                    .collect(),
            ))
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket().cloned());

            // Clone owned data for thread
            let workspace = owner.clone();
            let repo_slug = repo.clone();
            let storage = storage.clone();
            let reference = reference.to_string();
            let reference_for_checks = reference.clone();

            let statuses = std::thread::spawn(move || -> anyhow::Result<_> {
                let runtime = tokio::runtime::Runtime::new()
                    .map_err(|err| anyhow::anyhow!("Failed to create tokio runtime: {err}"))?;
                runtime.block_on(but_bitbucket::checks::list_for_ref(
                    preferred_account.as_ref(),
                    &workspace,
                    &repo_slug,
                    &reference,
                    &storage,
                ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

            Ok(statuses.map(|statuses| {
                statuses
                    .into_iter()
                    .map(|status| {
                        let mut ci_check = CiCheck::from(status);
                        ci_check.reference = reference_for_checks.to_string();
                        ci_check
                    })
                    .collect()
            }))
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

impl From<but_bitbucket::BitbucketBuildStatus> for CiCheck {
    fn from(status: but_bitbucket::BitbucketBuildStatus) -> Self {
        let started_at = status
            .created_on
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let completed_at = status
            .updated_on
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let ci_status = match status.state.as_str() {
            "SUCCESSFUL" => CiStatus::Complete {
                conclusion: CiConclusion::Success,
                completed_at,
            },
            "FAILED" => CiStatus::Complete {
                conclusion: CiConclusion::Failure,
                completed_at,
            },
            "STOPPED" => CiStatus::Complete {
                conclusion: CiConclusion::Cancelled,
                completed_at,
            },
            "INPROGRESS" => CiStatus::InProgress,
            _ => CiStatus::Unknown,
        };

        let url = status.url.unwrap_or_default();
        CiCheck {
            id: but_bitbucket::stable_id_hash(&format!(
                "{}\u{1f}{}",
                status.commit_hash, status.key
            )),
            name: status.name,
            output: CiOutput {
                summary: status.description.unwrap_or_default(),
                ..Default::default()
            },
            started_at,
            status: ci_status,
            head_sha: status.commit_hash,
            url: url.clone(),
            html_url: url.clone(),
            details_url: url,
            pull_requests: Vec::new(),
            reference: String::new(), // Will be set by the caller
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CiCheck, CiConclusion, CiOutput, CiStatus, refresh_cache_with_fetched};

    const REFERENCE: &str = "refs/heads/feature";

    fn cache_check(id: i64) -> CiCheck {
        CiCheck {
            id,
            name: format!("check-{id}"),
            output: CiOutput::default(),
            started_at: None,
            status: CiStatus::InProgress,
            head_sha: "deadbeef".into(),
            url: String::new(),
            html_url: String::new(),
            details_url: String::new(),
            pull_requests: Vec::new(),
            reference: REFERENCE.to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }

    fn test_db() -> (tempfile::TempDir, but_db::DbHandle) {
        let tmp = tempfile::tempdir().unwrap();
        let db = but_db::DbHandle::new_in_directory(tmp.path()).unwrap();
        (tmp, db)
    }

    fn cached_ids(db: &but_db::DbHandle) -> Vec<i64> {
        let mut ids: Vec<i64> = crate::db::ci_checks_from_cache(db, REFERENCE)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn some_non_empty_result_replaces_the_cache() {
        let (_tmp, mut db) = test_db();
        refresh_cache_with_fetched(&mut db, REFERENCE, Some(vec![cache_check(1)]));

        let displayed = refresh_cache_with_fetched(
            &mut db,
            REFERENCE,
            Some(vec![cache_check(2), cache_check(3)]),
        );

        assert_eq!(displayed.len(), 2);
        assert_eq!(cached_ids(&db), vec![2, 3]);
    }

    #[test]
    fn some_empty_result_clears_the_cache() {
        let (_tmp, mut db) = test_db();
        refresh_cache_with_fetched(&mut db, REFERENCE, Some(vec![cache_check(1)]));

        // An authoritative empty list (e.g. a force-push to a commit with no CI)
        // replaces the cache.
        let displayed = refresh_cache_with_fetched(&mut db, REFERENCE, Some(Vec::new()));

        assert!(displayed.is_empty());
        assert!(
            cached_ids(&db).is_empty(),
            "an authoritative empty list should clear the cache"
        );
    }

    #[test]
    fn none_result_preserves_the_cache() {
        let (_tmp, mut db) = test_db();
        refresh_cache_with_fetched(&mut db, REFERENCE, Some(vec![cache_check(1)]));

        // An unresolvable ref shows "no checks" for display but must not wipe
        // the previously-good cache.
        let displayed = refresh_cache_with_fetched(&mut db, REFERENCE, None);

        assert!(displayed.is_empty(), "an unresolvable ref shows no checks");
        assert_eq!(
            cached_ids(&db),
            vec![1],
            "a transient unresolvable ref must not wipe the cache"
        );
    }

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

    fn bb_status(state: &str) -> but_bitbucket::BitbucketBuildStatus {
        but_bitbucket::BitbucketBuildStatus {
            key: "PIPELINE-1".into(),
            name: "Build".into(),
            description: Some("desc".into()),
            state: state.into(),
            url: Some("https://bitbucket.org/ws/repo/pipelines/1".into()),
            commit_hash: "deadbeef".into(),
            created_on: Some("2026-05-01T12:00:00Z".into()),
            updated_on: Some("2026-05-01T12:05:00Z".into()),
        }
    }

    #[test]
    fn maps_bitbucket_build_states() {
        assert!(matches!(
            CiCheck::from(bb_status("SUCCESSFUL")).status,
            CiStatus::Complete {
                conclusion: CiConclusion::Success,
                ..
            }
        ));
        assert!(matches!(
            CiCheck::from(bb_status("FAILED")).status,
            CiStatus::Complete {
                conclusion: CiConclusion::Failure,
                ..
            }
        ));
        assert!(matches!(
            CiCheck::from(bb_status("STOPPED")).status,
            CiStatus::Complete {
                conclusion: CiConclusion::Cancelled,
                ..
            }
        ));
        assert!(matches!(
            CiCheck::from(bb_status("INPROGRESS")).status,
            CiStatus::InProgress
        ));
        assert!(matches!(
            CiCheck::from(bb_status("WEIRD")).status,
            CiStatus::Unknown
        ));
    }

    #[test]
    fn bitbucket_build_status_id_is_stable_and_nonzero() {
        let a = CiCheck::from(bb_status("SUCCESSFUL"));
        let b = CiCheck::from(bb_status("SUCCESSFUL"));
        assert_eq!(a.id, b.id);
        assert_ne!(a.id, 0);
        assert_eq!(a.head_sha, "deadbeef");
    }

    #[test]
    fn bitbucket_build_status_id_differs_across_commits_and_keys() {
        let base = bb_status("SUCCESSFUL");

        // Same key on a different commit must not collide (the id is the
        // ci_checks primary key, and keys like "build" recur across refs/repos).
        let other_commit = but_bitbucket::BitbucketBuildStatus {
            commit_hash: "feedface".into(),
            ..bb_status("SUCCESSFUL")
        };
        assert_ne!(CiCheck::from(base).id, CiCheck::from(other_commit).id);

        // Different key on the same commit also differs.
        let other_key = but_bitbucket::BitbucketBuildStatus {
            key: "PIPELINE-2".into(),
            ..bb_status("SUCCESSFUL")
        };
        assert_ne!(
            CiCheck::from(bb_status("SUCCESSFUL")).id,
            CiCheck::from(other_key).id
        );
    }
}
