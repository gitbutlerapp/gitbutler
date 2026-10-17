import preview from "#storybook/preview";
import { Badge } from "./Badge.tsx";
import { Button } from "./Button.tsx";
import { FileList, FileListItem } from "./FileList.tsx";
import { FolderIcon } from "./FolderIcon.tsx";
import { Icon } from "./Icon.tsx";
import { Dropdown, Popup, PopupEmpty, PopupItem, PopupSearch, PopupSection } from "./Popup.tsx";
import { ProgramIcon } from "./ProgramIcon.tsx";
import { Combobox } from "@base-ui/react";
import { useDeferredValue, useState } from "react";

const meta = preview.meta({
	title: "Overlays/Popup",
	id: "components-popup",
	component: Popup,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=1819-3456",
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
export const Default = meta.story({
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
				<PopupItem leading={<ProgramIcon program="vscode" />} trailing="tick">
					Leading image
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

type Project = { id: string; title: string; private: boolean };

const projects: Array<Project> = [
	{ id: "1", title: "rocketFlasher", private: false },
	{ id: "2", title: "Fliege-mono", private: true },
	{ id: "3", title: "brutalism", private: false },
	{ id: "4", title: "but-dev", private: false },
	{ id: "5", title: "clock-demo", private: true },
];

/**
 * The project selector: a combobox anchored under the button that names the current project.
 * Base UI's combobox owns its popup, input and rows, so `Popup`, `PopupSearch` and `PopupItem`
 * dress those parts through `render` rather than wrapping them; `anchored` makes it open the way
 * every dropdown does. The tick marks the current project, and the action below the list is a row
 * of its own section.
 */
export const SelectProject = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=1839-4085",
		},
	},
	render: function SelectProject() {
		const [open, setOpen] = useState(true);
		const [current, setCurrent] = useState(projects[0] ?? null);
		const [query, setQuery] = useState("");
		const deferredQuery = useDeferredValue(query);

		return (
			<Combobox.Root<Project>
				items={projects}
				open={open}
				onOpenChange={setOpen}
				value={current}
				onValueChange={(project) => {
					if (project === null) return;
					setCurrent(project);
					setOpen(false);
				}}
				inputValue={query}
				onInputValueChange={setQuery}
				itemToStringLabel={(project) => project.title}
				itemToStringValue={(project) => project.id}
				isItemEqualToValue={(a, b) => a.id === b.id}
				autoHighlight
			>
				<Combobox.Trigger
					aria-label={`Select project (current: ${current?.title ?? "none"})`}
					render={<Button variant="ghost" />}
				>
					<FolderIcon />
					{current?.title}
				</Combobox.Trigger>

				<Combobox.Portal>
					<Combobox.Positioner align="start" sideOffset={4}>
						<Popup anchored style={{ width: 256 }} render={<Combobox.Popup />}>
							<PopupSearch
								placeholder="Search projects…"
								aria-label="Search projects"
								onClear={query === "" ? undefined : () => setQuery("")}
								render={<Combobox.Input />}
							/>
							<Combobox.Empty>
								<PopupEmpty
									query={deferredQuery}
									nothingFound="No projects found"
									nothingToList="No projects yet"
									onClearSearch={() => setQuery("")}
								/>
							</Combobox.Empty>
							<Combobox.List>
								{(project: Project) => (
									<PopupItem
										key={project.id}
										icon={project.private ? "lock" : "folder-tree"}
										trailing={project.id === current?.id ? "tick" : undefined}
										render={<Combobox.Item value={project} />}
									>
										{project.title}
									</PopupItem>
								)}
							</Combobox.List>
							<PopupSection>
								<PopupItem trailing="plus" onClick={() => setOpen(false)}>
									Add local repository
								</PopupItem>
							</PopupSection>
						</Popup>
					</Combobox.Positioner>
				</Combobox.Portal>
			</Combobox.Root>
		);
	},
});

/** A search that matched nothing: the line a picker's list shows in place of its rows, and the way
 * back to all of them. */
export const NothingFound = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=3454-84770",
		},
	},
	args: { style: { width: 420 } },
	render: function NothingFound(args) {
		const [query, setQuery] = useState("undo");
		return (
			<Popup {...args}>
				<PopupSearch
					placeholder="Search hotkeys..."
					aria-label="Search hotkeys"
					value={query}
					onChange={(event) => setQuery(event.target.value)}
					onClear={query === "" ? undefined : () => setQuery("")}
				/>
				<PopupEmpty
					query={query}
					nothingFound="No hotkeys found"
					nothingToList="No hotkeys to show"
					onClearSearch={() => setQuery("")}
				/>
			</Popup>
		);
	},
});

/** A list with nothing in it before anything was typed: the cactus and the other line, since
 * nothing was searched for. */
export const NothingToList = meta.story({
	args: { style: { width: 420 } },
	render: function NothingToList(args) {
		const [query, setQuery] = useState("");
		return (
			<Popup {...args}>
				<PopupSearch
					placeholder="Search for branches to apply..."
					aria-label="Search branches"
					value={query}
					onChange={(event) => setQuery(event.target.value)}
					onClear={query === "" ? undefined : () => setQuery("")}
				/>
				<PopupEmpty
					query={query}
					nothingFound="No available branches found"
					nothingToList="Nothing left to apply"
					onClearSearch={() => setQuery("")}
				/>
			</Popup>
		);
	},
});

/**
 * A dropdown that opens on hover too: the button stands for a list it keeps hidden, so resting on
 * it shows the list without a click, and the pointer can move in to pick from it. A click is still
 * the button's own. Opened by hover, the panel leaves focus on the page. The button already names
 * and counts the files, so the list leads with a search instead of its own header.
 */
export const OpenOnHover = meta.story({
	render: () => (
		<Dropdown
			openOnHover
			style={{ width: 280 }}
			trigger={
				<Button variant="ghost" aria-label="Files, 3 files, +12, -3">
					<Icon name="sidebar-narrow" />
					<Badge variant="lightGray">3</Badge>
				</Button>
			}
		>
			<FileList
				title="Files"
				count={3}
				header={<PopupSearch placeholder="Filter files" aria-label="Filter files" />}
			>
				<FileListItem
					name="activity.rs"
					directory="crates/mesh-server/src"
					status="Modification"
					added={8}
					removed={2}
				/>
				<FileListItem
					name="feed.rs"
					directory="crates/mesh-server/src"
					status="Modification"
					added={3}
					removed={1}
				/>
				<FileListItem
					name="rebased.rs"
					directory="crates/mesh-server/tests"
					status="Addition"
					added={1}
				/>
			</FileList>
		</Dropdown>
	),
});
