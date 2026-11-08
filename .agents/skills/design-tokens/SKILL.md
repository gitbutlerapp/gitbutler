---
name: design-tokens
description: "Use when pulling design tokens from Figma into `packages/but-ui`, regenerating `tokens.css`, or reviewing a token change: sync/export/update tokens, a Figma variable was added, renamed or changed, or `pnpm check` says tokens.css is stale. Not for using tokens in components; that is the gitbutler-ui skill."
---

# Design tokens

Figma variables are the source. `packages/but-ui/tokens/json/*.tokens.json` is
their export and `tokens/tokens.css` is built from it; neither is edited by hand. The
package README has the pipeline in full. Run everything below from `packages/but-ui`.

## 1. Export from Figma

Every route writes the same three files: `core`, `semantic` and `fx.tokens.json`.

- **A snapshot through the file API (the route that works on GitButler's plan).** Figma's
  variables REST API is Enterprise-only, so the CLI's own fetch answers `403`. Instead, dump
  the variables with the Figma MCP and convert them with the CLI's snapshot mode:
  1. With `use_figma`, collect `getLocalVariableCollectionsAsync()`,
     `getLocalVariablesAsync()` and `getLocalEffectStylesAsync()` as plain objects (the
     fields in tokens-bruecke's `schemas/tokens-snapshot.schema.json`), and write the JSON
     into a text layer on a new page named `tmp: tokens snapshot`. A `use_figma` result
     can't carry it: it caps at 48 KB.
  2. Fetch that layer with the regular file API, which any plan has:
     `GET https://api.figma.com/v1/files/$FIGMA_FILE_KEY/nodes?ids=<node id>` with the
     `X-Figma-Token` header from `.env`. The snapshot is the node's `characters`.
  3. Decode the HTML entities in descriptions (Figma returns `&#39;`; the plugin decodes
     them, snapshot mode doesn't), then delete the page.
  4. `npx tokens-bruecke@latest --input snapshot.json -c figma-export.config.json -o tokens/json --split-by-collection`

- **tokens-bruecke CLI, on an Enterprise plan.** Needs `FIGMA_API_KEY` (with
  `file_variables:read`; it expires every 90 days) and `FIGMA_FILE_KEY` in a `.env` here.

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

A token's CSS name is its Figma path with dashes under `--but-` (`primary/soft/foreground`
is `--but-primary-soft-foreground`); a group's own variable (`primary` beside
`primary/foreground`) takes the group's name. The raw palettes, `clr/*` and `scale/*`, keep
`--clr-*` and `--scale-*`. `cssVarName` in `scripts/terrazzo-css-helpers.mjs` is the one rule.

## 2. Check something changed

Every export moves a `createdAt` stamp, so a diff alone means nothing:

```bash
git diff -U0 tokens/json | grep '^[+-]' | grep -v '^[+-][+-]' | grep -v createdAt
```

No output: `git checkout -- tokens/json`, tell the user Figma has no changes, and stop.

## 3. Build and review

```bash
pnpm build
git show HEAD:packages/but-ui/tokens/tokens.css > "$TMPDIR/tokens-before.css"
node scripts/diff-css-vars.mjs "$TMPDIR/tokens-before.css" tokens/tokens.css
```

The report lists added, changed and removed variables. A token that changed in the JSON
but not in the CSS is probably in a collection Terrazzo doesn't read; say so rather than
patching the CSS.

## 4. Fix what reads a removed or renamed variable

```bash
pnpm --filter @gitbutler/but-ui-react check:tokens
```

It names every `var(--…)` nothing declares any more. Fix those in the same change, and
search the rest of the repo too: `packages/but-mcp-app`, the GPUI theme in
`crates/but-gpui/src/theme.rs`, which copies values by hand, and
`crates/but-gpui/src/ui/illustration.rs`, which colours but-ui-react's illustrations by the names
in their SVGs. For a rename, `scripts/rename-tokens.mjs` reads `token-map.json`; add the old
name and its replacement there and run it. Projects outside the monorepo
(but-dev) pin a commit and pick the change up when they move it; mention removals to the
user so they know.

## 5. Commit

`tokens/json` and `tokens/tokens.css` go in one commit, with the GitButler skill, and the
message says what changed in the tokens. There is no version bump or release: the package
is not published.
