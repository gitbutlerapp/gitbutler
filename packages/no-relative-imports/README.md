# @gitbutler/no-relative-imports

An eslint rule for enforcing non-relative imports if there are `paths` in your `tsconfig.json` available to you.

## What the rule effects

The rule requires that any import that _could_ be referenced absolutly via an entry in `paths` will provide an error.

Even if a sibling import (IE `./foo`) is used, if it could be accessed via a path (IE `$lib/foo`). An error will be provided.

## The `paths` format

It should be noted that the current implementation does not try to handle all the possible `paths` entries. Instead it handles the two common cases:

```json
{
	"compilerOptions": {
		"paths": {
			"$lib": "./lib", // Without glob. IE pointing directly to a file
			"$lib/*": "./lib/*" // With glob, IE anything starting in $lib/
		}
	}
}
```
