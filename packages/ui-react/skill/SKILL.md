---
name: gitbutler-ui
description: Use when changing what users see in an app built on `@gitbutler/ui-react` — Lite, the but.dev web app — adding or restyling a component, modal, form, list, empty state or toast, or writing the words on screen. Not for logic, data or backend work, even inside a UI file.
---

# Building GitButler UI

Work as you would in any codebase: find the closest thing that already
exists and copy it. These notes say where to look and what to check. They are
not a gate to clear before starting.

## Scope

Change what the task needs. Older screens may not follow these notes; leave
them alone unless the task is to fix them.

## Start from the library

`@gitbutler/ui-react` has the controls: `Button`, `Popup` (modals, dropdowns),
`Select`, `Field`, `Checkbox`, `Switch`, `Tooltip`, `EmptyState`, `Snackbar`
and more. Use them rather than a control styled in a feature's CSS. Find a
surface that already uses the ones you need and follow its structure.

The source is the reference, because it is the version you build with:

- In the gitbutler repository: `packages/ui-react/src/`
- In but.dev: `web/node_modules/@gitbutler/ui-react/src/`

Each component's JSDoc says what its props do. The Storybook MCP server
(`docs-show`) and
<https://master--6ab536f5f40e41db628ccf1b.chromatic.com/manifests/components.json>
have the same docs with examples; use them when they answer, and go back to the
source when they don't. Don't wait on them.

When nothing fits, build the smallest thing that works and say so in the PR
description: what was needed, which components you tried.

## Read the part of DESIGN.md you need

The shared design language is the library's `DESIGN.md`, beside `src/` in the
same two places. Read the sections for what you are building, not the whole
file:

| Building                               | Sections                    |
| -------------------------------------- | --------------------------- |
| Buttons, choosing which one stands out | Emphasis, Button variants   |
| A link, or text that opens something   | Links                       |
| Anything clickable                     | States, Minimums, Cursors   |
| A form or settings row                 | Fields                      |
| Nothing to show yet                    | Empty states, Illustrations |
| Telling the user something happened    | Toasts and snackbars        |
| A hover hint                           | Tooltips                    |
| An animation                           | Motion                      |
| Any words on screen                    | Voice                       |

Each app keeps its own choices in its own `DESIGN.md`: Lite's is
`apps/lite/DESIGN.md`, but.dev's is at its repository root.

## Words

Short and plain. A label is one to three words; a hint is one sentence. Call
things what the rest of the app calls them. Say what the user can't already see
on screen, once, without parentheses.

## Check it

Look at the result in light and dark, and keep one screenshot. In the
gitbutler repository a component's story runs with `pnpm -F @gitbutler/lite
demos`; `apps/lite/AGENTS.md` has how to reach a single story.
