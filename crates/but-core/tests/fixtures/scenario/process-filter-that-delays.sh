#!/usr/bin/env bash

set -eu -o pipefail

git init

# A long-running filter, like Git LFS. It passes content through, except that it delays
# `never.slow` whenever it is allowed to and then never delivers it. It logs every request.
cat >.git/filter.sh <<'EOF'
#!/usr/bin/env bash
set -eu -o pipefail

log="${GIT_DIR:-.git}/filter-requests"
read_packet() {
    local header
    IFS= read -r -d '' -n 4 header || return 1
    local size=$((16#$header - 4))
    packet=''
    if ((size > 0)); then
        IFS= read -r -d '' -n "$size" packet
    fi
}
read_list() {
    while read_packet && [[ -n "$packet" ]]; do :; done
}
write_packet() {
    printf '%04x%s' "$((${#1} + 4))" "$1"
}
read_list
write_packet $'git-filter-server\n'
write_packet $'version=2\n'
printf 0000
read_list
write_packet $'capability=clean\n'
write_packet $'capability=smudge\n'
write_packet $'capability=delay\n'
printf 0000
# Git ends key=value packets with a newline, gix doesn't.
while read_packet; do
    command="${packet%$'\n'}"
    metadata="$command"$'\n'
    can_delay=0
    pathname=''
    while read_packet && [[ -n "$packet" ]]; do
        metadata+="${packet%$'\n'}"$'\n'
        case "${packet%$'\n'}" in
        can-delay=1) can_delay=1 ;;
        pathname=*) pathname="${packet%$'\n'}" ;;
        esac
    done
    if [[ "$command" == command=list_available_blobs ]]; then
        printf 0000
        write_packet $'status=success\n'
        printf 0000
        continue
    fi
    printf '%s' "$metadata" >>"$log"
    content=''
    while read_packet && [[ -n "$packet" ]]; do
        content+="$packet"
    done
    if ((can_delay)) && [[ "$pathname" == pathname=never.slow ]]; then
        write_packet $'status=delayed\n'
        printf 0000
    else
        write_packet $'status=success\n'
        printf 0000
        if [[ -n "$content" ]]; then
            write_packet "$content"
        fi
        printf 0000
        printf 0000
    fi
done
EOF
git config filter.slow.process 'bash "${GIT_DIR:-.git}/filter.sh"'
git config filter.slow.required true

printf '*.slow filter=slow\n' >.gitattributes
printf 'v1\n' >a.slow
printf 'v1\n' >never.slow
git add .gitattributes a.slow never.slow
git commit -m "init"
