```
but squash <source>+
but squash -t/--target <target>
but squash -m/--message <message> (can be repeated)
but squash --message-file <path>
but squash --no-message
but squash --use-source-message
but squash -u/--use-target-message
but squash -i/--interactive
but squash --format human/json/shell
but squash --abort-on-conflicts
```

# Questions

Q: `but squash branch`
A: Will squash all commits on the branch and keep the reference

Q: Can you squash a branch into another commit
A: Yes. That'll squash all the commits in the branch and remove the reference

Q: Can you squash into branches
A: No

Q: Does the editor open to edit the squashed messages
A: Yes

Q: Can you mix source types?
A: No but except you can mix files and hunks

Q: Does squash open the editor to edit the commit message

# Examples
Examples below assume this baseline
```
╭┄zz [uncommitted changes]
┊   ur M flake.nix
┊
┊╭┄op [top]
┊●   20e235d six
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊●   58410cd four
┊●   5a08eb5 three
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
┊
┴ ed7c1f1 (common base) 2026-06-01 add flake.lock
```

but squash e28a167 -t 20e235d
but squash top
```
╭┄zz [uncommitted changes]
┊   ur M flake.nix
┊
┊╭┄op [top]
┊●   21ec00c six+five
┊│
┊├┄mi [middle]
┊●   58410cd four
┊●   5a08eb5 three
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
┊
┴ ed7c1f1 (common base) 2026-06-01 add flake.lock
```

but squash 58410cd 5a08eb5 -t 20e235d
```
╭┄zz [uncommitted changes]
┊   ur M flake.nix
┊
┊╭┄op [top]
┊●   21ec00c six+four+three
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
┊
┴ ed7c1f1 (common base) 2026-06-01 add flake.lock
```

but squash middle -t e28a167
```
╭┄zz [uncommitted changes]
┊   ur M flake.nix
┊
┊╭┄op [top]
┊●   20e235d six
┊●   e28a167 five+four+three
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
┊
┴ ed7c1f1 (common base) 2026-06-01 add flake.lock
```

but squash 20e235d -t zz
```
╭┄zz [uncommitted changes]
┊   ur M flake.nix
┊   ab M six
┊
┊╭┄op [top]
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊●   58410cd four
┊●   5a08eb5 three
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
┊
┴ ed7c1f1 (common base) 2026-06-01 add flake.lock
```

but squash zz -t 20e235d
```
╭┄zz [uncommitted changes] (no changes)
┊
┊╭┄op [top]
┊●   20e235d six+flake.nix
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊●   58410cd four
┊●   5a08eb5 three
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
┊
┴ ed7c1f1 (common base) 2026-06-01 add flake.lock
```

but squash middle -t zz
```
╭┄zz [uncommitted changes]
┊   ur M flake.nix
┊   ur M four
┊   ur M three
┊
┊╭┄op [top]
┊●   20e235d six
┊●   e28a167 five
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
┊
┴ ed7c1f1 (common base) 2026-06-01 add flake.lock
```

## Squashing hunks
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

but squash f3:xv -t b440dea
```
┊╭┄h0 [top]
┊●   f3e56a9 six (no changes)
┊●   ed663c1 five
┊│
┊├┄i0 [middle]
┊●   b440dea four
┊│     b4:xv A six.txt
┊●   0db9c35 three
┊│
┊├┄fo [bottom]
┊●   9cd3abf two
┊●   58f9cc1 one
├╯
```
