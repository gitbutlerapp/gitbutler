# Conflicts

**A conflict is warn, never danger.** Most tools stop at a conflict: the rebase
or merge halts until it is resolved. GitButler doesn't. A conflicted commit
sits in its stack while the user keeps working, so a conflict is work waiting on
them, not something that failed. Its mark is the `warning` glyph in warn amber
wherever it shows: a conflicted commit's glyph, a conflicted file's last cell, a
branch's `conflicts` count, the conflict banner, the status bar's count of what
needs resolving. Danger stays for what broke (checks failed, a fetch that
couldn't reach the remote) and for actions that destroy.

**The mark is 14px everywhere.** One size in every place it shows, in both
clients, so a conflict reads as the same thing from the sidebar to the file
list to the status bar. Never the 16px default, even in a cell whose other
glyphs are larger.

**What a conflict blocks says so at that action.** Only a few things wait on
it: pushing, taking changes out of the conflicted commit, and committing a file
that still has markers. Each says why where the user reaches for it, never as a
red banner over the surface: on a conflicted branch's row, Push gives way to the
plain ↑ count, which says why, and Resolve waits on the branch's page
([Blocked states](blocked-states.md)).
