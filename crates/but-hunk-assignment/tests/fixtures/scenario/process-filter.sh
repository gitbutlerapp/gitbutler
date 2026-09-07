#!/usr/bin/env bash
set -eu -o pipefail

git init
printf 'original\n' > deleted.txt
printf 'staged.txt diff\n' > .gitattributes
git add deleted.txt .gitattributes
git commit -m initial
rm deleted.txt

# A staged addition must be read from the index, not its different worktree content.
printf 'staged\n' > staged.txt
git add staged.txt
printf 'worktree\n' > staged.txt
printf 'staged.txt -diff\n' > .gitattributes
printf 'binary\0content\n' > binary.bin

printf '*.filtered filter=counting\n' > .git/info/attributes
cat > .git/filter.sh <<'EOF'
#!/usr/bin/env bash
set -eu -o pipefail

# A minimal long-running clean filter speaking Git's packet-line protocol.
printf 'start\n' >> "$GIT_DIR/filter-starts"
read_packet() {
    local header
    IFS= read -r -N 4 header || return 1
    local size=$((16#$header - 4))
    packet=''
    if ((size > 0)); then
        IFS= read -r -N "$size" packet
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
printf 0000
while read_packet && [[ -n "$packet" ]]; do
    read_list
    content=''
    while read_packet && [[ -n "$packet" ]]; do
        content+="$packet"
    done
    if [[ "$content" == $'fail\n' ]]; then
        write_packet $'status=error\n'
        printf 0000
        continue
    fi
    write_packet $'status=success\n'
    printf 0000
    write_packet "cleaned $content"
    printf 00000000
done
EOF
git config filter.counting.process 'bash "$GIT_DIR/filter.sh"'
git config filter.counting.required true
printf 'first\n' > a.filtered
printf 'second\n' > b.filtered
