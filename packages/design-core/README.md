# @gitbutler/design-core

Design tokens and fonts for GitButler applications. This package provides CSS custom properties, design tokens in JSON format, and web fonts used across GitButler products.

## Installation

```bash
npm install @gitbutler/design-core
```

## Usage

### Single Import

The easiest way to get everything — fonts, CSS reset, text utilities, and design tokens:

```css
@import "@gitbutler/design-core/core";
```

Or in JavaScript/TypeScript:

```javascript
import "@gitbutler/design-core/core";
```

### Design Tokens (CSS)

Import only the design tokens CSS file:

```css
@import "@gitbutler/design-core/tokens";
```

Or in JavaScript/TypeScript:

```javascript
import "@gitbutler/design-core/tokens";
```

This provides CSS custom properties for colors, spacing, typography, and other design tokens with automatic light/dark mode support.

### Fonts

Import all font declarations:

```css
@import "@gitbutler/design-core/fonts";
```

Or reference individual font files directly:

```css
@import "@gitbutler/design-core/fonts/inter/Inter-Variable.woff2";
@import "@gitbutler/design-core/fonts/geist-mono/GeistMono-Variable.woff2";
@import "@gitbutler/design-core/fonts/but-head/But-Head-Regular.woff2";
```

### Styles

Import individual style sheets:

```css
@import "@gitbutler/design-core/styles/reset"; /* CSS reset */
@import "@gitbutler/design-core/styles/text"; /* Text utilities */
```

#### CSS Reset

The reset is scoped to `@layer reset` to avoid specificity conflicts with application styles. It applies the following normalizations:

- **Box sizing** — `box-sizing: border-box` on all elements and pseudo-elements.
- **Font rendering** — `-webkit-font-smoothing: antialiased`, `-moz-osx-font-smoothing: grayscale`, `text-rendering: optimizeLegibility`, and font size inflation prevention on `html`.
- **Body defaults** — zero padding and margin, `overflow-x: hidden`, and base values for `color`, `font-family`, `font-weight`, and `line-height` pulled from design tokens.
- **Form elements** — `input`, `textarea`, `select`, and `button` have their padding, border, and background stripped, and inherit `color` and `font` from the parent.
- **Headings** — `h1`–`h6` have `margin-block` removed.
- **Code & pre** — `code` and `pre` use `var(--fontfamily-mono)`.
- **Lists & paragraphs** — `ul` and `ol` have `padding-left` and `list-style` removed; `ul`, `pre`, and `p` have `margin-block` removed.

#### Text Utilities

Text classes are scoped to `@layer text`. The layer also sets three font-family custom properties on `:root`:

| Property              | Value                                 |
| --------------------- | ------------------------------------- |
| `--fontfamily-base`   | `var(--font-family-base), sans-serif` |
| `--fontfamily-mono`   | `var(--font-family-mono), monospace`  |
| `--fontfamily-accent` | `var(--font-family-accent), serif`    |

**Size classes** set `font-size`, `font-weight: regular`, `line-height`, and `font-family: base` as a self-contained unit:

| Class               | Font size                                          |
| ------------------- | -------------------------------------------------- |
| `.text-11`          | 11px (0.6875rem)                                   |
| `.text-12`          | 12px (0.75rem)                                     |
| `.text-13`          | 13px (0.8125rem)                                   |
| `.text-14`          | 14px (0.875rem)                                    |
| `.text-15`          | 15px (0.938rem)                                    |
| `.text-16`          | 16px (1rem)                                        |
| `.text-accent-head` | 62px (3.875rem) — display / brand heading          |
| `.text-h1`          | 20px (1.25rem), semibold, 130% — markdown heading  |
| `.text-h2`          | 18px (1.125rem), semibold, 130% — markdown heading |
| `.text-h3`          | 16px (1rem), semibold, 130% — markdown heading     |

**Modifier classes** are designed to be composed on top of a size class:

| Class             | Effect                                                                                     |
| ----------------- | ------------------------------------------------------------------------------------------ |
| `.text-semibold`  | `font-weight: var(--font-weight-semibold)`                                                 |
| `.text-bold`      | `font-weight: var(--font-weight-bold)`                                                     |
| `.text-italic`    | `font-style: italic`                                                                       |
| `.text-monospace` | Switches to `var(--fontfamily-mono)`                                                       |
| `.text-body`      | Switches to the looser `var(--font-line-height-body)` line height                          |
| `.text-balance`   | `text-wrap: balance` — ideal for short headings                                            |
| `.text-pre`       | `white-space: pre-wrap` + `word-break: break-word` — preserves whitespace without overflow |

Example usage:

```html
<p class="text-13 text-semibold">Label</p>
<code class="text-12 text-monospace">git commit</code>
<h1 class="text-accent-head text-italic">GitButler</h1>
```

## Available Exports

- `@gitbutler/design-core/core` - Single import: fonts + reset + text utilities + tokens
- `@gitbutler/design-core/tokens` - CSS custom properties (design tokens)
- `@gitbutler/design-core/fonts` - All font CSS declarations
- `@gitbutler/design-core/fonts/*` - Individual font files
- `@gitbutler/design-core/styles/reset` - CSS reset
- `@gitbutler/design-core/styles/text` - Text utility classes

## Included Fonts

All fonts are variable fonts unless noted.

- **Inter** — Primary UI font (`Inter-Variable.woff2`, `Inter-Variable-Italic.woff2`)
- **Geist Mono** — Monospace font for code (`GeistMono-Variable.woff2`)
- **But Head** — Brand display font (`But-Head-Regular.woff2`, `But-Head-Italic.woff2`)

## Development

Design tokens live in Figma and flow into this repo through a fixed pipeline:

```
Figma variables  ──tokens-bruecke──▶  tokens/json/*.tokens.json  ──terrazzo──▶  tokens/tokens.css
```

Nothing in `tokens/` is edited by hand. `tokens/json/*.tokens.json` is generated by the
export, and `tokens/tokens.css` is generated from it by the build.

### Building

The repo pins pnpm (`packageManager`) and Node (`devEngines.runtime`) in `package.json`.
`pnpm install` downloads that Node, so no separate Node version manager is needed.

```bash
# Install dependencies and the pinned Node
pnpm install

# Build tokens.css from tokens/json
pnpm run build

# Rebuild on every token change
pnpm run dev:tokens
```

`pnpm run build` runs [`scripts/postprocess-light-dark.mjs`](./scripts/postprocess-light-dark.mjs), which:

1. runs Terrazzo (`tz build`) over `core.tokens.json` and `semantic.tokens.json` — see [`terrazzo.config.js`](./terrazzo.config.js) — to write `tokens/tokens.css`. A color that aliases another and adds its own opacity (Figma's "opacity at scale") isn't valid DTCG, and Terrazzo rejects it, so [`scripts/composed-colors.mjs`](./scripts/composed-colors.mjs) first lifts those out of copies in `.terrazzo/`, and they come back in step 2 as `color-mix(in srgb, var(--base) var(--opacity), transparent)`, keeping both links;
2. merges the `:root` and `:root.dark` blocks into single `light-dark(…)` declarations, so light and dark mode need no class switching, and adds the lifted colors;
3. appends box-shadow custom properties generated from `fx.tokens.json` by [`scripts/generate-shadow-vars.mjs`](./scripts/generate-shadow-vars.mjs).
4. writes each token's Figma description as a comment above its declaration, so `tokens.css` says when to use a token.

Aliases stay aliases: a variable that points at another in Figma comes out as `var(--target)`, for sizes as well as colors.

### Token files

| File                               | Source                                                                        | Consumed by                |
| ---------------------------------- | ----------------------------------------------------------------------------- | -------------------------- |
| `tokens/json/core.tokens.json`     | `core` variable collection — raw palette, scales                              | Terrazzo                   |
| `tokens/json/semantic.tokens.json` | `semantic` variable collection — aliases onto core, with `light`/`dark` modes | Terrazzo                   |
| `tokens/json/fx.tokens.json`       | Figma **effect styles** — shadows                                             | `generate-shadow-vars.mjs` |

### Pulling tokens from Figma

Exports use the [tokens-bruecke](https://github.com/tokens-bruecke/figma-plugin) CLI, the
same tool as the Figma plugin, in [DTCG 2025.10](https://www.designtokens.org/tr/2025.10/format/) format.
It has to be 3.12 or later: older versions skip colors that alias another with their own opacity.

Credentials go in a gitignored `.env` at the repo root:

```bash
FIGMA_API_KEY=figd_…      # personal access token, must include the file_variables:read scope
FIGMA_FILE_KEY=…          # the design tokens Figma file
```

```bash
set -a && . ./.env && set +a
npx tokens-bruecke \
  -a "$FIGMA_API_KEY" \
  -f "$FIGMA_FILE_KEY" \
  -c .claude/skills/release-tokens/figma-export.config.json \
  -o tokens/json \
  --split-by-collection
```

The config file pins the export settings the committed tokens were generated with — hex
colors, no scopes, no Figma metadata, effect styles as `fx`. Changing it rewrites every
token file, so leave it alone unless that is the intent.

Two gotchas:

- The Figma **variables REST API requires an Enterprise plan** and a token carrying the
  `file_variables:read` scope. Without it the export fails with `403`. Personal access
  tokens also expire every 90 days.
- Every export rewrites a `createdAt` stamp in each file, so a diff alone does not mean
  the tokens changed. Check with:

    ```bash
    git diff -U0 tokens/json | grep '^[+-]' | grep -v '^[+-][+-]' | grep -v createdAt
    ```

    No output means only timestamps moved — revert and skip the release.

### Releasing

Version bumps follow the impact on the generated CSS custom properties, since those are
the public API:

| Change                                           | Bump  |
| ------------------------------------------------ | ----- |
| Token values tweaked                             | patch |
| Tokens or collections added                      | minor |
| Tokens removed or renamed (CSS var names change) | major |

House style: the version bump lives in the **same commit** as the token changes, tagged
with a bare `X.Y.Z` annotated tag (no `v` prefix).

```bash
npm version <patch|minor|major> --no-git-tag-version
# commit package.json + tokens/ together
git tag -a "$(node -p "require('./package.json').version")" -m "…"

npm publish --dry-run
npm publish                  # prompts for a 2FA one-time code
git push origin main && git push --tags
```

`prepublishOnly` rebuilds the CSS, so a published package always matches its committed
JSON. Pushing `tokens/tokens.css` also redeploys the hue-dini site via
[`.github/workflows/deploy-hue-dini.yml`](./.github/workflows/deploy-hue-dini.yml).

The whole sequence — export, no-op check, build, bump, commit, tag — is automated as a
Claude Code skill in [`.claude/skills/release-tokens/`](./.claude/skills/release-tokens/).
Run it with `/release-tokens`; it stops before `npm publish`, which stays manual because
npm 2FA requires a one-time code.

## Color Tool

[hue-dini](./tools/hue-dini) is the tool used to preview and export the GitButler color palette. It applies a single shared luminance scale across all hues — gray, accent, and semantic — so shades at the same step feel visually equivalent regardless of hue.

**Live:** https://gitbutlerapp.github.io/design-core/

```bash
cd tools/hue-dini
pnpm install
pnpm dev
```

See [tools/hue-dini/README.md](./tools/hue-dini/README.md) for more details.
