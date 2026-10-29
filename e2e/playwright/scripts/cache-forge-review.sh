#!/bin/bash

directory="${1:-local-clone}"
review_number="$2"
source_branch="$3"
target_branch="${4:-master}"
database="$directory/.git/gitbutler/but.sqlite"

if [ -z "$review_number" ]; then
  echo "A review number is required" >&2
  exit 1
fi

if ! [[ "$review_number" =~ ^[0-9]+$ ]]; then
  echo "Review number must be numeric" >&2
  exit 1
fi

if [ -z "$source_branch" ]; then
  echo "A source branch is required" >&2
  exit 1
fi

python3 - "$database" "$review_number" "$source_branch" "$target_branch" <<'PY'
import sqlite3
import sys

database, review_number, source_branch, target_branch = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]

with sqlite3.connect(database) as connection:
    connection.execute(
        """
        -- struct_version must match ForgeReview::struct_version() in
        -- crates/but-forge/src/review.rs, or the backend discards the row
        -- as an incompatible cache entry and the PR association never forms.
        INSERT INTO forge_reviews (
            html_url, number, title, body, author, labels, draft,
            source_branch, target_branch, sha, integration_commit_shas,
            created_at, modified_at, merged_at, closed_at,
            repository_ssh_url, repository_https_url, repo_owner,
            head_repo_is_fork, reviewers, unit_symbol, last_sync_at, struct_version
        )
        VALUES (
            ?, ?, ?, NULL, NULL, '[]', FALSE,
            ?, ?, ?, '[]',
            datetime('now'), datetime('now'), NULL, NULL,
            NULL, NULL, NULL,
            FALSE, '[]', '#', datetime('now'), 4
        )
        ON CONFLICT(number) DO UPDATE SET
            title = excluded.title,
            source_branch = excluded.source_branch,
            target_branch = excluded.target_branch,
            sha = excluded.sha,
            modified_at = excluded.modified_at,
            merged_at = NULL,
            closed_at = NULL,
            head_repo_is_fork = excluded.head_repo_is_fork,
            last_sync_at = excluded.last_sync_at,
            struct_version = excluded.struct_version
        """,
        (
            f"https://github.com/acme/widgets/pull/{review_number}",
            review_number,
            f"Review for {source_branch}",
            source_branch,
            target_branch,
            "0" * 40,
        ),
    )
PY
