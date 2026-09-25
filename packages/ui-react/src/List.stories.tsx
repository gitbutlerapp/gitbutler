import preview from "#storybook/preview";
import { FileIcon } from "./FileIcon.tsx";
import { Icon } from "./Icon.tsx";
import { List, ListItem } from "./List.tsx";

const meta = preview.meta({
	component: List,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2238-1670",
		},
	},
	decorators: [
		(Story) => (
			<div style={{ width: 320, padding: 24 }}>
				<Story />
			</div>
		),
	],
});

/** The default marker: a dot on each item's first line. A long item wraps under itself. */
export const Dots = meta.story({
	render: () => (
		<List>
			<ListItem>Rejected by a pre-receive hook: 2 files</ListItem>
			<ListItem>Too large for the remote: assets/video.mov</ListItem>
			<ListItem>
				A longer reason that wraps onto a second line stays aligned with its own text, not with the
				dot
			</ListItem>
		</List>
	),
});

/** Files carry their own icon in place of the dot, so the list says what each one is. */
export const Files = meta.story({
	render: () => (
		<List>
			{["screenshot.png", "pasted-image.png", "src/components/App.tsx"].map((name) => (
				<ListItem key={name} marker={<FileIcon fileName={name} />}>
					{name}
				</ListItem>
			))}
		</List>
	),
});

/** Any glyph from the icon set can stand in, for branches, commits and the like. */
export const Icons = meta.story({
	render: () => (
		<List>
			<ListItem marker={<Icon name="branch" />}>feature/modal-parts</ListItem>
			<ListItem marker={<Icon name="branch" />}>fix/menu-above-dialogs</ListItem>
		</List>
	),
});
