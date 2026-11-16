# The pantry

The but-ui design system's site: its design notes and a page for every
component, with each component's stories drawn live, and the GPUI client's
shots of them beside. It shows the libraries; it doesn't hold them.

- The stories live beside their components, in `packages/but-ui-react/src`,
  and the `stories()` format and the story tests in
  `packages/but-ui-react/stories`. Write and change stories there, as
  `packages/but-ui-react/AGENTS.md` says, not here.
- The design notes are `packages/but-ui`'s `DESIGN.md` and `design/`, and
  `packages/but-ui-react/design`. `vite.config.ts` lists the pages from those
  files, so a new note or stories file shows without touching the site.
- `src/gpui` holds the GPUI shots, which `scripts/gpui-shots.mjs` takes.

Run it on port 6007:

```console
$ pnpm -F @gitbutler/but-ui-pantry dev
```

`pnpm -F @gitbutler/but-ui-pantry check` typechecks it.
