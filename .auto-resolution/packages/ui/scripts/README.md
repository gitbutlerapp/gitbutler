# UI Package Scripts

## `optimize-ui-icons.js`

Optimizes all SVG files in `src/lib/icons/svg/` using svgo, then syncs `src/lib/icons/names.ts`.

### Usage

```bash
# From the UI package root
pnpm optimize-ui-icons
# or
node scripts/optimize-ui-icons.js
```

### What it does

1. Runs svgo on every `.svg` in `src/lib/icons/svg/` with `preset-default` (keeping `viewBox` and groups intact)
2. Sets `width="100%"` and `height="100%"` on every `<svg>` so icons scale via CSS
3. Replaces hardcoded `fill`/`stroke` colors with `currentColor` (leaves `none` untouched)
4. Adds `vector-effect="non-scaling-stroke"` to all vector shape elements
5. Updates `src/lib/icons/names.ts` to reflect the current set of icon files

---

## `optimize-file-icons.js`

Optimises SVG file-type icons and writes them into `src/lib/components/file/icon/svg/`.

Supports two modes:

- **Optimize in place** (no argument): optimises all existing SVGs in the output directory
- **Import from a source directory** (with argument): reads SVGs from `<svg-dir>`, optimises them, and writes each into the output directory (adding new files, updating changed ones)

### Optimisation steps

- Runs svgo `preset-default` (keeping `viewBox` and groups intact)
- Removes `width`/`height` attributes so icons scale via CSS
- Replaces hardcoded hex colours with CSS variables from a canonical colour map (`--file-icon-gray`, `--file-icon-green`, `--file-icon-teal`, `--file-icon-blue`, `--file-icon-dark-blue`, `--file-icon-yellow`, `--file-icon-orange`, `--file-icon-red`, `--file-icon-pink`, `--file-icon-purple`, `--file-icon-violet`)

### Usage

```bash
# Optimize in place
pnpm optimize-file-icons
# or
node scripts/optimize-file-icons.js

# Import + optimize from a source directory
node scripts/optimize-file-icons.js <svg-dir>
```
