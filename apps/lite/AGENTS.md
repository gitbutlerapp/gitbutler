# Lite

## Before changing anything users see

However small the change:

1. Read `packages/ui-react/DESIGN.md` and `DESIGN.md` beside this file. They
   are the rules; the code around your change may predate them.
2. Look up every component before you use it, with the Storybook MCP
   server's `docs-list` and `docs-show` (`.mcp.json` connects it). If it
   doesn't answer — it can time out while a new Storybook deploys — read
   <https://master--6ab536f5f40e41db628ccf1b.chromatic.com/manifests/components.json>,
   and failing that the component's source and JSDoc in
   `packages/ui-react/src/`. Use only the props they document; the tools being
   down is never a reason to guess one.
3. Build from `@gitbutler/ui-react` — `Button`, `Tooltip`, `EmptyState` and
   the rest — not from controls styled in a feature's CSS module.

## Preparing the checkout

Before implementing or validating Lite changes, ensure this checkout has installed dependencies, generated SDK types/native bindings, and the `but` CLI needed to seed E2E fixtures. In an unprepared checkout, run from the repository root:

```console
$ pnpm install
$ pnpm build:sdk
$ cargo build -p but
```

Reuse completed setup in this checkout. If another agent is preparing the same checkout, coordinate rather than starting duplicate installs/builds. Isolated checkouts need their own setup if missing. Read-only investigation does not require setup.

pnpm manages Node.js runtime and dependency installation, so always use pnpm scripts or `pnpm exec`.

Rebuild the SDK after Rust changes, not for frontend-only edits.

## Running the app

After preparing the checkout, run from the repository root:

```console
$ pnpm dev:lite
```

Verify running apps and servers belong to this checkout before reusing them, including the Vite server reused by E2E tests. Ask about port conflicts rather than stopping another checkout's processes.

## Writing the code

### Memoization

Memoization utilities such as `useMemo`, `useCallback`, and `React.memo` are usually redundant as we use React Compiler, however may be necessary in hot paths where the compiler fails to understand that a computation is pure and therefore safe to memoise. Always validate the memoisation properties of modified React body code directly against React Compiler.

To bypass this issue, where Redux store values are only needed at event-time (i.e. non-reactively), prefer `useAppStore` over `useAppSelector` subscriptions. The same goes for React Query where only cache reads are required.

### Code smells

`useEffect` is typically an anti-pattern. Think long and hard before declaring it the best option. Should it appear to be the best option, always ask for consent to include it.

### Comments

Only include code comments where the higher-level purpose of the code may not be self-evident, for example unobvious technical edge cases. The "what" should be self-evident. If in doubt don't include a comment.

### Data fetching

All data fetching in React should take place via React Query. If abstraction is necessary, start by extracting query options.

All persisted client-side state that's not a setting should live in IndexedDB.

Consider backwards compatibility for any persisted state.

## Design

The visual language — how icons, color, and composition should look — is in
`packages/ui-react/DESIGN.md`, with the component library it describes, and
what Lite decides for itself is in `DESIGN.md` beside this file. Read both
before changing anything users see. The library's own `AGENTS.md` covers
its tooling: icons, stories, the checks to run. This section covers what is
the app's own.

### Icons

There are two icon sets with two separate scripts, and each script only walks
its own directory:

| Path                                         | Owner                                  | Script                                           |
| -------------------------------------------- | -------------------------------------- | ------------------------------------------------ |
| `packages/ui-react/src/icons/*.svg`          | React component library (Lite, panel)  | `pnpm -F @gitbutler/ui-react optimize-icons`     |
| `packages/ui-svelte/src/lib/icons/svg/*.svg` | shared Svelte UI package (desktop/web) | `pnpm -F @gitbutler/ui-svelte optimize-ui-icons` |

Running `optimize-ui-icons` will **not** touch a Lite icon, and vice versa.
Dropping an SVG into the wrong folder is the most common reason an icon "won't
optimize". File icons (`packages/ui-react/src/file-icons/`) are deliberately not
run through either script — recoloring them to `currentColor` would destroy
them.

To add an icon, follow `packages/ui-react/AGENTS.md`.

### Tokens

Every `var(--x)` Lite reads has to be a name something declares: a token from
`@gitbutler/design-core`, a variable Lite's own CSS or TS sets, or one a
dependency documents. A name nothing declares doesn't error in the browser; the
property silently falls back, which is how a misremembered token once shipped
square corners. `pnpm -F @gitbutler/lite check` runs
`apps/lite/scripts/check-tokens.mjs`, which fails on any undefined name and
suggests the nearest real one. When a dependency sets a variable at runtime
that the script can't see, add it to `KNOWN_RUNTIME` in the script with who
sets it.

### Components

Every component in `packages/ui-react/src/` has a story beside it, and the story
is how a component is checked on its own, in both themes, before it goes
into a surface. Storybook runs on port 6007:

```console
$ pnpm -F @gitbutler/lite demos
```

A story renders alone, without the Storybook chrome, at

```
http://localhost:6007/iframe.html?id=<title>--<export>&viewMode=story
```

No story sets a title, so Storybook derives it from the file's path under
`ui/src`, and the export name gives the second half; both are kebab-cased.
`ui/src/components/Markdown.stories.tsx` with `export const Sample` is
`components-markdown--sample`. Append `&globals=theme:dark` for the dark
theme.

Check colour, underline, font and spacing from computed styles (the
browser's inspector, or a Playwright script against the iframe URL) rather
than by eye, and keep one screenshot as the proof. A story links its Figma
component through the `design` parameter when one exists, so the two sides
can be compared when either changes.

Storybook also writes the component manifest (`/manifests/components.json`)
from every story, the app's included, and the import it lists for a
component is the `@import` tag in that component's JSDoc, as
`@import import { Markdown } from "#ui/components/Markdown.tsx";`. Give an
app component with a story one, or mark a story that has no component
behind it with `tags: ["!manifest"]`, as `AppUpdater.stories.tsx` does.
`packages/ui-react/AGENTS.md` has the rest about the manifest.

Lite's own components are drawn on the ⚙️ Meta page of the Client working file,
<https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Client>, and drafts go on
its 🚧 Drafts pages; the library's are in ⚛️ Core.
`packages/ui-react/AGENTS.md`, under Figma, has the rules for both.

## Verifying your work

In dev the app is accessible for automation over CDP on port 9222.

Always run the specified commands **exactly** as written.

### Typechecking

Typechecking is the fastest way to validate that everything is okay.

```console
$ pnpm -F @gitbutler/lite check
```

### Testing

Our unit tests are written with Vitest and our E2E tests with Playwright.

```console
$ pnpm -F @gitbutler/lite test
$ pnpm -F @gitbutler/lite test:e2e
```

### Linting & formatting

Once the work is functionally complete, run the following linters and formatters.

```console
$ pnpm oxlint:fix
$ pnpm knip:prod
$ pnpm knip:non-prod
$ pnpm exec oxfmt apps/lite
$ pnpm exec prettier --write apps/lite
```
