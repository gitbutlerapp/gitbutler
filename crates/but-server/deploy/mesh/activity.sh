#!/usr/bin/env bash
# What's going on in one account on mesh.but.dev, for debugging a live problem:
#   crates/but-server/deploy/mesh/activity.sh <account id> [hours, default 2]
# Shows the follower activity the account's machines reported, its machines' connections and
# requests from the server's log, and what waits in its machines' inboxes. Needs AWS credentials
# for the account (SSM).
set -euo pipefail

REGION="${MESH_REGION:-us-east-1}"
INSTANCE="${MESH_INSTANCE:?set MESH_INSTANCE to the instance id}"
ACCOUNT="${1:?pass the account id, as its directory under /var/lib/mesh/data/users is named}"
HOURS="${2:-2}"
[[ "$ACCOUNT" =~ ^[0-9]+$ ]] || { echo "the account id is a number" >&2; exit 1; }
[[ "$HOURS" =~ ^[0-9]+$ ]] || { echo "hours is a number" >&2; exit 1; }

script=$(cat <<EOF
dir=/var/lib/mesh/data/users/$ACCOUNT
echo "== follower activity (newest last)"
tail -n 30 "\$dir/activity.jsonl" 2>/dev/null || echo "  none reported"
echo "== connections, last $HOURS h"
journalctl -u mesh --since "-${HOURS}h" --no-pager -o cat | grep -E "events (dis)?connected" | grep "user: $ACCOUNT " | tail -n 20
echo "== requests by machine and client, last $HOURS h"
journalctl -u mesh --since "-${HOURS}h" --no-pager -o cat | grep "hub request" | grep "user: $ACCOUNT " \
  | sed -E 's/.*machine: "([^"]*)".*client: "([^"]*)".*status: ([0-9]+).*/\1 | \2 | \3/' | sort | uniq -c | sort -rn | head -n 20
echo "== waiting in inboxes"
for s in "\$dir"/store/*.git; do
  [ -f "\$s/HEAD" ] || continue
  git --git-dir="\$s" for-each-ref --format="  \$(git --git-dir=\$s config gitbutler.title): %(refname:lstrip=3) since %(committerdate:relative)" refs/gitbutler/inbox
done
EOF
)
id=$(aws ssm send-command --region "$REGION" --instance-ids "$INSTANCE" --document-name AWS-RunShellScript \
  --parameters "commands=[\"echo $(printf '%s' "$script" | base64) | base64 -d | bash\"]" \
  --query Command.CommandId --output text)
for _ in $(seq 1 20); do
  sleep 1
  status=$(aws ssm get-command-invocation --region "$REGION" --command-id "$id" --instance-id "$INSTANCE" --query Status --output text 2>/dev/null || true)
  [[ "$status" == "Success" || "$status" == "Failed" ]] && break
done
aws ssm get-command-invocation --region "$REGION" --command-id "$id" --instance-id "$INSTANCE" \
  --query StandardOutputContent --output text
