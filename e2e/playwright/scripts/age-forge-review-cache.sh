#!/bin/bash

directory="${1:-local-clone}"
review_number="$2"
database="$directory/.git/gitbutler/but.sqlite"

if [ -z "$review_number" ]; then
  echo "A review number is required" >&2
  exit 1
fi

# Listed and optimistically inserted reviews share the same cache table. Age
# this fixture beyond the optimistic-insert grace period so an empty live list
# reconciles it as genuinely stale.
sqlite3 "$database" \
  "UPDATE forge_reviews SET last_sync_at = datetime('now', '-2 minutes') WHERE number = $review_number;"
