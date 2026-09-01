# Dependencies

JavaScript dependencies are sourced from pnpm. Commands are surfaced via pnpm.

# Automation

In dev the app is accessible for agent automation on port 9222. A working
CDP driver script and its gotchas are in
`.agents/skills/lite-render-perf/SKILL.md` under "Driving the dev app over
CDP".

# Components

Memoization utilities such as `useMemo`, `useCallback`, and `React.memo` are usually redundant as we use React Compiler, however may be necessary in hot paths where the compiler fails to understand that a computation is pure and therefore safe to memoise.

The compiler does not prevent re-render regressions: it silently skips memoizing calls to imported functions, and context still re-renders every consumer. Before writing code that derives values during render, adds a context, subscribes to the store, or renders lists of rows — or when the UI is slow or re-renders too much — use the `lite-render-perf` skill (`.agents/skills/lite-render-perf/SKILL.md`).

Component definitions should follow this pattern, optionally destructuring `p`:

```tsx
type Props = {
  ...
};

export const MyComponent: FC<Props> = (p) => {
  // [...]
};
```

# Design

The visual language — how icons, color, and composition should look — is in
`apps/lite/DESIGN.md`. Read it before changing anything users see. This section
covers the tooling that enforces it.

## Icons

There are two icon sets with two separate scripts, and each script only walks
its own directory:

| Path                                      | Owner                                  | Script                                    |
| ----------------------------------------- | -------------------------------------- | ----------------------------------------- |
| `apps/lite/ui/src/components/icons/*.svg` | Lite                                   | `pnpm -F @gitbutler/lite optimize-icons`  |
| `packages/ui/src/lib/icons/svg/*.svg`     | shared Svelte UI package (desktop/web) | `pnpm -F @gitbutler/ui optimize-ui-icons` |

Running `optimize-ui-icons` will **not** touch a Lite icon, and vice versa.
Dropping an SVG into the wrong folder is the most common reason an icon "won't
optimize". File icons (`ui/src/components/file-icons/`) are deliberately not
run through either script — recoloring them to `currentColor` would destroy
them.

To add an icon to Lite:

1. Export it from Figma at 16×16 (⚛️ Lite Core library) as SVG.
2. Save it to `ui/src/components/icons/` with a kebab-case name — the filename
   _is_ the icon name (`folder-lock.svg` → `<Icon name="folder-lock" />`).
3. Run:

   ```console
   $ pnpm -F @gitbutler/lite optimize-icons
   ```

4. Commit both the SVG and the regenerated `ui/src/components/iconNames.ts`.

The script is `apps/lite/scripts/optimize-icons.mjs`; its header comment
documents each transform and the export problems it can't fix. It is
idempotent, so it's safe to run any time. `iconNames.ts` is generated — never
hand-edit it; add or remove the SVG and re-run. Icons are inlined into the
bundle as raw strings and injected with `dangerouslySetInnerHTML`, which is why
the script minifies them.

After running the script, render the icon in the app (or in `Icon.stories.tsx`)
at both 16px and a larger size before committing.

# State

Share machinery, not state: when a new surface (a tab, pane, or mode) has its
own configuration or lifecycle, give it its own sub-state with its own
reducers/selectors (see `ui/src/projects/branches.ts`), even when it reuses the
same address/navigation machinery. Don't multiplex an existing state container
behind mode conditionals — the tell is an `if (tab === ...)` guard, or a
comment explaining a special case, in code that shouldn't know that mode
exists.

List cursors are the ratified exception: every list's cursor lives in the one
`cursors` table (`ui/src/cursors.ts`) because the entries are structurally
uniform — one identity-keyed value per named list, resolved against what the
list currently shows. That uniformity is the license. The moment an entry
needs a list-specific conditional inside the shared machinery
(`if (list === ...)`), it has stopped being an instance of the concept —
eject it back into its own sub-state.

# Verifying your work

Always run the specified commands **exactly** as written.

## Typechecking

Typechecking is the fastest way to validate that everything is okay.

```console
$ pnpm -F @gitbutler/lite check
```

## Testing

Our unit tests are written with Vitest and our E2E tests with Playwright.

```console
$ pnpm -F @gitbutler/lite test
$ pnpm -F @gitbutler/lite test:e2e
```

## Linting & formatting

Once the work is functionally complete, run the following linters and formatters.

```console
$ pnpm oxlint:fix
$ pnpm knip:prod
$ pnpm knip:non-prod
$ pnpm exec oxfmt apps/lite
$ pnpm exec prettier --write apps/lite
```
