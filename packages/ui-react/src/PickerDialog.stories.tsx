import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { Kbd } from "./Kbd.tsx";
import { PickerDialog, type PickerDialogGroup } from "./PickerDialog.tsx";
import { useState } from "react";

type Branch = { ref: string; name: string; updated: string };

const groups: Array<PickerDialogGroup<Branch>> = [
	{
		value: "Local",
		items: [
			{ ref: "refs/heads/toolbox-gap", name: "toolbox-gap", updated: "2 hours ago" },
			{ ref: "refs/heads/design-pages", name: "design-pages", updated: "yesterday" },
			{ ref: "refs/heads/panel-behind", name: "panel-behind", updated: "3 days ago" },
		],
	},
	{
		value: "origin",
		items: [
			{ ref: "refs/remotes/origin/askpass-modal", name: "askpass-modal", updated: "last week" },
			{
				ref: "refs/remotes/origin/hunk-separators",
				name: "hunk-separators",
				updated: "2 weeks ago",
			},
		],
	},
];

const meta = preview.meta({
	title: "Overlays/PickerDialog",
	id: "components-pickerdialog",
	// The bare name, not `PickerDialog<Branch>`: the manifest resolves the component by it. The
	// args' item is then `unknown`, so each callback names its type.
	component: PickerDialog,
	args: {
		ariaLabel: "Apply branch",
		closeLabel: "Close apply branch picker",
		nothingFoundLabel: "No available branches found",
		nothingToListLabel: "Nothing left to apply",
		placeholder: "Search for branches to apply…",
		selectLabel: "Apply branch",
		getItemKey: (branch) => (branch as Branch).ref,
		getItemLabel: (branch) => (branch as Branch).name,
		getItemType: (branch) => (branch as Branch).updated,
		items: groups,
		open: true,
		onOpenChange: () => {},
		onSelectItem: () => {},
	},
});

/** Grouped items, filtered as you type; Enter runs the named action on the highlighted one. */
export const Default = meta.story({});

type Command = { id: string; group: string; name: string; hotkey: string };

const commands: Array<PickerDialogGroup<Command>> = [
	{
		value: "Global",
		items: [
			{ id: "select-project", group: "Global", name: "Select project", hotkey: "Mod+Shift+P" },
			{ id: "toggle-sidebar", group: "Global", name: "Toggle sidebar", hotkey: "Mod+." },
		],
	},
	{
		value: "Operations log",
		items: [
			{ id: "redo", group: "Operations log", name: "Redo", hotkey: "Mod+Shift+Z" },
			{
				id: "operations-log",
				group: "Operations log",
				name: "Show operations log",
				hotkey: "Mod+Shift+O",
			},
			{ id: "undo", group: "Operations log", name: "Undo", hotkey: "Mod+Z" },
		],
	},
	{
		value: "Uncommitted changes",
		items: [
			{ id: "amend", group: "Uncommitted changes", name: "Amend", hotkey: "Mod+Alt+Enter" },
			{ id: "commit", group: "Uncommitted changes", name: "Commit", hotkey: "Mod+Enter" },
		],
	},
];

/**
 * The command palette: `PickerDialog`, opened from a hotkey rather than a trigger, over the whole
 * window. Rows are grouped, each shows its hotkey where a picker shows an item's type, and Enter
 * runs the highlighted one — `selectLabel` names that act in the footer.
 */
export const CommandPalette = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=1839-4149",
		},
	},
	render: function CommandPalette() {
		const [open, setOpen] = useState(true);

		return (
			<>
				<Button onClick={() => setOpen(true)}>Open command palette</Button>
				<PickerDialog<Command>
					ariaLabel="Command palette"
					closeLabel="Close command palette"
					nothingFoundLabel="No hotkeys found"
					nothingToListLabel="No hotkeys to show"
					getItemKey={(command) => command.id}
					getItemLabel={(command) => command.name}
					getItemType={(command) => <Kbd hotkey={command.hotkey} />}
					itemToStringValue={(command) => `${command.group}: ${command.name}`}
					items={commands}
					open={open}
					onOpenChange={setOpen}
					onSelectItem={() => setOpen(false)}
					placeholder="Search hotkeys…"
					selectLabel="Run"
				/>
			</>
		);
	},
});

/** Nothing in the list before anything was typed. */
export const NothingToList = meta.story({
	args: { items: [] },
});
