#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
cat >claims.c <<'EOF'
static void domain_recall_node_claims(void)
{
    return;
}
EOF
git add claims.c
git commit -m "base"
setup_target_to_match_main

git checkout -b A
for _ in $(seq 1 12); do
    echo '/* padding */' >>claims.c
done
cat >>claims.c <<'EOF'

void release_claims(void)
{
    domain_recall_node_claims();
}
EOF
for _ in $(seq 1 12); do
    echo '/* padding */' >>claims.c
done
cat >>claims.c <<'EOF'
void unset_claims(void)
{
    domain_recall_node_claims();
}
EOF
git add claims.c
git commit -m "add claim release call sites"

git checkout -b B
cat >claims.c <<'EOF'
static void domain_recall_node_claims(void)
{
    unsigned long recalled = 0;
    unsigned int node;

    for ( node = 0; node < 8; ++node )
    {
        recalled += node;
    }

    /* Keep claims from exceeding the domain's page limit. */
    /* The node-specific claims are tracked separately. */
    /* Allocations may be backed by another node. */
    /* Host-wide claims are redeemed before other nodes. */
    /* The caller holds heap_lock throughout this operation. */
    /* Claims on other nodes may still be available. */
    /* The domain's page limit remains enforced. */
    return recalled;
}

EOF
for _ in $(seq 1 12); do
    echo '/* padding */' >>claims.c
done
cat >>claims.c <<'EOF'
void release_claims(void)
{
    domain_recall_node_claims();
}

EOF
for _ in $(seq 1 12); do
    echo '/* padding */' >>claims.c
done
cat >>claims.c <<'EOF'
void unset_claims(void)
{
    domain_recall_node_claims();
}

EOF
for _ in $(seq 1 12); do
    echo '/* padding */' >>claims.c
done
cat >>claims.c <<'EOF'
void redeem_claims(void)
{
    domain_recall_node_claims();
}
EOF
git add claims.c
git commit -m "redeem claims during allocation"

create_workspace_commit_once B
