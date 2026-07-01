```
but move <source>+
but move -b/--branch <branch>
but move -A/--above <commit or branch>
but move -B/--below <commit or branch>
but move <commit or branch>+ --unstack
but move -i/--interactive
but move --format human/json/shell
but move --abort-on-conflicts
```

`but move f3e56a9 --branch existing-branch` moves the commit to the top of the branch

`but move f3e56a9 --branch non-existing-branch` creates the branch and moves the commit there
`but move f3e56a9 --branch` creates the branch, with a canned name, and moves the commit there

`but move some-branch --unstack` to unstack a branch because you cannot target zz/common base. This solution isn't great but we couldn't think of something nicer.

The order of the sources dont matter. We'll do a sort internally

`but move b440dea --above 20e235d --interactive` will open the TUI where you can pick the hunks to move

# Questions

Q: Can you do `but move zz` to create a commit?
A: No. Move doesn't create new commits. The number of commits doesn't change when moving

Q: Can you do `but move f3e56a9 --target zz` to create a uncommit?
A: No. `--target` isn't a thing and move doesn't uncommit/remove commits. The number of commits doesn't change when moving

Q: Can you move all the commits on a branch, without moving the branch itself?
A: Not for now. You can specify multiple sources so maybe we'll have ranges in the future or something specific to moving all commits on a branch

Q: Can you specify commit ranges?
A: Not for now.

Q: Can you do `but move f3e56a9 --after top --branch bar` to create a new branch?
A: Yes!

Q: Can you mix sources like `but move f3e56a9 middle --after top`?
A: No

Q: Can you do `but move top --below b440dea`?
A: No. The available targets for each source is
  - Branch: Branches
  - Commit: Branches + commits
  - File: Branches + commits
  - Hunk: Branches + commits

Q: Can you do `but move top -b`
A: No. You cannot use `-b` when the source is a branch. Use `--unstack`

Q: Moving commits to new unstacked branches
A: `but move b440dea -b`

Q: Can you specify both --above and --below
A: Not for now but someday maybe

# Examples
Examples below assume this baseline
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```

## Moving commits
but move 0db9c35 --branch top
```
┊╭┄h0 [top]
┊●   0db9c35 three <- to here
┊●   f3e56a9 six
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   0db9c35 three <- from here
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```

but move 9cd3abf --above 0db9c35
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   9cd3abf two <- to here
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two <- from here
┊●   58f9cc1 one
├╯
```

but move ed663c1 --above middle
but move ed663c1 --above middle -b (isn't necessary, it means the same thing)
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊│
┊├┄i0 [canned]
┊●   ed663c1 five <- to here
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   0db9c35 three
┊●   ed663c1 five <- from here
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```

but move ed663c1 --below middle
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   0db9c35 three
┊●   ed663c1 five <- to here
┊│
┊├┄i0 [canned]
┊●   ed663c1 five <- from here
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```

but move 58f9cc1 9cd3abf --above 0db9c35
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   9cd3abf two <- to here
┊●   58f9cc1 one <- to here
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two <- from here
┊●   58f9cc1 one <- from here
├╯
```

but move f3e56a9 --unstack
but move f3e56a9 --unstack -b foo
```
┊╭┄h0 [top]
┊●   f3e56a9 six <- from here
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
┊
┊╭┄fo [canned]
┊●   f3e56a9 six  <- to here
├╯
```

## Moving branches
but move middle --above top
```
┊╭┄i0 [middle]     <- to here
┊●   b440dea four  <- to here
┊●   0db9c35 three <- to here
┊│
┊├┄h0 [top]
┊●   f3e56a9 six
┊●   ed663c1 five
┊│
┊├ <- from here
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```

but move middle --unstack
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊●   ed663c1 five
┊│
┊├ <- from here
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
┊
┊╭┄fo [middle]     <- to here
┊●   b440dea four  <- to here
┊●   0db9c35 three <- to here
├╯
```

## Moving files
Baseline
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊│     f3:xv A six.txt
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```

but move f3:xv --above top
```
┊╭┄h0 [canned]
┊●   1dabfe5 (no commit message)
┊│     1d:xv A six.txt <- to here
┊│
┊├┄i0 [top]
┊●   f3e56a9 six
┊│     f3:xv A six.txt <- from here
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```

but move f3:xv --below b440dea
```
┊╭┄h0 [top]
┊●   f3e56a9 six
┊│     f3:xv A six.txt <- from here
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊●   1dabfe5 (no commit message)
┊│     1d:xv A six.txt <- to here
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```
