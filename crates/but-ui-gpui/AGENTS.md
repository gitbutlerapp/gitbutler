# but-ui-gpui instructions

GitButler's design system on GPUI: the theme, read from `packages/but-ui`'s tokens, the icons
and fonts, and the components that `packages/but-ui-react` has too, each drawn as that one is.
`crates/but-gpui` draws with it and re-exports it from its `ui` module.

- A component belongs here only when it has a counterpart in but-ui-react and reaches into no
  app: everything it needs arrives as an argument, the way a React component takes props. One
  that reads a `Store`, calls a backend or knows about scripted runs stays in the app.
- Name it after what but-ui-react calls it in its module's doc comment, and keep its variants
  and sizes the React component's.
- Everything in `crates/but-gpui/AGENTS.md` about sizes, colours, fonts, accessibility and GPUI
  applies here too: sizes in `dp`, colours from `Theme`, no panics.
