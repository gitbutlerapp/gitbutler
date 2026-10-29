#!/bin/bash

directory="${1:-local-clone}"
review_number="$2"
database="$directory/.git/gitbutler/but.sqlite"

if [ -z "$review_number" ]; then
  echo "A review number is required" >&2
  exit 1
fi

if ! [[ "$review_number" =~ ^[0-9]+$ ]]; then
  echo "Review number must be numeric" >&2
  exit 1
fi

# Listed and optimistically inserted reviews share the same cache table. Age
# this fixture beyond the optimistic-insert grace period so an empty live list
# reconciles it as genuinely stale.
python3 - "$database" "$review_number" <<'PY'
import sqlite3
import sys

database, review_number = sys.argv[1], int(sys.argv[2])

with sqlite3.connect(database) as connection:
    connection.execute(
        """
        UPDATE forge_reviews
        SET last_sync_at = datetime('now', '-2 minutes')
        WHERE number = ?
        """,
        (review_number,),
    )
PY
