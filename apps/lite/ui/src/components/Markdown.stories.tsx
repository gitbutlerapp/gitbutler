import preview from "#storybook/preview";
import type { GUISettings } from "#electron/settings.ts";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Markdown } from "./Markdown.tsx";

// The code block reads the syntax theme through the settings query, which
// the Electron bridge answers in the app. Seeding the client with the
// defaults keeps the story offline and quiet.
const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
queryClient.setQueryData(guiSettingsQueryOptions.queryKey, defaultSettings as GUISettings);

const meta = preview.meta({
	component: Markdown,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=1930-1729",
		},
	},
	decorators: [
		// The width a PR description gets in the details pane; the blocks in
		// Figma are set to the same measure.
		(Story) => (
			<QueryClientProvider client={queryClient}>
				<div style={{ width: 480, padding: 24 }}>
					<Story />
				</div>
			</QueryClientProvider>
		),
	],
});

/**
 * The same document as the default content of "Markdown / slot" in ⚛️ Lite
 * Core: every block the kit has a component for, in the app's rhythm.
 * Compare the two when either side changes.
 */
export const Sample = meta.story({
	args: {
		children: `# Summary

Adds a description editor to the PR create form. When the body is empty, the editor offers a one-line prompt instead of a blank field, so a short description is one click away.

## What changed

- The empty state is a button that opens the editor with the commit messages pasted in
- Existing descriptions open as before

Run it locally with the CLI:

\`\`\`sh
$ but commit -m "Add description editor"
$ but push
\`\`\`

### Notes for reviewers

> Reviewers asked for a shorter first paragraph, so the summary now leads with what changed.

| File | Change | Status |
| --- | --- | --- |
| Markdown.tsx | Description editor | Done |
| Field.tsx | Empty prompt | In review |

- [ ] Update the screenshots in the PR
- [ ] Add the DESIGN.md section

![App preview](https://raw.githubusercontent.com/gitbutlerapp/gitbutler/master/apps/web/static/images/app-preview-light.png)

---

[See the design notes](https://github.com/gitbutlerapp/gitbutler/blob/master/apps/lite/DESIGN.md)

Then run \`but status\` to check.`,
	},
});

/** The three heading levels next to body text, and the levels below them that share the body size. */
export const Headings = meta.story({
	args: {
		children: `# Heading 1

Body text at 13px on a 160% line.

## Heading 2

Body text at 13px on a 160% line.

### Heading 3

Body text at 13px on a 160% line.

#### Heading 4

Levels four to six stay at the body size and only go semibold.`,
	},
});

/** Inline chips, a keycap, and a fold: the pieces Figma can't run inside a text line. */
export const Inline = meta.story({
	args: {
		children: `Press <kbd>⌘</kbd> <kbd>Enter</kbd> to commit, or run \`but commit\` from a shell.

<details>
<summary>Why a fold?</summary>

Long logs and stack traces go behind a fold so the description stays readable.

</details>`,
	},
});
