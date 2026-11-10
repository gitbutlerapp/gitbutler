# @gitbutler/but-ui

Design tokens and fonts for GitButler applications. This package provides CSS custom properties, design tokens in JSON format, and web fonts used across GitButler products.

It lived in [gitbutlerapp/design-core](https://github.com/gitbutlerapp/design-core) until 11.3.0, whose history stays there. It is no longer published to npm.

## Installation

Inside this monorepo, depend on the workspace copy:

```json
"@gitbutler/but-ui": "workspace:*"
```

Outside it, pin a commit of this repo, the same commit as `@gitbutler/but-ui-react` when you use both, so the components and the tokens they read always match:

```json
"@gitbutler/but-ui": "github:gitbutlerapp/gitbutler#<sha>&path:packages/but-ui",
"@gitbutler/but-ui-react": "github:gitbutlerapp/gitbutler#<sha>&path:packages/but-ui-react"
```

`tokens/tokens.css` and the fonts are committed, so nothing builds on install. Versions up to 11.3.0 are still on npm for projects that have not moved.

## Usage

### Single Import

The easiest way to get everything — fonts, CSS reset, text utilities, and design tokens:

```css
@import "@gitbutler/but-ui/core";
```

Or in JavaScript/TypeScript:

```javascript
import "@gitbutler/but-ui/core";
```

### Design Tokens (CSS)

Import only the design tokens CSS file:

```css
@import "@gitbutler/but-ui/tokens";
```

Or in JavaScript/TypeScript:

```javascript
import "@gitbutler/but-ui/tokens";
```

This provides CSS custom properties for colors, spacing, typography, and other design tokens with automatic light/dark mode support.

### Names

Every token is `--but-*`: roles for colour (`--but-background`, `--but-primary`, `--but-primary-soft-foreground`) and the scales for the rest (`--but-radius-control`, `--but-space-lg`). The CSS name is the Figma variable's path with dashes, so `primary/soft/foreground` in 🍑 But UI is `--but-primary-soft-foreground`. The ramps, `--but-<hue>-<step>` (`--but-teal-9`), are the steps the roles are built from; components read the roles, not the steps.

### Accent

Teal is the accent. A person can pick another by setting `data-but-accent` on the root element, `<html data-but-accent="blue">` (or `purple`): the `--but-primary` family then points at that hue's steps, and every component follows. It has to be the root, where the tokens are declared, or the tints made from `--but-primary` stay teal. Figma keeps teal; the other accents are generated in the build and held to the same contrast as teal.

Any colour works too: `<html data-but-accent="custom" style="--but-accent-color: #ee5858">`. The family takes the colour's hue and chroma at teal's lightness, a little further from the ground and with the chroma capped, so it reads wherever teal does; the build's contrast check runs it with a full-strength colour every 15° around the wheel. So the accent drawn is the tokens' take on the colour, not the colour itself: `#ee5858` comes out a deeper red. It needs relative colour syntax (Chrome 119, Safari 18, Firefox 128).

### Corners

A person can pick sharper or rounder corners with `data-but-radius` on the root element: `<html data-but-radius="sharp">` (or `round`); without it, the corners are soft. The whole radius scale moves 4px, so a control 4px inside a popup keeps its corners concentric; a step no bigger than that, as a checkbox's, moves half its size instead, so sharp never squares it. `--but-radius-full` never changes. The role tokens follow, as they alias the steps, as long as the attribute is on the root. Figma keeps the soft scale; the other two are generated in the build.

| Setting | control | card | popup | section | xs  |
| ------- | ------- | ---- | ----- | ------- | --- |
| sharp   | 2       | 4    | 6     | 10      | 1   |
| soft    | 6       | 8    | 10    | 14      | 2   |
| round   | 10      | 12   | 14    | 18      | 3   |

### Density

A person can pick a denser or roomier screen with `data-but-density` on the root element: `<html data-but-density="compact">` (or `spacious`); without it, the screen is balanced. Heights move most, with the padding beside a control's text following by a smaller step, so a compact control is a little narrower as well as shorter and a spacious one doesn't read as tall and thin; type, icons and corners stay put, so a denser screen holds more lines rather than smaller ones. A height of 28px or more moves 4px and a smaller one 2px, so a small button keeps room for its line. Side padding moves 2px through `--but-density-padding` (0 by default, −2px compact, +2px spacious), which buttons, segments, fields, tabs and popup rows add to their own padding, so a row and the heading over it stay aligned. `--but-size-tag` follows `--but-size-control-sm`. Figma keeps the balanced heights; the other two are generated in the build.

| Setting  | control-sm | control | bar | row | header |
| -------- | ---------- | ------- | --- | --- | ------ |
| compact  | 18         | 24      | 32  | 36  | 40     |
| balanced | 20         | 28      | 36  | 40  | 44     |
| spacious | 22         | 32      | 40  | 44  | 48     |

A component follows only if it takes its height from these tokens and adds `--but-density-padding` to the padding beside its text, so a height that sizes a control, a row or a bar is one of them, never a number.

### Presets

A preset is a named pair of the two settings above, so a person can pick a look in one step. [`presets.json`](./presets.json), exported as `@gitbutler/but-ui/presets`, lists them:

| Preset    | Corners | Density  |
| --------- | ------- | -------- |
| GitButler | soft    | balanced |
| Tight     | sharp   | compact  |
| Bubble    | round   | spacious |

A preset sets shape only: colour and theme stay as the person picked them. It has no attribute or CSS of its own; an app applies one by setting `data-but-radius` and `data-but-density` to its values. `soft` and `balanced` are Figma's, which no block overrides, so setting them works as well as leaving the attribute off. When a person changes either setting by hand, the look is no longer a preset. `scripts/check-presets.mjs`, run by the build, fails when a preset names a setting the build doesn't define.

### Settings as data

Everything a person can pick is also in [`tokens/settings.json`](./tokens/settings.json), exported as `@gitbutler/but-ui/settings`, for whatever can't read CSS: a native client, a settings panel listing the choices. Each of `accent`, `radius` and `density` has its `attribute`, its `default` (Figma's) and `values`: every choice, the default included, as the tokens it sets. Token names are `tokens.css`'s without `--but-`, lengths are px, and an accent's values name the ramp step each token points at. `accent.custom` is any colour: each token is that colour's hue, its chroma capped at `chromaCap` and taken at the `chroma` share, at the `lightness` given, in OKLCH, for light and for dark.

The build works the settings out once, writes this file, and writes the `:root[data-but-…]` blocks of `tokens.css` from the same objects, so the two can't differ. `pnpm check` fails when either is stale.

### Tokens as data

The tokens themselves are in [`tokens/tokens.json`](./tokens/tokens.json), for the same readers. `colors` has what each colour token comes to in light and in dark, as sRGB hex with its alpha last when it has one, and `lengths` has each length in px, both named as in `tokens.css` without `--but-`. These are Figma's values, so a client applies what a setting changes, from `settings.json`, on top. Fonts, shadows, easings and opacities stay in `tokens.css` alone, as they're written for CSS.

The GPUI client compiles the colours in as constants (`crates/but-gpui/build.rs`), so its theme draws in the colours the web does, and a token it reads that goes fails its build. It compiles the accents in from `settings.json` too, and draws the any-colour one with the same formula; [`tokens/accent-samples.json`](./tokens/accent-samples.json) holds what 25 picks come to in a browser, which its test checks it against.

`token-map.json` says where each name before but-ui went, and `scripts/rename-tokens.mjs` moves a folder onto the new names:

```bash
node packages/but-ui/scripts/rename-tokens.mjs path/to/src
```

### Fonts

Import all font declarations:

```css
@import "@gitbutler/but-ui/fonts";
```

Or reference individual font files directly:

```css
@import "@gitbutler/but-ui/fonts/inter/Inter-Variable.woff2";
@import "@gitbutler/but-ui/fonts/geist-mono/GeistMono-Variable.woff2";
@import "@gitbutler/but-ui/fonts/but-head/But-Head-Regular.woff2";
```

### Styles

Import individual style sheets:

```css
@import "@gitbutler/but-ui/styles/reset"; /* CSS reset */
@import "@gitbutler/but-ui/styles/text"; /* Text utilities */
```

#### CSS Reset

The reset is scoped to `@layer reset` to avoid specificity conflicts with application styles. It applies the following normalizations:

- **Box sizing** — `box-sizing: border-box` on all elements and pseudo-elements.
- **Font rendering** — `-webkit-font-smoothing: antialiased`, `-moz-osx-font-smoothing: grayscale`, `text-rendering: optimizeLegibility`, and font size inflation prevention on `html`.
- **Body defaults** — zero padding and margin, `overflow-x: hidden`, and base values for `color`, `font-family`, `font-weight`, and `line-height` pulled from design tokens.
- **Form elements** — `input`, `textarea`, `select`, and `button` have their padding, border, and background stripped, and inherit `color` and `font` from the parent.
- **Headings** — `h1`–`h6` have `margin-block` removed.
- **Code & pre** — `code` and `pre` use `var(--but-font-family-mono)`.
- **Lists & paragraphs** — `ul` and `ol` have `padding-left` and `list-style` removed; `ul`, `pre`, and `p` have `margin-block` removed.

#### Text Utilities

Text classes are scoped to `@layer text`. They read the font-family tokens, which carry their generic fallback (`--but-font-family-base` is `"Inter", sans-serif`; mono ends in `monospace`, accent in `serif`), so the text shows in the right kind of font while the font file loads.

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
| `.text-prose`       | 14px (0.875rem), regular, 160% — reading text      |

**Modifier classes** are designed to be composed on top of a size class:

| Class             | Effect                                                                                     |
| ----------------- | ------------------------------------------------------------------------------------------ |
| `.text-semibold`  | `font-weight: var(--but-font-weight-semibold)`                                             |
| `.text-bold`      | `font-weight: var(--but-font-weight-bold)`                                                 |
| `.text-italic`    | `font-style: italic`                                                                       |
| `.text-monospace` | Switches to `var(--but-font-family-mono)`                                                  |
| `.text-body`      | Switches to the looser `var(--but-font-line-height-body)` line height                      |
| `.text-balance`   | `text-wrap: balance` — ideal for short headings                                            |
| `.text-pre`       | `white-space: pre-wrap` + `word-break: break-word` — preserves whitespace without overflow |

Example usage:

```html
<p class="text-13 text-semibold">Label</p>
<code class="text-12 text-monospace">git commit</code>
<h1 class="text-accent-head text-italic">GitButler</h1>
```

## Available Exports

- `@gitbutler/but-ui/core` - Single import: fonts + reset + text utilities + tokens
- `@gitbutler/but-ui/tokens` - CSS custom properties (design tokens)
- `@gitbutler/but-ui/fonts` - All font CSS declarations
- `@gitbutler/but-ui/fonts/*` - Individual font files
- `@gitbutler/but-ui/styles/reset` - CSS reset
- `@gitbutler/but-ui/styles/text` - Text utility classes
- `@gitbutler/but-ui/presets` - The style presets, as JSON
- `@gitbutler/but-ui/settings` - Every accent, corner and density setting, as JSON

## Included Fonts

All fonts are variable fonts unless noted.

- **Inter** — Primary UI font (`Inter-Variable.woff2`, `Inter-Variable-Italic.woff2`)
- **Geist Mono** — Monospace font for code (`GeistMono-Variable.woff2`)
- **But Head** — Brand display font (`But-Head-Regular.woff2`, `But-Head-Italic.woff2`)

## Development

Design tokens live in Figma and flow into this package through a fixed pipeline:

```
Figma variables  ──tokens-bruecke──▶  tokens/json/*.tokens.json  ──terrazzo──▶  tokens/tokens.css
```

Nothing in `tokens/` is edited by hand. `tokens/json/*.tokens.json` is generated by the
export, and `tokens/tokens.css` is generated from it by the build.

### Building

Run these from `packages/but-ui`, after a `pnpm install` at the repo root:

```bash
# Build tokens.css from tokens/json
pnpm build

# Rebuild on every token change
pnpm dev:tokens

# Build, and fail if the committed tokens.css is stale (CI runs this through `pnpm check`)
pnpm check
```

`pnpm run build` runs [`scripts/postprocess-light-dark.mjs`](./scripts/postprocess-light-dark.mjs), which:

1. runs Terrazzo (`tz build`) over `core`, `semantic` and `ramps.tokens.json` — see [`terrazzo.config.js`](./terrazzo.config.js) — to write `tokens/tokens.css`. A color that aliases another and adds its own opacity (Figma's "opacity at scale") isn't valid DTCG, and Terrazzo rejects it, so [`scripts/composed-colors.mjs`](./scripts/composed-colors.mjs) first lifts those out of copies in `.terrazzo/`, and they come back in step 2 as `color-mix(in srgb, var(--base) var(--opacity), transparent)`, keeping both links;
2. merges the `:root` and `:root.dark` blocks into single `light-dark(…)` declarations, so light and dark mode need no class switching, and adds the lifted colors;
3. appends box-shadow custom properties generated from `fx.tokens.json` by [`scripts/generate-shadow-vars.mjs`](./scripts/generate-shadow-vars.mjs);
4. writes each token's Figma description as a comment above its declaration, so `tokens.css` says when to use a token;
5. works out what a person can pick, as data: each accent in [`scripts/generate-accents.mjs`](./scripts/generate-accents.mjs), the `--but-primary` family pointed at that hue or drawn from any colour; each corner setting in [`scripts/generate-radius.mjs`](./scripts/generate-radius.mjs), the radius scale moved sharper or rounder; and each density in [`scripts/generate-density.mjs`](./scripts/generate-density.mjs), the control, row and bar heights moved shorter or taller, and the density padding step;
6. writes that data to `tokens/settings.json`, and appends it to `tokens.css` as an `:root[data-but-accent=…]`, `[data-but-radius=…]` or `[data-but-density=…]` block for each choice but Figma's;
7. writes `tokens/tokens.json` in [`scripts/tokens-data.mjs`](./scripts/tokens-data.mjs): what each colour token comes to in light and in dark, and each length in px;
8. writes `tokens/accent-samples.json` in [`scripts/accent-samples.mjs`](./scripts/accent-samples.mjs): the any-colour accent for 25 picks, as a browser draws it.

Aliases stay aliases: a variable that points at another in Figma comes out as `var(--target)`, for sizes as well as colors.

### Token files

| File                               | Source                                                                         | Consumed by                                      |
| ---------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------ |
| `tokens/json/core.tokens.json`     | `core` variable collection — sizes, spacing, radii, type, motion               | Terrazzo                                         |
| `tokens/json/semantic.tokens.json` | `semantic` variable collection — aliases onto ramps, with `light`/`dark` modes | Terrazzo                                         |
| `tokens/json/ramps.tokens.json`    | `ramps` variable collection — twelve steps per hue, with `light`/`dark` modes  | Terrazzo                                         |
| `tokens/json/fx.tokens.json`       | Figma **effect styles** — shadows                                              | `generate-shadow-vars.mjs`                       |
| `tokens/settings.json`             | The build — every accent, corner and density setting, as data                  | Clients that can't read CSS                      |
| `tokens/tokens.json`               | The build — each colour for light and dark, each length in px                  | The GPUI client, and clients that can't read CSS |
| `tokens/accent-samples.json`       | The build — the any-colour accent for 25 picks, as a browser draws it          | GPUI's accent test                               |

### Pulling tokens from Figma

Exports use the [tokens-bruecke](https://github.com/tokens-bruecke/figma-plugin) CLI, the
same tool as the Figma plugin, in [DTCG 2025.10](https://www.designtokens.org/tr/2025.10/format/) format.
It has to be 3.12 or later: older versions skip colors that alias another with their own opacity.

Credentials go in a gitignored `.env` in `packages/but-ui`:

```bash
FIGMA_API_KEY=figd_…      # personal access token, must include the file_variables:read scope
FIGMA_FILE_KEY=…          # the design tokens Figma file
```

```bash
set -a && . ./.env && set +a
npx tokens-bruecke \
  -a "$FIGMA_API_KEY" \
  -f "$FIGMA_FILE_KEY" \
  -c figma-export.config.json \
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

  No output means only timestamps moved: revert, there is nothing to change.

The plugin can also push the export from Figma as a pull request, with its **GitHub PR**
server. Point it at `gitbutlerapp/gitbutler`, base branch `master`, a branch of its own such
as `tokens-bruecke/update-tokens`, and file name `packages/but-ui/tokens/json` with
_Split collections into separate files_ on, plus the settings in
[`figma-export.config.json`](./figma-export.config.json). The PR carries only the JSON, so
check out its branch, run `pnpm build` and commit `tokens/tokens.css` before it can pass.

### Changing tokens

A token change is one pull request in this repo, made the usual way: export, `pnpm build`,
commit `tokens/json` and `tokens/tokens.css` together. In the same PR:

- CI's `pnpm check` rebuilds `tokens.css` and fails if the committed one is stale, and [`scripts/check-contrast.mjs`](./scripts/check-contrast.mjs) fails when a pair meant to be read stops reading in light or dark: text on its grounds at 4.5:1, marks and faint details at 3:1.
- `@gitbutler/but-ui-react`'s `check:tokens` fails on any `var(--…)` a component still reads but
  no stylesheet declares any more, so fix those components in that PR.

Removing or renaming a variable breaks whatever reads it, here and in every project that
pins this package. Projects outside the monorepo pick a change up when they move their pin.

There is no version to bump: `version` stays at 11.3.0, the last npm release, so peer
ranges like `^11.3.0` keep resolving.

## Colour ramps

Each hue is a ramp of twelve steps, in light and dark, made by [`scripts/generate-ramps.mjs`](./scripts/generate-ramps.mjs) on Radix Colors' curves (through dittoTones) and nudged until text and labels read. It writes `tokens/ramps.json`, which goes into 🍑 But UI's `ramps` collection; Figma stays the source the tokens are exported from. `pnpm check` fails when `ramps.json` is older than the script.
