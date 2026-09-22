import preview from "#storybook/preview";
import { ProgramIcon } from "./ProgramIcon.tsx";
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

/** A program's own mark on each row and on the trigger, for a list of editors or terminals. */
export const WithImages = meta.story({
	args: {
		label: "Terminal",
		placeholder: "Select terminal",
		defaultValue: "warp",
		items: terminals.map((terminal) => ({
			...terminal,
			leading: <ProgramIcon program={terminal.value} />,
		})),
	},
});

/** Without a label of its own, for a select the surface already names — a row in a settings table. */
export const Unlabelled = meta.story({
	args: { label: undefined, "aria-label": "Terminal", defaultValue: "warp" },
});

/** A list too long to scan leads with the search row every other long list has, and filters as
 * the query is typed. */
export const Searchable = meta.story({
	args: {
		label: "Syntax theme",
		"aria-label": undefined,
		placeholder: undefined,
		defaultValue: "github-light-default",
		searchable: true,
		searchPlaceholder: "Search themes...",
		nothingFound: "No themes found",
		style: { width: 240 },
		items: [
			"Ayu Light",
			"Catppuccin Latte",
			"Everforest Light",
			"GitHub Light",
			"GitHub Light Default",
			"GitHub Light High Contrast",
			"Gruvbox Light Hard",
			"Gruvbox Light Medium",
			"Gruvbox Light Soft",
			"Horizon Bright",
			"Kanagawa Lotus",
			"Light+ (VS Code)",
			"Material Lighter",
			"Min Light",
			"One Light",
			"Rosé Pine Dawn",
			"Slack Ochin",
			"Snazzy Light",
			"Solarized Light",
			"Vitesse Light",
		].map((label) => ({ value: label.toLowerCase().replace(/[^a-z0-9]+/g, "-"), label })),
	},
});
