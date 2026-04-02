set -eu -o pipefail

function create_workspace_commit_once() {
  local workspace_commit_subject="GitButler Workspace Commit"

  if [ $# == 1 ]; then
    local current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "$1" ]]; then
      echo "BUG: Must assure the current branch is the branch passed as argument: $current_branch != $1"
      return 42
    fi
  fi

  git checkout -b gitbutler/workspace
  if [ $# == 1 ] || [ $# == 0 ]; then
    git commit --allow-empty -m "$workspace_commit_subject"
  else
    git merge --no-ff -m "$workspace_commit_subject" "${@}"
  fi
}

function create_consistent_signing_key_at() {
  if [ -z "${1-}" ]; then
    echo "usage: create_consistent_signing_key_at <output_path>" >&2
    return 1
  fi

  echo "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAABFwAAAAdzc2gtcn
NhAAAAAwEAAQAAAQEAuBhnTC0+8nJnjSpZEh7wBsBiEpiC3RtZfdnXo/JmNYQX4UXH1tFJ
OFjQFzjlM3OifXff9ppNYwGc71EM/DnTBkfZQsjEXxD3QGQGr0YjiVyWLPyi+nCfd7M3pN
C75RvUttNYPYY5oLJQqm5Af3oCyY5Pko0BJ9t0mN/x7Ns76RmDz4nUcxLzeA7GHGPXkbB/
VwIkAidev+mFhfwGYBlZIdke7x+jLogbWDV262vZDIAYV13AMo5uytt6Ow6HBsXu7s9MQZ
ZY7rdmUpLn9B9eDiEKjJaytNbuVWojpeDGTjM5pT4Ses1KvYEFcZJKACp7W+jxNVaCA2H8
AJ2dlrhjoQAAA8hDQKQaQ0CkGgAAAAdzc2gtcnNhAAABAQC4GGdMLT7ycmeNKlkSHvAGwG
ISmILdG1l92dej8mY1hBfhRcfW0Uk4WNAXOOUzc6J9d9/2mk1jAZzvUQz8OdMGR9lCyMRf
EPdAZAavRiOJXJYs/KL6cJ93szek0LvlG9S201g9hjmgslCqbkB/egLJjk+SjQEn23SY3/
Hs2zvpGYPPidRzEvN4DsYcY9eRsH9XAiQCJ16/6YWF/AZgGVkh2R7vH6MuiBtYNXbra9kM
gBhXXcAyjm7K23o7DocGxe7uz0xBlljut2ZSkuf0H14OIQqMlrK01u5VaiOl4MZOMzmlPh
J6zUq9gQVxkkoAKntb6PE1VoIDYfwAnZ2WuGOhAAAAAwEAAQAAAQBzUx5K00FOoiqKfU/l
ESpuIFCPs6ivGHX8Z941nyE2PzSyc4NX6C2FNeXN1l+G1tag4NqVYl4+OoF0TgLjctnmYl
YRBzI1F6y8Uqz5WefjIfQV5IG4f5r2YnfmMLi0MrYTfdwWVqJ9L5dm3MBc2zMpzpO8i8aA
kHK/XfLw3Pnv8HLgbfmxRDVfMJ46UtsMuTtHcFQdXpQh9JpOlbG+xvCKfCSN+W/SoaSGQo
1Bt96/MSPPausBnSkcyk4LaeHDO3h2TjVfxCd6fTN0JqgMQ4vvHkiz7UPhx6T0ofkDm+gc
hbZ8RDOY7msYQcdYziwXRozkWmc/u3fhw37Orji6SzgBAAAAgBurWQGzpqnHSTDbvWOEkF
LLW3m87GY6MwZFbGnDR2T5sH5nLsVsAgV7D2JwAigM5lGf245E5zyOUSo5QGaVg67mu4Fd
j05zDi7FESnADqZPCwyH4UrU0jFTTsbgWlo++uEH9ghlYkOodoCBeiG7t7+B1j9dyBWMVJ
XsV1VmYJSLAAAAgQDc6HENFCofL+9ZI02ATx0z9I4yfEE8f4l4azGVa18ziRFsuH//vzOO
ZNKUcHmnD5qWSOWzl7UMHfcn2cdv75Oac2CJEAg/lIEtPcTwDngHiESZtqiwOcInwxH1iN
d4trHNnyvtFoaPWJR0RQ5gkOQrPMd/ZqXpTugkS2pjqNcNwQAAAIEA1Vbra7Tys8xfUZFz
vZtHxp6cDZ9MV/YH0RLvGqjPueAPerqUgMVnGa/6yRABfPauLhqfqs2q8eMjcfb5hnZ8lB
YGsxf0dDAMkeeAsKmtMroNGqDHODfnBVyemBH+YuvBR7IS64zOpEGU9DpeDnoqBXOezmkW
+VXuLOvsScuijeEAAAAQdGVzdEBleGFtcGxlLmNvbQECAw==
-----END OPENSSH PRIVATE KEY-----
" > "$1"
  chmod 600 "$1"
}
