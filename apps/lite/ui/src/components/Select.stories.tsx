import preview from "#storybook/preview";
import { Select } from "./Select.tsx";

const terminals = [
	{ value: "terminal", label: "Terminal" },
	{ value: "warp", label: "Warp" },
	{ value: "ghostty", label: "Ghostty" },
	{ value: "iterm2", label: "iTerm2" },
];

const meta = preview.meta({
	component: Select,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2039-2321",
		},
	},
	args: {
		items: terminals,
		label: "Terminal",
		placeholder: "Select terminal",
		style: { width: 170 },
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "48px 48px 200px" }}>
				<Story />
			</div>
		),
	],
});

/** Nothing chosen yet: the trigger reads the placeholder, and the list opens with it as its first row. */
export const Default = meta.story({});

/** The chosen row is ticked, and the list opens with that row over the trigger. */
export const WithValue = meta.story({
	args: { defaultValue: "terminal" },
});

export const Disabled = meta.story({
	args: { defaultValue: "terminal", disabled: true },
});

/** A glyph on each row, for a list whose choices are of different kinds. */
export const WithIcons = meta.story({
	args: {
		label: "Open in",
		placeholder: "Select editor",
		style: { width: 220 },
		items: [
			{ value: "vscode", label: "Visual Studio Code", icon: "edit" },
			{ value: "zed", label: "Zed", icon: "edit" },
			{ value: "finder", label: "Finder", icon: "folder" },
			{ value: "workbench", label: "Workbench", icon: "workbench", disabled: true },
		],
	},
});

/** Without a label of its own, for a select the surface already names — a row in a settings table. */
export const Unlabelled = meta.story({
	args: { label: undefined, "aria-label": "Terminal", defaultValue: "warp" },
});
