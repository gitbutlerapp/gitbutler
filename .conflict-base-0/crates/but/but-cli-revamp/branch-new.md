```
but branch new <name>?
but branch new -A/--above <commit or branch>
but branch new -B/--below <commit or branch>
but branch new --format human/json/shell
```

Example baseline
```
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
```

# Above/below branches
but branch new
```
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
┊╭┄dp [canned]
├╯
```

but branch new foo
```
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
┊╭┄dp [foo]
├╯
```

but branch new --above bottom
```
┊╭┄op [top]
┊●   20e235d six
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊●   58410cd four
┊●   5a08eb5 three
┊│
┊├┄mi [canned]
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
```

but branch new --below bottom
```
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
┊│
┊├┄bo [canned]
├╯
```

but branch new --above top
```
┊╭┄op [canned]
┊│
┊├┄op [top]
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
```

# Above/below commits
but branch new --above 5a08eb5
```
┊╭┄op [top]
┊●   20e235d six
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊●   58410cd four
┊│
┊├┄mi [canned]
┊●   5a08eb5 three
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
```

but branch new --below 5a08eb5
```
┊╭┄op [top]
┊●   20e235d six
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊●   58410cd four
┊●   5a08eb5 three
┊│
┊├┄mi [canned]
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
```

but branch new --above 58410cd
```
┊╭┄op [top]
┊●   20e235d six
┊●   e28a167 five
┊│
┊├┄mi [middle]
┊│
┊├┄mi [canned]
┊●   58410cd four
┊●   5a08eb5 three
┊│
┊├┄bo [bottom]
┊●   87a11e5 two
┊●   28e6961 one
├╯
```
