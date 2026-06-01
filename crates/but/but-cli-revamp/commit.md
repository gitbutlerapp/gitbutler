```
but commit
but commit <uncommitted file>*
but commit -i/--interactive
but commit --empty
but commit -b/--branch
but commit -A/--above <commit or branch>
but commit -B/--below <commit or branch>
but commit -m/--message <message> (can be repeated)
but commit --message-file <path>
but commit --no-message
but commit --format human/json/shell
but commit --abort-on-conflicts
```

We assume that we remove staging/assigments. The rational is that if deciding
what to commit is easy and commits are easy to mutate, then you don’t need
staging

We expect most people will just run `but commit` to commit everything to
whatever branch they're working on. With no branches `but commit` will also
create a branch and when run again will commit to that same branch, since there
is only one branch in the workspace.

`but commit -b foo` can also be repeated in the same way to keep committing to
the same branch.

# Questions

Q: Are commit messages required?
A: No and saving the editor without a commit message still creates the commit without a message

Q: Can you do `but commit --above middle -b`?
A: No. Mixing --above/--below with -b is not allowed

# What to commit
```
but commit
```
commit everything unassigned

```
but commit -i/--interactive
```
opens the tui where you can mark changes
confirming will exit the tui and commit

```
but commit cli-id-1 cli-id-2
```
for agents picking what to commit

# Where to commit
Examples below assume this baseline
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```

## Without above/below
```
but commit
```
Will try and figure out where to commit. If thats not obvious it'll show a picker

```
but commit -b
```
Commit to a new branch with a canned name

```
but commit -b new-branch
```
Commit to a new branch with a given name. Creates a new stack

```
but commit -b existing-branch
```
Commit to an existing branch

```
but commit --above foo
```
Creates a new branch above foo and commits there
Mixing --above/--below with -b is not allowed

```
but commit --below foo
```
Same idea as `--above foo`, i.e. creates a new branch below foo and commits there

```
but commit --above/--below f3e56a9
```
Will create a commit above/below f3e56a9 but wont create a new branch

## above/below branches
### top
#### above
but commit --above top
```
┊╭┄h0 [canned]
┊●   NEW COMMIT GOES HERE
┊│
┊├┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```

#### below
but commit --below top
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [canned]
┊●   NEW COMMIT GOES HERE
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```

### middle
#### above
but commit --above middle
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [canned]
┊●   NEW COMMIT GOES HERE
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```

#### below
but commit --below middle
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊│
┊├┄i0 [canned]
┊●   NEW COMMIT GOES HERE
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```

### bottom
#### above
but commit --above bottom
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊│
┊├┄i0 [canned]
┊●   NEW COMMIT GOES HERE
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```

#### below
but commit --below bottom
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
┊│
┊├┄i0 [canned]
┊●   NEW COMMIT GOES HERE
├╯
```

## above/below commits
### above
but commit --above b440dea
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [middle]
┊●   NEW COMMIT GOES HERE
┊●   b440dea add bar-1.txt
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```

### below
but commit --below b440dea
```
┊╭┄h0 [top]
┊●   f3e56a9 some commit (no changes)
┊│
┊├┄i0 [middle]
┊●   b440dea add bar-1.txt
┊●   NEW COMMIT GOES HERE
┊│
┊├┄fo [bottom]
┊●   9cd3abf add foo.txt
├╯
```
