# @gitbutler/ui-react

The React component library: the components, icons, illustrations and base
styles. It ships source, not a build; each consumer's Vite compiles it, and
React Compiler runs on it under the consumer's config as it does on the
consumer's own code.

`DESIGN.md` beside this file is the visual language: how things should look
and read. It is an index of the pages in `design/`, one per topic; read the
ones for what you are changing before changing anything a user sees. This file
is the tooling that enforces them.

## Writing the code

React Compiler makes `useMemo`, `useCallback` and `React.memo` redundant,
except in hot paths where the compiler can't tell that a computation is pure;
check the compiler's output before adding one. `useEffect` is usually an
anti-pattern: think hard before reaching for it, and ask for consent before
adding one. Comments explain the non-obvious why, never the what; when in
doubt, leave the comment out.

### Changing a component

**A change to a component here needs the designer's okay.** Every app draws
with these components, so a fix made for one surface changes all of them.
When an app's bug seems to need a change here (a style, a prop, a default, a
wrapper), try fixing it in the app first. If the library really is at fault,
ask before changing it. When you can't ask, make the change its own
`fix(ui-react): …` commit that says which surface showed the bug and what
else it touches, and call it out in the PR description.

### What belongs here

A component belongs here when it doesn't reach into an app: no API calls,
no settings, no store, nothing on `window`. Everything a component needs
from its host arrives as a prop, the way `TextLink` takes its `onClick` rather
than assuming how its host opens a link. A component that needs the app stays
in the app.

Imports are deep and carry the extension: `@gitbutler/ui-react/Button.tsx`.
There is no barrel file. Inside the package, siblings import each other
relatively.

### Styles

Tokens come from `@gitbutler/design-core`, imported by the host once.
`src/base.css` declares the few variables components read that aren't tokens
(`--focus-ring`, `--control-cursor`, `--transition-button`) and the control
cursor rules; every host imports it right after design-core. A variable nothing declares doesn't error, the
property quietly falls back, so a new shared variable goes in `base.css`, not
in a host's stylesheet.

### Icons

Icons are SVGs in `src/icons/`, inlined into the bundle as raw strings; the
filename is the icon name (`folder-lock.svg` → `<Icon name="folder-lock" />`).
To add one:

1. Export it from Figma at 16×16 (⚛️ Core library) as SVG into `src/icons/`.
2. Run `pnpm -F @gitbutler/ui-react optimize-icons`. It minifies the SVG,
   recolors it to `currentColor` and regenerates `src/iconNames.ts`.
3. Commit both. `iconNames.ts` is generated; never hand-edit it.

`src/file-icons/` are the language and filetype glyphs. They keep their brand
colours and are deliberately not run through the script.

`src/program-icons/` are the marks of editors and terminals, as PNGs at 2×.
Their rounded corner is part of the image: export each from the "Programm"
frame on ⚛️ Core's Icons page, where the mark is a 14px rectangle with
the corner on it, as PNG at 2× with the layer named as the file is
(`vscode` → `vscode@2x.png`). No CSS rounds them, so a square export shows
square.

## Figma

Two files, split the way the code is:

- **⚛️ Core** —
  <https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core>,
  the published library: tokens (Tokens page), icons (Icons page) and this
  package's components, one section per component. It is the source of truth
  for tokens; for components, code is, and the drawing follows it. The
  components are on six pages, by what they are for: Actions, Inputs,
  Overlays, Content & status, Preview (a details view's header and the bar
  above it), and Sidebar. A new component's section goes at
  the end of its group's page, with its examples section under it. The docs
  site shelves the stories in the same groups (plus Icons, which Figma keeps
  on its own page): a stories file names its group in `title` and keeps its
  old `id`, so the addresses below don't change when a component moves group.
- **Client**, the working file —
  <https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Client>: app
  components on the ⚙️ Meta page, the full screens, and drafts on the
  🚧 Drafts page and the pages under it. A new mockup or an exploration goes
  here, never in the library.

A component is drawn where its code lives: one in this package goes in
⚛️ Core, one in an app on ⚙️ Meta. When a component moves between the two
in code, its drawing moves with it in the same piece of work, and the
mockups' instances are swapped to the new one.

The drawing mirrors the code's structure: the same component under the same
name, variant properties named as the props and their values (`status`,
`variant`, `glyph`), and no property the code lacks. Figma may add only what
the code produces from data, such as a toggle for a side the code drops when
its count is zero, and a `state` property (`default`, `hover`) for what CSS
draws under the pointer, as Button has; two components the code keeps apart
stay apart in Figma and are placed side by side where a surface shows both.

A component's description in Figma says what its JSDoc and DESIGN.md say
about it: what it is for, when to pick which variant, and a `Code:` line
naming the component and its file. Change one and change the other in the
same piece of work; a description that lags the code is what an agent reading
Figma will build from.

## Verifying your work

Always run the specified commands **exactly** as written. The package has no
app to run, so the story is how a component is checked before it goes into a
surface.

### Typechecking, the CSS rules, and tests

```console
$ pnpm -F @gitbutler/ui-react check
$ pnpm -F @gitbutler/ui-react test
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
`@gitbutler/design-core`, a variable `base.css` or a component declares, or
one a dependency sets. A name nothing declares doesn't error in the browser;
the property silently falls back. `check` runs `scripts/check-tokens.mjs`,
which fails on any undefined name and suggests the nearest real one. When a
dependency sets a variable at runtime that the script can't see, or a
component reads a knob its host may set, add it to `KNOWN_RUNTIME` in the
script with who sets it.

### Stories and the docs site

Every component has a story beside it, in both themes, and a component that
lives only in code is half a component. `pnpm check` holds the package to it
through `scripts/check-stories.mjs`, which fails on a component no story
imports.

A stories file describes its page with `stories()` from `docs/stories.ts`,
and each story with `meta.story()`:

```tsx
const meta = stories({ title: "Content & status/Badge", id: "components-badge", component: Badge });
export const Default = meta.story({ args: { children: "42" } });
```

A story takes `args`, a `render(args)`, `decorators`, `parameters` (`design`,
the Figma link, and `layout`) and a `play` function. The docs site, in
`docs/`, is a small Vite app that draws every page with these components: one
page per component under its group's heading, every story on it with its
JSDoc, and the design notes from `DESIGN.md` and `design/`. It runs on port
6007:

```console
$ pnpm -F @gitbutler/ui-react demos
```

A page is at `http://localhost:6007/?page=components-<name>`. A story renders
alone at `http://localhost:6007/?story=components-<name>--<export>`, with its
`play` function run; append `&theme=dark` for the dark theme. Overlays' stories
and full-window ones are shown that way on their page, each in a frame of its
own. Check colour, font and spacing from computed styles rather than by eye. A
story links its Figma component through the `design` parameter; keep the link
when you add a story, and compare the two sides when either changes.
`grep -l 'type: "figma"' src/*.stories.tsx` lists the stories that have one;
a story without one has no drawn spec yet.

`test` runs the unit tests and then every story as a test in headless
Chromium (`docs/stories.test.tsx`). A story fails when it throws while
rendering or its `play` function fails. Run the stories alone with:

```console
$ pnpm -F @gitbutler/ui-react test:stories
```

They need Playwright's Chromium, once per checkout:

```console
$ pnpm -F @gitbutler/ui-react playwright:install:unit
```

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

In ⚛️ Core the examples sit in a section named `<Component> examples`
directly under the component's own section, on its group's page, one
instance per story, named as the story is; an example story links its
instance through the `design` parameter.

### Looking up a component

Before using a component, read its source and JSDoc in `src/` and its stories
beside it, rather than guessing. Use only props the types declare; if one
seems missing, ask instead of inventing it. A component's JSDoc names the
import to write in its `@import` tag, as
`@import import { Badge } from "@gitbutler/ui-react/Badge.tsx";`; give a new
component one. A styling helper is not how a control is offered:
`getButtonClassName` is for making another component look like a button, and
a button is `<Button>`.

A new design notes page is a Markdown file in its folder under `design/` and a
row in DESIGN.md; the site finds it and titles it from its heading.

### Linting & formatting

The package is linted by oxlint with its own `oxlint.config.ts`, not by
ESLint:

```console
$ pnpm exec prettier --check packages/ui-react
$ pnpm run oxlint
```

Run these before declaring the work done. The lint job in CI runs both and
fails on what either reports.
