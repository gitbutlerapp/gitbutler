use super::ForgeReview;

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
            created_at: parse_datetime(&value.created_at),
            modified_at: parse_datetime(&value.modified_at),
            merged_at: parse_datetime(&value.merged_at),
            closed_at: parse_datetime(&value.closed_at),
            repository_ssh_url: value.repository_ssh_url,
            repository_https_url: value.repository_https_url,
            repo_owner: value.repo_owner,
            reviewers: serde_json::to_string(&value.reviewers)?,
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
            created_at: to_iso_8601(&value.created_at),
            modified_at: to_iso_8601(&value.modified_at),
            merged_at: to_iso_8601(&value.merged_at),
            closed_at: to_iso_8601(&value.closed_at),
            repository_ssh_url: value.repository_ssh_url,
            repository_https_url: value.repository_https_url,
            repo_owner: value.repo_owner,
            reviewers: serde_json::from_str(&value.reviewers)?,
            unit_symbol: value.unit_symbol,
            last_sync_at: value.last_sync_at,
        })
    }
}

pub(crate) fn reviews_from_cache(db: &but_db::DbHandle) -> anyhow::Result<Vec<ForgeReview>> {
    let db_reviews = db.forge_reviews().list_all()?;
    let reviews: Vec<ForgeReview> = db_reviews
        .into_iter()
        .map(|r| r.try_into())
        .collect::<anyhow::Result<Vec<ForgeReview>>>()?;
    Ok(reviews)
}

pub(crate) fn cache_reviews(
    db: &mut but_db::DbHandle,
    reviews: &[ForgeReview],
) -> anyhow::Result<()> {
    let db_reviews: Vec<but_db::ForgeReview> = reviews
        .iter()
        .map(|r| r.clone().try_into())
        .collect::<anyhow::Result<Vec<but_db::ForgeReview>>>(
    )?;
    db.forge_reviews_mut()?
        .set_all(db_reviews)
        .map_err(Into::into)
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
