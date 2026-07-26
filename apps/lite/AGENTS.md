# Dependencies

JavaScript dependencies are sourced from pnpm. Commands are surfaced via pnpm.

# Automation

In dev the app is accessible for agent automation on port 9222. A working
CDP driver script and its gotchas are in
`.agents/skills/lite-render-perf/SKILL.md` under "Driving the dev app over
CDP".

# Typechecking

Typechecking is the fastest way to validate that everything is okay. Always run this **exact** command to typecheck:

```console
$ pnpm -F @gitbutler/lite check
```

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

# State

Share machinery, not state: when a new surface (a tab, pane, or mode) has its
own selection or lifecycle, give it its own sub-state with its own
reducers/selectors (see `ui/src/projects/branches.ts`), even when it reuses the
same operand/navigation machinery. Don't multiplex an existing state container
behind mode conditionals — the tell is an `if (tab === ...)` guard, or a
comment explaining a special case, in code that shouldn't know that mode
exists.

# Concluding your work

Once the work is functionally complete, lint and format it with Oxlint, Oxfmt,
Prettier, and Knip. Oxfmt only formats TypeScript; CI runs Prettier over the
whole repo, including the CSS and Markdown that Oxfmt leaves untouched, so run
it too or those files fail CI:

```console
$ pnpm oxlint:fix && pnpm exec oxfmt apps/lite && pnpm exec prettier --write . && pnpm knip:prod && pnpm knip:non-prod
```
