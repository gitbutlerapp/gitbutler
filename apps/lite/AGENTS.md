# Lite

JavaScript dependencies are sourced from pnpm. Commands are surfaced via pnpm.

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
`apps/lite/DESIGN.md`. Read it before changing anything users see. This section
covers the tooling that enforces it.

### Icons

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
