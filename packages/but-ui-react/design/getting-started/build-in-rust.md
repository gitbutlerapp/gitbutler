# Build in Rust

**Two libraries, one design system.** `@gitbutler/but-ui-react` is the React
library this site shows. Its Rust twin is `crates/but-ui-gpui`, which the GPUI
client draws with. Both read the same tokens
(`packages/but-ui/tokens/tokens.json`), the same icons
(`packages/but-ui-react/src/icons`) and the same illustrations, so a colour, a
radius or an icon changes in one place. The components are written twice, once
for each, and drawn to match.

**Use it from Rust.** Depend on `but-ui-gpui` and import what you need, as
`use but_ui_gpui::{button, ButtonStyle}`. Sizes go in `dp`, colours come from
the theme. Inside the GPUI client, `ui` re-exports the crate, so `ui::button` is
the same button.

**Only shared components live there.** A component goes in the crate when this
library has it too and it reaches into no app: everything it needs arrives as an
argument, as props do here. Each file names its counterpart in its first doc
comment ("as but-ui-react's `Button`"), which is how this site knows. A part
that reads the app's store or backend, such as the diff view, stays in the app.

**The GPUI badge marks a shared component.** In the sidebar, GPUI means the
crate has that component. On its page every example gets a React/GPUI switch:
GPUI shows the GPUI client's screenshot of the same story, light or dark as the
site is. "Not drawn in GPUI yet" means the component is shared but that story
isn't: a gap to fill, or a variant the GPUI component doesn't have yet.

**Add a story's GPUI version** to the component's story in
`crates/but-gpui/src/stories`, under this site's id and name for it
(`components-button--icon-only`, "Icon Only"). Then run
`packages/but-ui-react/scripts/gpui-shots.mjs`, which takes it light and dark,
and commit the pictures in `packages/but-ui-react/docs/gpui`. `pnpm check` fails
when an example names a story this site doesn't have.

**Keep the two in step.** Change a shared component on both sides in the same
piece of work, and take its shots again, so the GPUI view never shows an old
picture.
