---
name: design-tokens
description: "Use when pulling design tokens from Figma into `packages/design-core`, regenerating `tokens.css`, or reviewing a token change: sync/export/update tokens, a Figma variable was added, renamed or changed, or `pnpm check` says tokens.css is stale. Not for using tokens in components; that is the gitbutler-ui skill."
---

# Design tokens

Figma variables are the source. `packages/design-core/tokens/json/*.tokens.json` is
their export and `tokens/tokens.css` is built from it; neither is edited by hand. The
package README has the pipeline in full. Run everything below from `packages/design-core`.

## 1. Export from Figma

Every route writes the same three files: `core`, `semantic` and `fx.tokens.json`.

- **tokens-bruecke CLI.** Needs `FIGMA_API_KEY` (with `file_variables:read`, an Enterprise
  plan feature; it expires every 90 days) and `FIGMA_FILE_KEY` in a `.env` here. Use 3.12
  or later.

  ```bash
  set -a && . ./.env && set +a
  npx tokens-bruecke -a "$FIGMA_API_KEY" -f "$FIGMA_FILE_KEY" \
    -c figma-export.config.json -o tokens/json --split-by-collection
  ```

  A `403` with `Token expired` or `Invalid scope(s)` needs the user to make a new token.
  Stop and ask; never hand-edit the JSON instead.

- **Figma agent.** `/export-tokens` in the tokens file, choosing hex, everything, one file
  per collection; copy the three files into `tokens/json/`.

- **A PR from the plugin's GitHub PR server.** It carries only the JSON: check out its
  branch and continue from step 2.

`figma-export.config.json` holds the settings the committed tokens were made with. Changing
it rewrites every file, so don't, unless that is the point.

## 2. Check something changed

Every export moves a `createdAt` stamp, so a diff alone means nothing:

```bash
git diff -U0 tokens/json | grep '^[+-]' | grep -v '^[+-][+-]' | grep -v createdAt
```

No output: `git checkout -- tokens/json`, tell the user Figma has no changes, and stop.

## 3. Build and review

```bash
pnpm build
git show HEAD:packages/design-core/tokens/tokens.css > "$TMPDIR/tokens-before.css"
node scripts/diff-css-vars.mjs "$TMPDIR/tokens-before.css" tokens/tokens.css
```

The report lists added, changed and removed variables. A token that changed in the JSON
but not in the CSS is probably in a collection Terrazzo doesn't read; say so rather than
patching the CSS.

## 4. Fix what reads a removed or renamed variable

```bash
pnpm --filter @gitbutler/ui-react check:tokens
```

It names every `var(--…)` nothing declares any more. Fix those in the same change, and
search the rest of the repo too: `apps/`, `packages/but-mcp-app`, and the GPUI theme in
`crates/but-gpui/src/theme.rs`, which copies values by hand. Projects outside the monorepo
(but-dev) pin a commit and pick the change up when they move it; mention removals to the
user so they know.

## 5. Commit

`tokens/json` and `tokens/tokens.css` go in one commit, with the GitButler skill, and the
message says what changed in the tokens. There is no version bump or release: the package
is not published.
