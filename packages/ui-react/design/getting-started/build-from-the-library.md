# Build from the library

**Build from the library.** Every control users touch — a button, a switch, a
segmented toggle, a popup — already has a component in `@gitbutler/ui-react`
(`packages/ui-react/src/`), with a spec in ⚛️ Core or a Storybook story,
published at <https://master--6ab536f5f40e41db628ccf1b.chromatic.com>. Use it
even when hand-styling in the feature's CSS module would be quicker: a control
styled locally drifts. Look up props and the import rather than guessing, in
the Storybook manifest,
<https://master--6ab536f5f40e41db628ccf1b.chromatic.com/manifests/components.json>,
or its MCP server, `https://master--6ab536f5f40e41db628ccf1b.chromatic.com/mcp`.

**If the library lacks it, think twice, then ask.** Check whether a variant or
prop of an existing component fits — a small size, an icon-only mode — and if
nothing does, ask the designer before building.

**When you can't ask, flag it.** Build the smallest thing that works. The PR
description gets a "New UI" note: what was needed, which library components
were tried and why none fit, and where the new one lives. File a Linear ticket
in GitButler Internal (GB) assigned to Pavel Laptew (@pavel) with that note,
the PR link and a screenshot if you can. If you can't reach Linear, say so in
the note and the PR author files it.

**A custom control needs a reason.** The commit or a comment says what the
library could not do and why that mattered here. Without it, a one-off is a
bug waiting for a redesign.

**New components are documented.** Anything that graduates into
`@gitbutler/ui-react` gets a story and, once the designer has drawn it, a
Figma spec.
