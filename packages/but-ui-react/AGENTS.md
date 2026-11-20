# @gitbutler/but-ui-react

The React component library: the components, icons, illustrations and base
styles. It ships source, not a build; each consumer's Vite compiles it, and
React Compiler runs on it under the consumer's config as it does on the
consumer's own code.

The visual language, how things should look and read on every platform, is
`@gitbutler/but-ui`'s `DESIGN.md` (`packages/but-ui/DESIGN.md`): an index of
the pages in its `design/`, one per topic; read the ones for what you are
changing before changing anything a user sees. `DESIGN.md` beside this file
has only how to build with this library.

## Writing the code

React Compiler makes `useMemo`, `useCallback` and `React.memo` redundant,
except in hot paths where the compiler can't tell that a computation is pure;
check the compiler's output before adding one. Ask for consent before adding a
`useEffect`.

### Changing a component

**A change to a component here needs the designer's okay.** Every app built on
this library draws with it, so a fix made for one surface changes all of them.
When an app's bug seems to need a change here (a style, a prop, a default, a
wrapper), try fixing it in the app first. If the library really is at fault,
ask before changing it. When you can't ask, make the change its own
`fix(but-ui-react): …` commit that says which surface showed the bug and what
else it touches, and call it out in the PR description.

### What belongs here

A component belongs here when it doesn't reach into an app: no API calls,
no settings, no store, nothing on `window`. Everything a component needs
from its host arrives as a prop, the way `TextLink` takes its `onClick` rather
than assuming how its host opens a link. A component that needs the app stays
in the app.

Imports are deep and carry the extension: `@gitbutler/but-ui-react/Button.tsx`.
There is no barrel file. Inside the package, siblings import each other
relatively.

### Styles

Tokens come from `@gitbutler/but-ui`, imported by the host once.
`src/base.css` declares the few variables components read that aren't tokens
(`--but-focus-ring`, `--control-cursor`, `--transition-button`) and the control
cursor rules; every host imports it right after but-ui. A variable nothing declares doesn't error, the
property quietly falls back, so a new shared variable goes in `base.css`, not
in a host's stylesheet.

A component's own values that the GPUI app draws too (sizes, gaps, padding) are
but-ui component tokens, in `packages/but-ui/components/<component>.tokens.json`
(`--but-button-regular-gap`). Its CSS reads those, so change a value there, not
in the module. Button has them so far.

### Icons

Icons are SVGs in `src/icons/`, named as their component in Figma
(`folder-lock.svg` → `<Icon name="folder-lock" />`). To add one:

1. Export it at 16×16 from 🍑 But UI as SVG, ungrouped: each icon is one union
   shape, which exports as a filled outline that thickens as the icon grows,
   while its ungrouped paths export as strokes. Through the Figma MCP, clone the
   component's children into a 16×16 frame, move each union's children up to
   its parent, and `exportAsync({ format: "SVG_STRING" })` the frame. Flatten a
   stroked union (`pointer`, `key`) into one vector with no fill instead, and
   export a subtract (`theme-dark`) merged.
2. Run `pnpm -F @gitbutler/but-ui-react optimize-icons`, which recolours it to
   `currentColor` and regenerates `src/iconNames.ts`. The pantry picks up a new
   file only after a restart.
3. Commit both. The All icons story's Size range shows an export gone wrong: a
   filled outline thickens at 48px where its neighbours stay thin.

`src/file-icons/` keep their brand colours and skip the script.
`src/program-icons/` are PNGs at 2× from the "Programm" frame on 🍑 But UI's
Icons page, named as the layer (`vscode@2x.png`), with the rounded corner in
the image.

## Figma

One file, **🍑 But UI** —
<https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI>, holds
the library (💎 Tokens, 💠 Icons, and this package's components under 🎛️
Components, a page per group), the app's screens (📱 Panel, 🖼️ Preview, 🎴
Overlays, 🪟 Modals), app components (⚙️ Meta) and drafts (the 🚧 pages).

- Figma is the source of truth for tokens. For components, code is, and the
  drawing follows it: the same name, variant properties named as the props and
  their values, and no property the code lacks beyond a `state` for what CSS
  draws under the pointer.
- Anything an agent draws goes on a 🚧 page unless the designer names another
  place, built from the file's own components and variables.
- A component is drawn where its code lives: this package's on its group's
  page, an app's on ⚙️ Meta. When one moves in code, its drawing moves in the
  same piece of work.
- A component's Figma description is generated from its JSDoc, its `Variant`
  type and its `@figma` and `@see` tags by `scripts/figma-descriptions.mjs`
  into `figma-descriptions.json`. After changing any of those, run it and apply
  the file with the Figma MCP's `use_figma` (the REST API can't write
  descriptions). `check` fails when the file is out of date.

## Verifying your work

The package has no app to run, so the story is how a component is checked
before it goes into a surface.

### Typechecking, the CSS rules, and tests

```console
$ pnpm -F @gitbutler/but-ui-react check
$ pnpm -F @gitbutler/but-ui-react test
```

`check` typechecks, checks the stories and tokens (below), and runs Stylelint
over the components' CSS with the
rules the design pages give a machine: colours are tokens, font sizes are on
the 11–16 scale, radii come from the radius tokens. An error names what to use
instead. The exceptions are the ones the pages name, and each is marked where
it is: the file icons' brand colours (a file-level override in
`stylelint.config.mjs`) and a mask's opaque stop. A radius that is a share of
another, an inner corner following its outer one, is a `calc()` from the outer
token and needs no exception (`design/foundations/radius.md`). A new exception
is a design decision first; if it stands, disable the rule on that line with
the reason after `--`.

### Tokens

Every `var(--x)` in this package has to resolve: a token from
`@gitbutler/but-ui`, a variable `base.css` or a component declares, or
one a dependency sets. A name nothing declares doesn't error in the browser;
the property silently falls back. `check` runs `scripts/check-tokens.mjs`,
which fails on any undefined name and suggests the nearest real one. When a
dependency sets a variable at runtime that the script can't see, or a
component reads a knob its host may set, add it to `KNOWN_RUNTIME` in the
script with who sets it.

### The GPUI client's components

`crates/but-ui-gpui` draws some of these components for the GPUI client, with
the same variants and sizes. `check` runs `scripts/check-gpui-parity.mjs`,
whose table says how that crate draws each value of a component's variant
and size types, or `null` where it doesn't yet. A new value here fails until
the table says how GPUI draws it, and a new variant there fails until a value
here maps to it; the gaps are printed on every run.

### Design notes

A design note names a token, a component, a file or a prop rather than writing
out what it is, and `check` runs `scripts/check-notes.mjs`, which fails on any
of them that's gone: a rename that leaves a note pointing at nothing fails here,
not in an agent's build. It checks but-ui's notes and this library's, and the
links between them. The 🍑 But UI nodes a note names, as the toasts note's, are
checked only with `FIGMA_API_KEY` and `FIGMA_FILE_KEY` set, as when pulling
tokens.

### Stories and the pantry

Every component has a story beside it, in both themes; `check` fails on a
component no story imports. A stories file describes its page with `stories()`
from `stories/stories.ts`, and each story with `meta.story()`:

```tsx
const meta = stories({ title: "Content & status/Badge", id: "components-badge", component: Badge });
export const Default = meta.story({ args: { children: "42" } });
```

A story takes `args`, a `render(args)`, `decorators`, `parameters` (`design`,
the Figma link, and `layout`) and a `play` function. `title` names its Figma
group; keep `id` when it moves group. Keep the `design` link when you add a
story, and compare the two sides when either changes.

In the pantry (`apps/but-ui-pantry`), a page is at
`http://localhost:6007/?page=components-<name>`, and a story alone, with its
`play` run, at `?story=components-<name>--<export>`; append `&theme=dark` for
the dark theme. A note in but-ui's `design/components/` shows as Guidelines on
the page of each component whose JSDoc names it with `@see`.

`test` runs every story as a test in headless Chromium; a story fails when it
throws or its `play` fails. `pnpm -F @gitbutler/but-ui-react test:stories`
runs them alone, after `pnpm -F @gitbutler/but-ui-react playwright:install:unit`
once per checkout.

### Default, then examples

A component's stories are one page, in one order:

- **`Default` first:** the simplest use of the component. It answers what
  the component is and accepts. `pnpm check` fails on a stories file that
  opens with anything else.
- **Examples after it:** each named after what it shows — a state
  (`OnlyAdditions`, `NothingToList`) or a surface the app builds from it
  (`Confirm`, `SelectProject`, `CommandPalette`). An example is written as
  the app writes it: its own state, real copy, rendered open, since its
  source is what an agent copies from.

A surface built from several components is an example of the outermost one:
the upload confirmation is Modal's, the project picker is Popup's (a
combobox dressed in its parts), the command palette is PickerDialog's.

In 🍑 But UI the examples sit in a section named `<Component> examples`
directly under the component's own section, on its group's page, one
instance per story, named as the story is; an example story links its
instance through the `design` parameter.

### How a story stands on its page

Place a story with the `layout` parameter and size it with `width` and
`height`, never with a wrapper, border, card or background the story draws:

- default: a muted ground as wide as the text, for most components.
- `wide`: the ground at the column's full width, for a part that fills its pane
  (`Table`, `DiffFile`, `Markdown`) or a grid that fills its row.
- `bar`: edge to edge in a bordered card, for a bar that spans a window or pane
  (`StatusBar`, `LocationBar`, `ViewHeader`).
- `fullscreen`: its own frame, for a whole window or view and every overlay.
- `centered`: the `fullscreen` frame with the story centred in it, for a part that
  floats over a pane (`Toolbox`), so its border and shadow aren't clipped.

Give a size only when the size is the point (a narrow pane, a window), and pick
content that fits about 570px (770px when `wide`).

### Looking up a component

A component's JSDoc names the import to write in its `@import` tag, as
`@import import { Badge } from "@gitbutler/but-ui-react/Badge.tsx";`; give a new
component one. A styling helper is not how a control is offered:
`getButtonClassName` is for making another component look like a button, and
a button is `<Button>`.

A new design notes page is a Markdown file in its folder under
`packages/but-ui/design/` and a row in that package's DESIGN.md; the site finds
it and titles it from its heading. Only a page about building with this library
goes in `design/` here.

### Linting & formatting

The package is linted by oxlint with its own `oxlint.config.ts`, not by
ESLint:

```console
$ pnpm exec prettier --check packages/but-ui-react
$ pnpm run oxlint
```

The lint job in CI runs both and fails on what either reports.
