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
- `src/adoption.json` is what but.dev uses of the library, for the Adoption
  page the Components card opens. `scripts/adoption.mjs` reads it off a
  but-dev checkout's `origin/main`; rerun it there rather than editing it:
  `pnpm -F @gitbutler/but-ui-pantry adoption <but-dev checkout>`.

Run it on port 6007:

```console
$ pnpm -F @gitbutler/but-ui-pantry dev
```

`pnpm -F @gitbutler/but-ui-pantry check` typechecks it.

It's hosted at <https://pantry.gitbutler.com>, a Vercel project rooted here
that builds master; a pull request that changes the pantry or what it draws
from gets a preview of its own. `vercel.json` says what that is, and skips the
build when none of it changed.
