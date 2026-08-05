use super::ForgeReview;

const MERGED_REVIEW_RETENTION_DAYS: i64 = 15;
const ABSENT_OPEN_REVIEW_GRACE_SECONDS: i64 = 60;

impl TryFrom<ForgeReview> for but_db::ForgeReview {
    type Error = anyhow::Error;
    fn try_from(value: ForgeReview) -> anyhow::Result<Self, Self::Error> {
        fn parse_datetime(datetime_str: &Option<String>) -> Option<chrono::NaiveDateTime> {
            datetime_str
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.naive_local())
        }
        let version = ForgeReview::struct_version();
        Ok(but_db::ForgeReview {
            html_url: value.html_url,
            number: value.number,
            title: value.title,
            body: value.body,
            author: match value.author {
                Some(ref author) => Some(serde_json::to_string(author)?),
                None => None,
            },
            labels: serde_json::to_string(&value.labels)?,
            draft: value.draft,
            source_branch: value.source_branch,
            target_branch: value.target_branch,
            sha: value.sha,
            integration_commit_shas: serde_json::to_string(&value.integration_commit_shas)?,
            created_at: parse_datetime(&value.created_at),
            modified_at: parse_datetime(&value.modified_at),
            merged_at: parse_datetime(&value.merged_at),
            closed_at: parse_datetime(&value.closed_at),
            repository_ssh_url: value.repository_ssh_url,
            repository_https_url: value.repository_https_url,
            repo_owner: value.repo_owner,
            head_repo_is_fork: value.head_repo_is_fork,
            reviewers: serde_json::to_string(&value.reviewers)?,
            auto_merge_enabled: value.auto_merge_enabled,
            unit_symbol: value.unit_symbol,
            last_sync_at: value.last_sync_at,
            struct_version: version,
        })
    }
}

impl TryFrom<but_db::ForgeReview> for ForgeReview {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ForgeReview) -> anyhow::Result<Self, Self::Error> {
        fn to_iso_8601(datetime: &Option<chrono::NaiveDateTime>) -> Option<String> {
            datetime.map(|dt| {
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)
                    .to_rfc3339()
            })
        }
        if value.struct_version != ForgeReview::struct_version() {
            return Err(anyhow::Error::msg(format!(
                "Incompatible ForgeReview struct version: expected {}, found {}",
                ForgeReview::struct_version(),
                value.struct_version
            )));
        }
        Ok(ForgeReview {
            html_url: value.html_url,
            number: value.number,
            title: value.title,
            body: value.body,
            author: match value.author {
                Some(ref author_str) => Some(serde_json::from_str(author_str)?),
                None => None,
            },
            labels: serde_json::from_str(&value.labels)?,
            draft: value.draft,
            source_branch: value.source_branch,
            target_branch: value.target_branch,
            sha: value.sha,
            integration_commit_shas: serde_json::from_str(&value.integration_commit_shas)?,
            created_at: to_iso_8601(&value.created_at),
            modified_at: to_iso_8601(&value.modified_at),
            merged_at: to_iso_8601(&value.merged_at),
            closed_at: to_iso_8601(&value.closed_at),
            repository_ssh_url: value.repository_ssh_url,
            repository_https_url: value.repository_https_url,
            repo_owner: value.repo_owner,
            head_repo_is_fork: value.head_repo_is_fork,
            reviewers: serde_json::from_str(&value.reviewers)?,
            auto_merge_enabled: value.auto_merge_enabled,
            unit_symbol: value.unit_symbol,
            last_sync_at: value.last_sync_at,
        })
    }
}

pub(crate) struct CachedReviews {
    reviews: Vec<ForgeReview>,
    saw_incompatible: bool,
}

impl CachedReviews {
    pub(crate) fn fresh_rows(
        self,
        max_age_seconds: u64,
        now: chrono::NaiveDateTime,
    ) -> Option<Vec<ForgeReview>> {
        if self.saw_incompatible {
            return None;
        }
        let last_sync_at = self.reviews.first()?.last_sync_at;
        let age_seconds = (now - last_sync_at).num_seconds();
        (age_seconds >= 0 && age_seconds as u64 <= max_age_seconds).then_some(self.reviews)
    }
}

pub(crate) fn reviews_from_cache(db: &but_db::DbHandle) -> anyhow::Result<CachedReviews> {
    let db_reviews = db.forge_reviews().list_all()?;
    let expected_version = ForgeReview::struct_version();
    let saw_incompatible = db_reviews
        .iter()
        .any(|review| review.struct_version != expected_version);
    let reviews = db_reviews
        .into_iter()
        .filter(|review| review.struct_version == expected_version)
        .map(ForgeReview::try_from)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(CachedReviews {
        reviews,
        saw_incompatible,
    })
}

/// Lists compatible persisted reviews without performing network I/O.
///
/// Rows written with another [`ForgeReview::struct_version`] are cache misses.
pub fn list_cached_forge_reviews(db: &but_db::DbHandle) -> anyhow::Result<Vec<ForgeReview>> {
    Ok(reviews_from_cache(db)?.reviews)
}

/// Refreshes the cached review rows returned by a forge listing.
///
/// Listed reviews are upserted instead of replacing the whole table so directly
/// fetched reviews, such as recently merged PRs used by upstream integration,
/// are not deleted by an open-review list response. Cached open reviews that no
/// longer appear in the listed response are deleted, while retained merged
/// reviews are pruned after 15 days. Rows written with another persisted-model
/// version are deleted because their missing fields cannot be reconstructed.
pub(crate) fn cache_reviews(
    db: &mut but_db::DbHandle,
    reviews: &[ForgeReview],
) -> anyhow::Result<()> {
    let now = chrono::Local::now().naive_local();
    let merged_cutoff = now - chrono::Duration::days(MERGED_REVIEW_RETENTION_DAYS);
    let absent_open_cutoff = now - chrono::Duration::seconds(ABSENT_OPEN_REVIEW_GRACE_SECONDS);
    let db_reviews = reviews
        .iter()
        .map(|review| review.clone().try_into())
        .collect::<anyhow::Result<Vec<_>>>()?;
    db.forge_reviews_mut()?
        .reconcile_listed(
            db_reviews,
            ForgeReview::struct_version(),
            merged_cutoff,
            absent_open_cutoff,
        )
        .map_err(anyhow::Error::from)?;
    Ok(())
}

pub(crate) fn upsert_review(db: &mut but_db::DbHandle, review: &ForgeReview) -> anyhow::Result<()> {
    let db_review: but_db::ForgeReview = review.clone().try_into()?;
    db.forge_reviews_mut()?
        .upsert(db_review)
        .map_err(Into::into)
}

use super::CiCheck;

impl TryFrom<CiCheck> for but_db::CiCheck {
    type Error = anyhow::Error;
    fn try_from(value: CiCheck) -> anyhow::Result<Self, Self::Error> {
        let version = CiCheck::struct_version();
        let (status_type, status_conclusion, status_completed_at) = match value.status {
            super::CiStatus::Complete {
                conclusion,
                completed_at,
            } => {
                let conclusion_str = match conclusion {
                    super::CiConclusion::ActionRequired => "ActionRequired",
                    super::CiConclusion::Cancelled => "Cancelled",
                    super::CiConclusion::Failure => "Failure",
                    super::CiConclusion::Neutral => "Neutral",
                    super::CiConclusion::Skipped => "Skipped",
                    super::CiConclusion::Success => "Success",
                    super::CiConclusion::TimedOut => "TimedOut",
                    super::CiConclusion::Unknown => "Unknown",
                };
                (
                    "Complete".to_string(),
                    Some(conclusion_str.to_string()),
                    completed_at.map(|dt| dt.naive_local()),
                )
            }
            super::CiStatus::InProgress => ("InProgress".to_string(), None, None),
            super::CiStatus::Queued => ("Queued".to_string(), None, None),
            super::CiStatus::Unknown => ("Unknown".to_string(), None, None),
        };

        Ok(but_db::CiCheck {
            id: value.id,
            name: value.name,
            output_summary: value.output.summary,
            output_text: value.output.text,
            output_title: value.output.title,
            started_at: value.started_at.map(|dt| dt.naive_local()),
            status_type,
            status_conclusion,
            status_completed_at,
            head_sha: value.head_sha,
            url: value.url,
            html_url: value.html_url,
            details_url: value.details_url,
            pull_requests: serde_json::to_string(&value.pull_requests)?,
            reference: value.reference,
            last_sync_at: value.last_sync_at,
            struct_version: version,
        })
    }
}

impl TryFrom<but_db::CiCheck> for CiCheck {
    type Error = anyhow::Error;
    fn try_from(value: but_db::CiCheck) -> anyhow::Result<Self, Self::Error> {
        if value.struct_version != CiCheck::struct_version() {
            return Err(anyhow::Error::msg(format!(
                "Incompatible CiCheck struct version: expected {}, found {}",
                CiCheck::struct_version(),
                value.struct_version
            )));
        }

        let status = match value.status_type.as_str() {
            "Complete" => {
                let conclusion_str = value
                    .status_conclusion
                    .ok_or_else(|| anyhow::Error::msg("Complete status missing conclusion"))?;
                let conclusion = match conclusion_str.as_str() {
                    "ActionRequired" => super::CiConclusion::ActionRequired,
                    "Cancelled" => super::CiConclusion::Cancelled,
                    "Failure" => super::CiConclusion::Failure,
                    "Neutral" => super::CiConclusion::Neutral,
                    "Skipped" => super::CiConclusion::Skipped,
                    "Success" => super::CiConclusion::Success,
                    "TimedOut" => super::CiConclusion::TimedOut,
                    _ => super::CiConclusion::Unknown,
                };
                let completed_at = value
                    .status_completed_at
                    .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc));
                super::CiStatus::Complete {
                    conclusion,
                    completed_at,
                }
            }
            "InProgress" => super::CiStatus::InProgress,
            "Queued" => super::CiStatus::Queued,
            _ => super::CiStatus::Unknown,
        };

        Ok(CiCheck {
            id: value.id,
            name: value.name,
            output: super::CiOutput {
                summary: value.output_summary,
                text: value.output_text,
                title: value.output_title,
            },
            started_at: value
                .started_at
                .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc)),
            status,
            head_sha: value.head_sha,
            url: value.url,
            html_url: value.html_url,
            details_url: value.details_url,
            pull_requests: serde_json::from_str(&value.pull_requests)?,
            reference: value.reference,
            last_sync_at: value.last_sync_at,
        })
    }
}

pub(crate) fn ci_checks_from_cache(
    db: &but_db::DbHandle,
    reference: &str,
) -> anyhow::Result<Vec<CiCheck>> {
    let db_checks = db.ci_checks().list_for_reference(reference)?;
    let checks: Vec<CiCheck> = db_checks
        .into_iter()
        .map(|c| c.try_into())
        .collect::<anyhow::Result<Vec<CiCheck>>>()?;
    Ok(checks)
}

pub(crate) fn cache_ci_checks(
    db: &mut but_db::DbHandle,
    reference: &str,
    checks: &[CiCheck],
) -> anyhow::Result<()> {
    let db_checks: Vec<but_db::CiCheck> = checks
        .iter()
        .map(|c| c.clone().try_into())
        .collect::<anyhow::Result<Vec<but_db::CiCheck>>>()?;
    db.ci_checks_mut()?
        .set_for_reference(reference, db_checks)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{cache_reviews, list_cached_forge_reviews, reviews_from_cache};
    use crate::ForgeReview;

    fn review(
        number: i64,
        source_branch: &str,
        last_sync_at: chrono::NaiveDateTime,
    ) -> ForgeReview {
        ForgeReview {
            html_url: String::new(),
            number,
            title: String::new(),
            body: None,
            author: None,
            labels: Vec::new(),
            draft: false,
            source_branch: source_branch.to_string(),
            target_branch: "main".to_string(),
            sha: String::new(),
            integration_commit_shas: Vec::new(),
            created_at: None,
            modified_at: None,
            merged_at: None,
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            head_repo_is_fork: false,
            reviewers: Vec::new(),
            auto_merge_enabled: false,
            unit_symbol: "#".to_string(),
            last_sync_at,
        }
    }

    fn test_db() -> (tempfile::TempDir, but_db::DbHandle) {
        let tmp = tempfile::tempdir().unwrap();
        let db = but_db::DbHandle::new_in_directory(tmp.path()).unwrap();
        (tmp, db)
    }

    fn stored_review(
        number: i64,
        source_branch: &str,
        struct_version: i32,
        last_sync_at: chrono::NaiveDateTime,
    ) -> but_db::ForgeReview {
        let mut row: but_db::ForgeReview = review(number, source_branch, last_sync_at)
            .try_into()
            .unwrap();
        row.struct_version = struct_version;
        row
    }

    #[test]
    fn mixed_versions_are_filtered_and_force_a_refetch() {
        let (_tmp, mut db) = test_db();
        let now = chrono::Local::now().naive_local();
        db.forge_reviews_mut()
            .unwrap()
            .set_all(vec![
                stored_review(1, "stale-v1", 1, now),
                stored_review(2, "stale-v2", 2, now),
                stored_review(3, "current", ForgeReview::struct_version(), now),
            ])
            .unwrap();

        let compatible = list_cached_forge_reviews(&db).unwrap();
        assert_eq!(
            compatible
                .iter()
                .map(|review| review.number)
                .collect::<Vec<_>>(),
            vec![3],
            "cache-only readers should skip incompatible rows"
        );
        assert!(
            reviews_from_cache(&db)
                .unwrap()
                .fresh_rows(60, now)
                .is_none(),
            "fallback readers should refetch rather than reuse a partial cache"
        );
    }

    #[test]
    fn compatible_fresh_rows_are_reused() {
        let (_tmp, mut db) = test_db();
        let now = chrono::Local::now().naive_local();
        db.forge_reviews_mut()
            .unwrap()
            .set_all(vec![stored_review(
                3,
                "current",
                ForgeReview::struct_version(),
                now,
            )])
            .unwrap();

        let cached = reviews_from_cache(&db)
            .unwrap()
            .fresh_rows(60, now)
            .expect("a compatible fresh cache should be reused");
        assert_eq!(cached[0].number, 3, "the cached review should be returned");
    }

    #[test]
    fn compatible_stale_rows_force_a_refetch() {
        let (_tmp, mut db) = test_db();
        let now = chrono::Local::now().naive_local();
        db.forge_reviews_mut()
            .unwrap()
            .set_all(vec![stored_review(
                3,
                "current",
                ForgeReview::struct_version(),
                now - chrono::Duration::seconds(61),
            )])
            .unwrap();

        assert!(
            reviews_from_cache(&db)
                .unwrap()
                .fresh_rows(60, now)
                .is_none(),
            "a compatible cache older than the maximum age should be refetched"
        );
    }

    #[test]
    fn refreshing_the_cache_deletes_incompatible_rows() {
        let (_tmp, mut db) = test_db();
        let now = chrono::Local::now().naive_local();
        db.forge_reviews_mut()
            .unwrap()
            .set_all(vec![
                stored_review(1, "stale-v1", 1, now),
                stored_review(2, "stale-v2", 2, now),
            ])
            .unwrap();

        cache_reviews(&mut db, &[review(3, "current", now)]).unwrap();

        let rows = db.forge_reviews().list_all().unwrap();
        assert_eq!(rows.len(), 1, "refresh should discard stale cache entries");
        assert_eq!(
            rows[0].struct_version,
            ForgeReview::struct_version(),
            "refreshed cache only contains the current persisted model"
        );
    }

    #[test]
    fn current_version_corruption_is_an_error() {
        let (_tmp, mut db) = test_db();
        let now = chrono::Local::now().naive_local();
        let mut corrupt: but_db::ForgeReview = review(1, "corrupt", now).try_into().unwrap();
        corrupt.labels = "not-json".to_string();
        db.forge_reviews_mut()
            .unwrap()
            .set_all(vec![corrupt])
            .unwrap();

        assert!(
            list_cached_forge_reviews(&db).is_err(),
            "current-version corruption must not be treated as a cache miss"
        );
    }
}
