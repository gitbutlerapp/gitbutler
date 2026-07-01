function add_change_id_to_given_commit() {
  local a="00000000-0000-0000-0000-000000000000"
  local b="${1:?first argument is the single-digit number of the change-id}"
  local change_id="${a:0:${#a}-${#b}}${b}"

   # Insert the GitButler header lines at the first blank line (before the commit message).
   git cat-file -p "${2:?second argument is the commit to add a changeid to}" \
   | awk -v cid="$change_id" '
     BEGIN { injected = 0 }
     /^$/ && !injected {
       print "gitbutler-headers-version 2"
       print "gitbutler-change-id " cid
       print ""
       injected = 1
       next
     }
     { print }
     ' \
   | git hash-object -wt commit --stdin
}
