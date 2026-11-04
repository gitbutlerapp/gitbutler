import { stories } from "../docs/stories.ts";
import { Markdown } from "./Markdown.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/Markdown",
	id: "components-markdown",
	component: Markdown,
	parameters: figma("4809-1328"),
});

const copyToClipboard = (value: string) => void navigator.clipboard.writeText(value);

export const Default = meta.story({
	args: {
		children: `## Summary

Adds a description editor to the PR create form.`,
	},
});

const screenshot = `data:image/svg+xml,${encodeURIComponent(
	'<svg xmlns="http://www.w3.org/2000/svg" width="1480" height="832"><rect width="100%" height="100%" fill="#f4f4f4"/></svg>',
)}`;

/** A pull request description using every block: the text in its column, the table and the
 * screenshot wide. */
export const PullRequestDescription = meta.story({
	parameters: figma("4809-1656"),
	args: {
		onCopy: copyToClipboard,
		children: `# Add a description editor

Adds a description editor to the PR create form. When the body is empty, the editor offers a one-line prompt instead of a blank field, so a short description is one click away.

## What changed

- The empty state is a button that opens the editor with the commit messages pasted in
- Existing descriptions open as before

Run it locally with the CLI:

\`\`\`
but commit -m "Add description editor"
but push
\`\`\`

### Notes for reviewers

> Reviewers asked for a shorter first paragraph, so the summary now leads with what changed.

| File | Change | Status |
| --- | --- | --- |
| Markdown.tsx | Description editor | Done |
| Field.tsx | Empty prompt | In review |

- [ ] Update the screenshots in the PR
- [x] Add the DESIGN.md section

![The empty state](${screenshot})

---

See [the design notes](https://github.com/gitbutlerapp/gitbutler/blob/master/packages/ui-react/DESIGN.md) before changing a component.

Then run \`but status\` to check.`,
	},
});
