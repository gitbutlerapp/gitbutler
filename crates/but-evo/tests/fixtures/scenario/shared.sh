set -eu -o pipefail

function with-header() {
  local name="${1:?}" value="${2:?}"
  git cat-file commit HEAD \
    | sed "/^committer /a\\
$name $value" \
    | git hash-object -t commit -w --stdin
}
