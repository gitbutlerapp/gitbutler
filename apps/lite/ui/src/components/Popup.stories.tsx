import preview from "#storybook/preview";
import { Popup, PopupItem, PopupSearch, PopupSection } from "./Popup.tsx";

const meta = preview.meta({
	component: Popup,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Lite?node-id=4518-292100",
		},
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "48px" }}>
				<Story />
			</div>
		),
	],
});

/** The bare container — what a modal, a dropdown and the toolbox all sit in. */
export const Container = meta.story({
	args: {
		style: { width: 320, padding: 16 },
		className: "text-13",
		children:
			"All modals, dropdowns and the toolbox use this same container. A modal adds a backdrop.",
	},
});

/** Every combination of the row's slots, and the tint it takes from pointer or keyboard alike. */
export const Items = meta.story({
	args: { style: { width: 248 } },
	render: (args) => (
		<Popup {...args}>
			<PopupSection label="Item variants">
				<PopupItem>Bare label</PopupItem>
				<PopupItem icon="plus">Leading glyph</PopupItem>
				<PopupItem trailing="plus">Trailing glyph</PopupItem>
				<PopupItem icon="folder-tree" trailing="tick">
					Both
				</PopupItem>
			</PopupSection>
			<PopupSection label="Shortcuts and submenus">
				<PopupItem kbd="Mod+B">With a shortcut</PopupItem>
				<PopupItem icon="branch" trailing="chevron-right">
					Steps further in
				</PopupItem>
				<PopupItem icon="branch" kbd="Mod+B" trailing="chevron-right">
					Both again
				</PopupItem>
			</PopupSection>
			<PopupSection label="States">
				<PopupItem data-highlighted>Highlighted</PopupItem>
				<PopupItem disabled>Disabled</PopupItem>
			</PopupSection>
		</Popup>
	),
});

/** The project selector, as drawn in the Figma file. */
export const ProjectSelector = meta.story({
	args: { style: { width: 256 } },
	render: (args) => (
		<Popup {...args}>
			<PopupSearch placeholder="Search projects..." aria-label="Search projects" />
			<PopupSection label="Recent projects">
				<PopupItem icon="folder-tree" trailing="tick">
					rocketFlasher
				</PopupItem>
				<PopupItem icon="lock">Fliege-mono</PopupItem>
				<PopupItem icon="folder-tree">brutalism</PopupItem>
			</PopupSection>
			<PopupSection label="Older">
				<PopupItem icon="folder-tree">but-dev</PopupItem>
				<PopupItem icon="lock">clock-demo</PopupItem>
				<PopupItem icon="folder-tree">blogerator</PopupItem>
			</PopupSection>
			<PopupSection>
				<PopupItem trailing="plus">Add local repository</PopupItem>
				<PopupItem trailing="copy">Clone repository</PopupItem>
			</PopupSection>
		</Popup>
	),
});

/** The hotkeys palette: a search, grouped rows, and a body that scrolls under a capped height. */
export const HotkeysPalette = meta.story({
	args: { style: { width: 420, maxHeight: 360 } },
	render: (args) => (
		<Popup {...args}>
			<PopupSearch placeholder="Search hotkeys..." aria-label="Search hotkeys" />
			<div style={{ minHeight: 0, overflow: "auto" }}>
				<PopupSection>
					<PopupItem kbd="F">Toggle files</PopupItem>
				</PopupSection>
				<PopupSection label="Global">
					<PopupItem kbd="Mod+Shift+P">Select project</PopupItem>
					<PopupItem kbd="Mod+.">Toggle sidebar</PopupItem>
				</PopupSection>
				<PopupSection label="Operations log">
					<PopupItem kbd="Mod+Z">Undo</PopupItem>
					<PopupItem kbd="Mod+Shift+O">Show operations log</PopupItem>
					<PopupItem kbd="Mod+Shift+Z">Redo</PopupItem>
				</PopupSection>
				<PopupSection label="Uncommitted changes">
					<PopupItem kbd="Mod+Alt+Enter">Amend</PopupItem>
					<PopupItem kbd="Mod+Enter">Commit</PopupItem>
				</PopupSection>
			</div>
		</Popup>
	),
});
