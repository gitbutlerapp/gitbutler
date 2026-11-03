import { stories } from "../docs/stories.ts";
import { CodeBlock } from "./CodeBlock.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/CodeBlock",
	id: "components-codeblock",
	component: CodeBlock,
	parameters: figma("4614-15710"),
	decorators: [
		(Story) => (
			<div style={{ maxWidth: 480 }}>
				<Story />
			</div>
		),
	],
});

const copyToClipboard = (code: string) => void navigator.clipboard.writeText(code);

export const Default = meta.story({
	args: {
		children: 'but commit -m "Add description editor"\nbut push',
		prompt: true,
		onCopy: copyToClipboard,
	},
});

/** One command, the copy button centred beside it. */
export const OneCommand = meta.story({
	parameters: figma("4614-15714"),
	args: {
		children: "but config forge auth",
		prompt: true,
		onCopy: copyToClipboard,
	},
});

/** A command longer than the block scrolls sideways rather than wrapping onto a second line. */
export const LongCommand = meta.story({
	parameters: figma("4614-15734"),
	args: {
		children:
			'but commit -m "Keep the description editor open while the branch rebases" --branch description-editor',
		prompt: true,
		onCopy: copyToClipboard,
	},
});

/** Lines to paste rather than run take no prompt. */
export const Snippet = meta.story({
	parameters: figma("4614-15752"),
	args: {
		children: "[gpg]\n\tformat = ssh\n[user]\n\tsigningkey = ~/.ssh/id_ed25519.pub",
		onCopy: copyToClipboard,
	},
});
