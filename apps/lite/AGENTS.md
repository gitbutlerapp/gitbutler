## Dependencies

- Native dependencies: sourced from Nix in a flake devshell
- JavaScript dependencies: sourced from pnpm

All commands should run in a Nix flake devshell.

## Typechecking

Typechecking is the fastest way to validate that everything is okay. Always run this **exact** command to typecheck:

```console
$ nix develop -c pnpm -F @gitbutler/lite check
```

## Components

Memoization utilities such as `useMemo`, `useCallback`, and `React.memo` are redundant as we use React Compiler.

Component definitions should follow this pattern, optionally destructuring `p`:

```tsx
type Props = {
  ...
};

export const MyComponent: FC<Props> = (p) => {
  // [...]
};
```

# Concluding your work

Once the work is functionally complete, lint and format it with Oxlint, Prettier, and Knip:

```console
$ nix develop -c bash -c "pnpm oxlint:fix && pnpm exec prettier --write apps/lite && pnpm knip:prod && pnpm knip:non-prod"
```
