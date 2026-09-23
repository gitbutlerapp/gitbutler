# @gitbutler/ui-react

The React component library: the components, icons, illustrations and base
styles shared by Lite, `but panel` and the MCP app. It ships source, not a
build; each consumer's Vite compiles it, and React Compiler runs on it under
the consumer's config as it does on the consumer's own code.

`DESIGN.md` beside this file is the visual language: how things should look
and read. Read it before changing anything a user sees. This file is the
tooling that enforces it.

## Writing the code

Lite's rules apply here unchanged; `apps/lite/AGENTS.md` has them. In short:
React Compiler makes `useMemo`, `useCallback` and `React.memo` redundant
outside hot paths; `useEffect` is usually an anti-pattern and needs consent;
comments explain the non-obvious why, never the what.

### What belongs here

A component belongs here when it doesn't reach into an app: no API calls,
no settings, no store, nothing on `window.lite`. Everything a component needs
from its host arrives as a prop, the way `TextLink` takes its `onClick` rather
than assuming Electron. A component that needs the app stays in the app;
Lite keeps its Markdown renderer and mention suggestions for that reason.

Imports are deep and carry the extension: `@gitbutler/ui-react/Button.tsx`.
There is no barrel file. Inside the package, siblings import each other
relatively.

### Styles

Tokens come from `@gitbutler/design-core`, imported by the host once.
`src/base.css` declares the few variables components read that aren't tokens
(`--focus-ring`, `--control-cursor`, `--list-item-hover-bg`,
`--transition-button`) and the control cursor rules; every host imports it
right after design-core. A variable nothing declares doesn't error, the
property quietly falls back, so a new shared variable goes in `base.css`, not
in a host's stylesheet.

### Icons

Icons are SVGs in `src/icons/`, inlined into the bundle as raw strings; the
filename is the icon name (`folder-lock.svg` → `<Icon name="folder-lock" />`).
To add one:

1. Export it from Figma at 16×16 (⚛️ Lite Core library) as SVG into `src/icons/`.
2. Run `pnpm -F @gitbutler/ui-react optimize-icons`. It minifies the SVG,
   recolors it to `currentColor` and regenerates `src/iconNames.ts`.
3. Commit both. `iconNames.ts` is generated; never hand-edit it.

`src/file-icons/` are the language and filetype glyphs. They keep their brand
colours and are deliberately not run through the script.

`src/program-icons/` are the marks of editors and terminals, as PNGs at 2×.
Their rounded corner is part of the image: export each from the "Programm"
frame on ⚛️ Lite Core's Icons page, where the mark is a 14px rectangle with
the corner on it, as PNG at 2× with the layer named as the file is
(`vscode` → `vscode@2x.png`). No CSS rounds them, so a square export shows
square.

## Figma

Two files, split the way the code is:

- **⚛️ Lite Core** —
  <https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core>,
  the published library: tokens (Tokens page), icons (Icons page) and this
  package's components (Components page), one section per component. It is
  the source of truth for tokens; for components, code is, and the drawing
  follows it.
- **Lite**, the working file —
  <https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Lite>: Lite's own
  components on the ⚙️ Meta page, the full screens, and drafts on the
  🚧 Drafts page and the pages under it. A new mockup or an exploration goes
  here, never in the library.

A component is drawn where its code lives: one in this package goes in Lite
Core, one in `apps/lite` on ⚙️ Meta. When a component moves between the two
in code, its drawing moves with it in the same piece of work, and the
mockups' instances are swapped to the new one.

The drawing mirrors the code's structure: the same component under the same
name, variant properties named as the props and their values (`status`,
`variant`, `glyph`), and no property the code lacks. Figma may add only what
the code produces from data, such as a toggle for a side the code drops when
its count is zero; two components the code keeps apart stay apart in Figma
and are placed side by side where a surface shows both.

## Verifying your work

Always run the specified commands **exactly** as written. The package has no
app to run, so the story is how a component is checked before it goes into a
surface.

### Typechecking, the CSS rules, and tests

```console
$ pnpm -F @gitbutler/ui-react check
$ pnpm -F @gitbutler/ui-react test
```

`check` typechecks and then runs Stylelint over the components' CSS with the
rules DESIGN.md gives a machine: colours are tokens, font sizes are on the
11–16 scale, radii come from the radius tokens. An error names what to use
instead. The exceptions are the ones DESIGN.md names, and each is marked
where it is: the file icons' brand colours (a file-level override in
`stylelint.config.mjs`) and a mask's opaque stop. A radius that is a share of
another, an inner corner following its outer one, is a `calc()` from the
outer token and needs no exception (DESIGN.md, Radius). A new exception is
a design decision first; if it stands, disable the rule on that line with the
reason after `--`.

### Tokens

Every `var(--x)` in this package has to resolve. Lite's token check reads this
package's source alongside its own:

```console
$ pnpm -F @gitbutler/lite check
```

It fails on any undefined name and suggests the nearest real one. Run it
after any CSS change here, even when the package's own check passes.

### Stories

Every component has a story beside it, in both themes, and a component that
lives only in code is half a component. Storybook is Lite's, on port 6007, and
lists this package's stories under `components/`:

```console
$ pnpm -F @gitbutler/lite demos
```

A story renders alone at
`http://localhost:6007/iframe.html?id=components-<name>--<export>&viewMode=story`;
append `&globals=theme:dark` for the dark theme. Check colour, font and
spacing from computed styles rather than by eye. A story links its Figma
component through the `design` parameter; keep the link when you add a story,
and compare the two sides when either changes.

### The manifest

Storybook writes `/manifests/components.json` (and `components.html`, to
read) from the stories: every component, its props from the TypeScript
types, its stories with their source, and the import to write. An agent
reads it instead of guessing an API. Two things about it to know:

- The import to write is the `@import` tag in each component's JSDoc, as
  `@import import { Badge } from "@gitbutler/ui-react/Badge.tsx";`. Give a
  new component one. The entry's `reactDocgenTypescript.tags.import` carries
  it; the top-level `import` field is Storybook's own guess, which names
  the package without the file, and there is no such barrel.
- The link to the Figma component is the story's `design` parameter, not
  the manifest. `grep -l 'type: "figma"' src/*.stories.tsx` lists the stories
  that have one; a story without one has no drawn spec yet.

A story whose subject is a function rather than a component, as `Button`'s
`getButtonClassName` is, carries `tags: ["!manifest"]` and says why.

### Linting & formatting

The package is linted by oxlint with Lite's config, not by ESLint:

```console
$ pnpm exec prettier --check packages/ui-react
$ pnpm run oxlint
$ pnpm knip:non-prod
```

Run these before declaring the work done, and run `check-tokens` (above) with
them. The lint job in CI runs all of them and fails on what any one reports.
